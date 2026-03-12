use tauri::AppHandle;

pub fn apply_image_wallpaper(path: &str) -> Result<(), String> {
    crate::wallpaper::image::apply_image_wallpaper(path)
}

pub fn apply_video_wallpaper(app: &AppHandle, path: &str) -> Result<(), String> {
    crate::wallpaper::video::apply_video_wallpaper(app, path)
}

pub fn stop_video_wallpaper(app: &AppHandle) -> Result<(), String> {
    crate::wallpaper::video::stop_video_wallpaper(app)
}
