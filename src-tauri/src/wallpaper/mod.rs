pub mod image;
pub mod video;
pub mod web;

use crate::models::CommandResult;
use tauri::AppHandle;

pub trait WallpaperEngine {
    fn apply(&self, path: &str) -> Result<(), String>;
    fn stop(&self) -> Result<(), String>;
}

#[tauri::command]
pub fn wallpaper_apply(app: AppHandle, wallpaper_id: String, path: String, wallpaper_type: String) -> CommandResult<bool> {
    use crate::library::{load_settings, save_settings};

    log::info!("[壁纸] 开始应用：id={} type={}", wallpaper_id, wallpaper_type);

    // Important invariant on Windows:
    // - video wallpaper uses a persistent WorkerW child window.
    // - if we don't stop it, image wallpaper may "apply" but remain hidden behind the video window.
    #[cfg(target_os = "windows")]
    {
        if wallpaper_type.as_str() != "video" {
            if let Err(e) = video::stop_video_wallpaper(&app) {
                log::warn!("[壁纸] 应用 {} 前停止视频壁纸失败：{}", wallpaper_type, e);
            }
        }
    }

    let result = match wallpaper_type.as_str() {
        "image" => image::apply_image_wallpaper(&path),
        "video" => video::apply_video_wallpaper(&app, &path),
        "web" => web::apply_web_wallpaper(&path),
        _ => Err("Unsupported wallpaper type".to_string()),
    };

    match result {
        Ok(_) => {
            // Save current wallpaper ID
            let mut settings = load_settings();
            settings.current_wallpaper_id = Some(wallpaper_id.clone());
            if let Err(e) = save_settings(&settings) {
                log::warn!("[壁纸] 保存当前壁纸 id 失败：{}", e);
            }
            CommandResult::success(true)
        },
        Err(e) => {
            if cfg!(target_os = "linux")
                && e.contains("Video wallpaper is not supported on Linux")
            {
                return CommandResult::error("VIDEO_WALLPAPER_NOT_SUPPORTED", "");
            }
            CommandResult::error("APPLY_FAILED", &e)
        },
    }
}

#[tauri::command]
pub fn wallpaper_stop(app: AppHandle) -> CommandResult<bool> {
    #[cfg(target_os = "windows")]
    {
        if let Err(e) = video::stop_video_wallpaper(&app) {
            return CommandResult::error("STOP_FAILED", &e);
        }
    }
    CommandResult::success(true)
}
