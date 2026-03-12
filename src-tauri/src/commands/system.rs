use crate::library;
use crate::models::CommandResult;
use crate::media_server;

#[tauri::command]
pub fn system_get_platform() -> CommandResult<String> {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    CommandResult::success(platform.to_string())
}

#[tauri::command]
pub fn system_get_data_dir() -> CommandResult<String> {
    CommandResult::success(library::get_data_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub fn system_get_log_dir() -> CommandResult<String> {
    let data_dir = library::get_data_dir();
    let log_dir = data_dir.join("logs");
    CommandResult::success(log_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn system_open_path(path: String) -> CommandResult<bool> {
    match open::that(&path) {
        Ok(_) => CommandResult::success(true),
        Err(e) => CommandResult::error("OPEN_FAILED", &e.to_string()),
    }
}

#[tauri::command]
pub fn system_get_media_base_url() -> CommandResult<String> {
    match media_server::get_base_url() {
        Some(url) => CommandResult::success(url),
        None => CommandResult::error("NOT_READY", "Media server not started"),
    }
}

#[derive(serde::Serialize)]
pub struct ScreenResolution {
    pub width: i32,
    pub height: i32,
}

#[tauri::command]
pub fn system_get_screen_resolution() -> CommandResult<ScreenResolution> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        CommandResult::success(ScreenResolution { width, height })
    }

    #[cfg(not(target_os = "windows"))]
    {
        CommandResult::error("UNSUPPORTED", "Screen resolution query only supported on Windows")
    }
}
