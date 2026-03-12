use crate::models::CommandResult;
use auto_launch::AutoLaunch;

fn get_auto_launch() -> Result<AutoLaunch, String> {
    let app_name = "WallCraft";
    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {}", e))?;

    Ok(AutoLaunch::new(app_name, app_path.to_str().unwrap_or(""), &[] as &[&str]))
}

#[tauri::command]
pub fn autolaunch_is_enabled() -> CommandResult<bool> {
    match get_auto_launch() {
        Ok(auto_launch) => match auto_launch.is_enabled() {
            Ok(enabled) => CommandResult::success(enabled),
            Err(e) => CommandResult::error("AUTOLAUNCH_CHECK_FAILED", &format!("Failed to check auto-launch status: {}", e)),
        },
        Err(e) => CommandResult::error("AUTOLAUNCH_INIT_FAILED", &e),
    }
}

#[tauri::command]
pub fn autolaunch_enable() -> CommandResult<bool> {
    match get_auto_launch() {
        Ok(auto_launch) => match auto_launch.enable() {
            Ok(_) => CommandResult::success(true),
            Err(e) => CommandResult::error("AUTOLAUNCH_ENABLE_FAILED", &format!("Failed to enable auto-launch: {}", e)),
        },
        Err(e) => CommandResult::error("AUTOLAUNCH_INIT_FAILED", &e),
    }
}

#[tauri::command]
pub fn autolaunch_disable() -> CommandResult<bool> {
    match get_auto_launch() {
        Ok(auto_launch) => match auto_launch.disable() {
            Ok(_) => CommandResult::success(false),
            Err(e) => CommandResult::error("AUTOLAUNCH_DISABLE_FAILED", &format!("Failed to disable auto-launch: {}", e)),
        },
        Err(e) => CommandResult::error("AUTOLAUNCH_INIT_FAILED", &e),
    }
}
