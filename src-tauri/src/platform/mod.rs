#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux as platform;

#[cfg(target_os = "windows")]
pub use windows as platform;

#[cfg(target_os = "macos")]
pub use macos as platform;

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub mod platform {
    use tauri::AppHandle;

    pub fn apply_image_wallpaper(_path: &str) -> Result<(), String> {
        Err("Unsupported platform".to_string())
    }

    pub fn apply_video_wallpaper(_app: &AppHandle, _path: &str) -> Result<(), String> {
        Err("Unsupported platform".to_string())
    }

    pub fn stop_video_wallpaper(_app: &AppHandle) -> Result<(), String> {
        Ok(())
    }
}
