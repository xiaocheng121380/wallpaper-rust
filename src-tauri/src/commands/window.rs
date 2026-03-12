use crate::library;
use crate::models::CommandResult;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn window_handle_first_close(
    app: AppHandle,
    minimize_to_tray: bool,
    remember_choice: bool,
) -> CommandResult<bool> {
    // 加载当前设置
    let mut settings = library::load_settings();

    // 更新设置
    settings.minimize_to_tray = minimize_to_tray;
    settings.first_close_handled = remember_choice;

    // 保存设置
    if let Err(e) = library::save_settings(&settings) {
        return CommandResult::error("SAVE_SETTINGS_FAILED", &format!("Failed to save settings: {}", e));
    }

    // 执行关闭操作
    if minimize_to_tray {
        // 最小化到托盘
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    } else {
        // 退出应用
        #[cfg(target_os = "windows")]
        {
            if settings.stop_video_on_exit {
                if let Err(e) = crate::wallpaper::video::stop_video_wallpaper(&app) {
                    log::warn!("停止视频壁纸失败：{}", e);
                }
            }
        }
        app.exit(0);
    }

    CommandResult::success(true)
}
