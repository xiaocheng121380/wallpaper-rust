#ifndef WINVER
#define WINVER 0x0A00
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif

#include <windows.h>
#include <tlhelp32.h>
#include <string>
#include <thread>
#include <atomic>
#include <mutex>

static PROCESS_INFORMATION g_ffplay_process = {0};
static HWND g_ffplay_window = nullptr;
static std::thread g_monitor_thread;
static std::atomic<bool> g_stop(false);
static std::mutex g_mtx;

extern "C" __declspec(dllexport) void ffplay_stop();

// Kill all ffplay.exe processes to ensure singleton
static void kill_all_ffplay_processes() {
    HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnapshot == INVALID_HANDLE_VALUE) {
        return;
    }

    PROCESSENTRY32W pe32;
    pe32.dwSize = sizeof(PROCESSENTRY32W);

    if (Process32FirstW(hSnapshot, &pe32)) {
        do {
            if (_wcsicmp(pe32.szExeFile, L"ffplay.exe") == 0) {
                HANDLE hProcess = OpenProcess(PROCESS_TERMINATE, FALSE, pe32.th32ProcessID);
                if (hProcess) {
                    TerminateProcess(hProcess, 0);
                    CloseHandle(hProcess);
                }
            }
        } while (Process32NextW(hSnapshot, &pe32));
    }

    CloseHandle(hSnapshot);
}

static std::wstring exe_dir() {
    wchar_t buf[MAX_PATH];
    DWORD n = GetModuleFileNameW(nullptr, buf, MAX_PATH);
    if (n == 0 || n >= MAX_PATH) return std::wstring();
    std::wstring p(buf, n);
    size_t pos = p.find_last_of(L"\\/");
    if (pos == std::wstring::npos) return std::wstring();
    return p.substr(0, pos);
}

static std::wstring join_w(const std::wstring& a, const std::wstring& b) {
    if (a.empty()) return b;
    if (b.empty()) return a;
    if (a.back() == L'\\' || a.back() == L'/') return a + b;
    return a + L"\\" + b;
}

static HWND find_ffplay_window() {
    // FFplay creates window with title containing the filename
    HWND hwnd = nullptr;

    // Try to find by class name first
    hwnd = FindWindowW(L"SDL_app", nullptr);
    if (hwnd) return hwnd;

    // Fallback: enumerate all windows and find SDL window
    struct EnumData {
        HWND result;
        DWORD pid;
    };

    EnumData data = {nullptr, g_ffplay_process.dwProcessId};

    EnumWindows([](HWND hwnd, LPARAM lparam) -> BOOL {
        EnumData* pdata = reinterpret_cast<EnumData*>(lparam);
        DWORD pid = 0;
        GetWindowThreadProcessId(hwnd, &pid);
        if (pid == pdata->pid && IsWindowVisible(hwnd)) {
            wchar_t className[256];
            GetClassNameW(hwnd, className, 256);
            if (wcsstr(className, L"SDL") != nullptr) {
                pdata->result = hwnd;
                return FALSE;
            }
        }
        return TRUE;
    }, reinterpret_cast<LPARAM>(&data));

    return data.result;
}

