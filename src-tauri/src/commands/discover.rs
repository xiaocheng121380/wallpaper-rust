use crate::library;
use crate::models::{CommandResult, DiscoverHistory};
use std::fs;
use std::path::PathBuf;

fn get_discover_history_path() -> PathBuf {
    library::get_database_dir().join("discover_history.json")
}

#[tauri::command]
pub fn discover_history_get() -> CommandResult<DiscoverHistory> {
    let path = get_discover_history_path();

    if !path.exists() {
        return CommandResult::success(DiscoverHistory::default());
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(history) => CommandResult::success(history),
            Err(_) => CommandResult::success(DiscoverHistory::default()),
        },
        Err(_) => CommandResult::success(DiscoverHistory::default()),
    }
}

#[tauri::command]
pub fn discover_history_update(history: DiscoverHistory) -> CommandResult<DiscoverHistory> {
    let path = get_discover_history_path();

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let content = match serde_json::to_string_pretty(&history) {
        Ok(c) => c,
        Err(e) => return CommandResult::error("SERIALIZE_FAILED", &e.to_string()),
    };

    match fs::write(&path, &content) {
        Ok(_) => CommandResult::success(history),
        Err(e) => CommandResult::error("WRITE_FAILED", &e.to_string()),
    }
}
