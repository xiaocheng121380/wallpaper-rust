use crate::library;
use crate::models::{CommandResult, Settings};
use std::fs;
use std::path::PathBuf;

fn legacy_settings_path() -> PathBuf {
    library::get_data_dir().join("settings.json")
}

fn migrate_legacy_settings_if_needed() {
    let legacy = legacy_settings_path();
    let current = library::get_settings_path();
    if current.exists() {
        return;
    }
    if !legacy.exists() {
        return;
    }
    if let Some(parent) = current.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = fs::read_to_string(&legacy) {
        if fs::write(&current, content).is_ok() {
            let _ = fs::remove_file(&legacy);
        }
    }
}

#[tauri::command]
pub fn settings_get() -> CommandResult<Settings> {
    migrate_legacy_settings_if_needed();
    let path = library::get_settings_path();

    if !path.exists() {
        return CommandResult::success(Settings::default());
    }

    match fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str(&content) {
                Ok(settings) => CommandResult::success(settings),
                Err(_) => CommandResult::success(Settings::default()),
            }
        }
        Err(_) => CommandResult::success(Settings::default()),
    }
}

#[tauri::command]
pub fn settings_update(settings: Settings) -> CommandResult<Settings> {
    let path = library::get_settings_path();

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let content = match serde_json::to_string_pretty(&settings) {
        Ok(c) => c,
        Err(e) => return CommandResult::error("SERIALIZE_FAILED", &e.to_string()),
    };

    match fs::write(&path, &content) {
        Ok(_) => CommandResult::success(settings),
        Err(e) => CommandResult::error("WRITE_FAILED", &e.to_string()),
    }
}