extern "C" __declspec(dllexport) int ffplay_start(HWND parent_hwnd, const wchar_t* video_path) {
    if (!parent_hwnd || !video_path || video_path[0] == L'\0') {
        return -1;
    }

    // Kill all ffplay processes to ensure singleton
    kill_all_ffplay_processes();
    Sleep(100);  // Give processes time to terminate

    ffplay_stop();

    std::lock_guard<std::mutex> lock(g_mtx);
    g_stop.store(false);

    // Build ffplay command - try multiple paths
    std::wstring exe_path;
    std::wstring base_dir = exe_dir();

    // Try 1: <exe_dir>\bin\ffplay.exe (release mode)
    exe_path = join_w(base_dir, L"bin\\ffplay.exe");
    if (GetFileAttributesW(exe_path.c_str()) == INVALID_FILE_ATTRIBUTES) {
        // Try 2: <exe_dir>\..\..\bin\ffplay.exe (debug mode: target\debug -> src-tauri\bin)
        std::wstring parent = base_dir;
        size_t pos = parent.find_last_of(L"\\/");
        if (pos != std::wstring::npos) {
            parent = parent.substr(0, pos);  // target
            pos = parent.find_last_of(L"\\/");
            if (pos != std::wstring::npos) {
                parent = parent.substr(0, pos);  // src-tauri
                exe_path = join_w(parent, L"bin\\ffplay.exe");
            }
        }
    }

    // Check if ffplay exists
    if (GetFileAttributesW(exe_path.c_str()) == INVALID_FILE_ATTRIBUTES) {
        return -2;
    }

    // Build command line - keep it simple
    std::wstring cmdline = L"\"" + exe_path + L"\" ";
    cmdline += L"-loop 0 ";  // Loop forever
    cmdline += L"-autoexit ";  // Exit when done
    cmdline += L"-noborder ";  // No window border
    cmdline += L"-left 0 -top 0 ";  // Position

    int screen_w = GetSystemMetrics(SM_CXSCREEN);
    int screen_h = GetSystemMetrics(SM_CYSCREEN);
    cmdline += L"-x " + std::to_wstring(screen_w) + L" -y " + std::to_wstring(screen_h) + L" ";

    cmdline += L"\"" + std::wstring(video_path) + L"\"";

    // Create process
    STARTUPINFOW si = {0};
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE;  // Start hidden

    PROCESS_INFORMATION pi = {0};

    if (!CreateProcessW(
        nullptr,
        const_cast<wchar_t*>(cmdline.c_str()),
        nullptr,
        nullptr,
        FALSE,
        CREATE_NO_WINDOW,
        nullptr,
        nullptr,
        &si,
        &pi)) {
        return -3;
    }

    g_ffplay_process = pi;

    // Wait for window to appear and embed it
    g_monitor_thread = std::thread([parent_hwnd]() {
        Sleep(50);  // Reduced delay

        HWND ffplay_wnd = nullptr;
        for (int i = 0; i < 40 && !g_stop.load(); i++) {
            ffplay_wnd = find_ffplay_window();
            if (ffplay_wnd) break;
            Sleep(25);  // Faster polling
        }

        if (ffplay_wnd && IsWindow(ffplay_wnd) && IsWindow(parent_hwnd)) {
            std::lock_guard<std::mutex> lock(g_mtx);
            g_ffplay_window = ffplay_wnd;

            // Hide window immediately before any modifications
            ShowWindow(ffplay_wnd, SW_HIDE);

            // Convert to child window
            LONG_PTR style = GetWindowLongPtrW(ffplay_wnd, GWL_STYLE);
            style &= ~(WS_OVERLAPPEDWINDOW | WS_POPUP | WS_CAPTION | WS_THICKFRAME);
            style |= (WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS);
            SetWindowLongPtrW(ffplay_wnd, GWL_STYLE, style);

            LONG_PTR exstyle = GetWindowLongPtrW(ffplay_wnd, GWL_EXSTYLE);
            exstyle &= ~(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_WINDOWEDGE | WS_EX_DLGMODALFRAME);
            SetWindowLongPtrW(ffplay_wnd, GWL_EXSTYLE, exstyle);

            SetParent(ffplay_wnd, parent_hwnd);

            // Position and show in one atomic operation
            SetWindowPos(
                ffplay_wnd,
                HWND_BOTTOM,
                0, 0,
                GetSystemMetrics(SM_CXSCREEN),
                GetSystemMetrics(SM_CYSCREEN),
                SWP_SHOWWINDOW | SWP_FRAMECHANGED | SWP_NOACTIVATE);

            UpdateWindow(ffplay_wnd);
        }

        // Monitor process
        while (!g_stop.load()) {
            if (g_ffplay_process.hProcess) {
                DWORD exit_code = 0;
                if (GetExitCodeProcess(g_ffplay_process.hProcess, &exit_code)) {
                    if (exit_code != STILL_ACTIVE) {
                        break;
                    }
                }
            }
            Sleep(500);
        }
    });

    return 0;
}

extern "C" __declspec(dllexport) void ffplay_stop() {
    g_stop.store(true);

    if (g_monitor_thread.joinable()) {
        g_monitor_thread.join();
    }

    std::lock_guard<std::mutex> lock(g_mtx);

    if (g_ffplay_window) {
        // Hide window immediately to prevent visual flash
        if (IsWindow(g_ffplay_window)) {
            ShowWindow(g_ffplay_window, SW_HIDE);
        }
        g_ffplay_window = nullptr;
    }

    if (g_ffplay_process.hProcess) {
        // Quick termination without waiting
        TerminateProcess(g_ffplay_process.hProcess, 0);

        // Don't wait for process to exit - just close handles
        // The OS will clean up the process
        CloseHandle(g_ffplay_process.hProcess);
        CloseHandle(g_ffplay_process.hThread);
        g_ffplay_process = {0};
    }
}
