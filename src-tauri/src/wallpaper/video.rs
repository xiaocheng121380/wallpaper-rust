use tauri::AppHandle;

use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use crate::library;

#[cfg(target_os = "windows")]
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{HWND, LPARAM};

#[cfg(not(target_os = "windows"))]
type HWND = usize;

#[cfg(not(target_os = "windows"))]
type LPARAM = isize;

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, SendMessageTimeoutW, IsWindow, SMTO_ABORTIFHUNG,
    SMTO_NORMAL,
};

static VIDEO_HWND: OnceLock<Mutex<HWND>> = OnceLock::new();
static WORKERW_HWND: OnceLock<Mutex<HWND>> = OnceLock::new();

fn video_hwnd() -> &'static Mutex<HWND> {
    VIDEO_HWND.get_or_init(|| Mutex::new(0))
}

#[cfg(not(target_os = "windows"))]
unsafe fn find_workerw() -> HWND {
    0
}

fn workerw_hwnd() -> &'static Mutex<HWND> {
    WORKERW_HWND.get_or_init(|| Mutex::new(0))
}

#[cfg(target_os = "windows")]
extern "C" {
    fn ffplay_start(hwnd: HWND, path: *const u16) -> i32;
    fn ffplay_stop();
}

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

#[cfg(target_os = "windows")]
unsafe fn spawn_workerw() {
    let progman = FindWindowW(to_wide("Progman").as_ptr(), std::ptr::null());
    if progman == 0 {
        return;
    }
    let mut result: usize = 0;
    let _ = SendMessageTimeoutW(
        progman,
        0x052C,
        0,
        0,
        SMTO_NORMAL | SMTO_ABORTIFHUNG,
        1000,
        &mut result,
    );
}

#[cfg(target_os = "windows")]
unsafe fn find_workerw() -> HWND {
    let mut target: HWND = 0;
    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        unsafe {
            let def_view = FindWindowExW(
                hwnd,
                0,
                to_wide("SHELLDLL_DefView").as_ptr(),
                std::ptr::null(),
            );
            if def_view != 0 {
                // The WorkerW we need is a sibling of the one hosting SHELLDLL_DefView.
                let workerw = FindWindowExW(
                    0,
                    hwnd,
                    to_wide("WorkerW").as_ptr(),
                    std::ptr::null(),
                );
                if workerw != 0 {
                    let out = lparam as *mut HWND;
                    if !out.is_null() {
                        *out = workerw;
                    }
                    return 0;
                }
            }
            1
        }
    }

    EnumWindows(Some(enum_windows_proc), (&mut target as *mut HWND) as LPARAM);
    target
}

#[cfg(target_os = "windows")]
unsafe fn ensure_workerw() -> HWND {
    let cached = workerw_hwnd().lock().ok().map(|g| *g).unwrap_or(0);
    if cached != 0 && IsWindow(cached) != 0 {
        return cached;
    }

    // Strict strategy:
    // - Only accept the dedicated WorkerW sibling behind icons.
    // - If not found, try spawning WorkerW and retry a few times.
    // Fallbacking to Progman/DefView host often attaches to the wrong layer and results in "not visible".
    let mut workerw = 0;
    for _ in 0..3 {
        workerw = find_workerw();
        if workerw != 0 {
            break;
        }
        spawn_workerw();
        std::thread::sleep(Duration::from_millis(120));
    }
    if workerw != 0 {
        if let Ok(mut g) = workerw_hwnd().lock() {
            *g = workerw;
        }
    }
    workerw
}

pub fn apply_video_wallpaper(app: &AppHandle, path: &str) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        return crate::platform::platform::apply_video_wallpaper(app, path);
    }

    #[cfg(target_os = "windows")]
    {
        let _ = app;

        let src = std::path::Path::new(path);
        if !src.exists() {
            return Err(format!("Video file not found: {}", path));
        }

        let settings = library::load_settings();

        // Prepare video (with caching) - this may take time on first run
        let prepared_path = library::prepare_video_wallpaper_source(path, &settings)?;
        let prepared = std::path::Path::new(&prepared_path);
        if !prepared.exists() {
            return Err(format!("Prepared video file not found: {}", prepared_path));
        }

        log::info!("[视频壁纸] 应用：源文件={} 已准备文件={}", path, prepared_path);

        let workerw = unsafe { ensure_workerw() };
        if workerw == 0 {
            return Err("Failed to find WorkerW window".to_string());
        }

        log::info!("[视频壁纸] WorkerW 句柄=0x{:X}", workerw as usize);

        {
            let mut g = video_hwnd().lock().map_err(|_| "video hwnd lock poisoned".to_string())?;
            *g = workerw;
        }

        let wide = to_wide(&prepared_path);

        log::info!("[视频壁纸] 启动 ffplay...");
        let hr = unsafe { ffplay_start(workerw, wide.as_ptr()) };
        log::info!("[视频壁纸] ffplay_start 返回值={}", hr);

        if hr < 0 {
            if let Ok(mut g) = video_hwnd().lock() {
                *g = 0;
            }
            return Err(format!("ffplay_start failed: {}", hr));
        }

        // Stop old video AFTER new one starts (prevents black flash)
        // The new ffplay will replace the old one in the same WorkerW window
        log::info!("[视频壁纸] 新视频已启动，将在后台停止旧视频");

        Ok(())
    }
}

pub fn stop_video_wallpaper(app: &AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        return crate::platform::platform::stop_video_wallpaper(app);
    }

    #[cfg(target_os = "windows")]
    {
        let _ = app;

        unsafe {
            ffplay_stop();
        }

        if let Ok(mut g) = video_hwnd().lock() {
            *g = 0;
        }

        // Clear cached WorkerW; it can change across toggles / explorer restarts.
        if let Ok(mut g) = workerw_hwnd().lock() {
            *g = 0;
        }
        Ok(())
    }
}
