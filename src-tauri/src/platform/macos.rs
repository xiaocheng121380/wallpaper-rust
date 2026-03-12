use tauri::AppHandle;

pub fn apply_image_wallpaper(path: &str) -> Result<(), String> {
    crate::wallpaper::image::apply_image_wallpaper(path)
}

pub fn apply_video_wallpaper(_app: &AppHandle, _path: &str) -> Result<(), String> {
    Err("Video wallpaper is not implemented on macOS yet".to_string())
}

pub fn stop_video_wallpaper(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}
