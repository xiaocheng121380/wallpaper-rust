use crate::library;
use crate::models::CommandResult;
use serde_json::Value;
use std::fs;
use std::path::{PathBuf};

fn is_safe_file_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Reject any path separator / drive-like patterns.
    if name.contains('/') || name.contains('\\') || name.contains(':') {
        return false;
    }
    // Basic allowlist to avoid weird characters.
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn normalize_json_file_name(name: &str) -> Option<String> {
    if !is_safe_file_name(name) {
        return None;
    }
    if name.ends_with(".json") {
        Some(name.to_string())
    } else {
        Some(format!("{}.json", name))
    }
}

fn database_file_path(file_name: &str) -> Result<PathBuf, String> {
    let normalized = normalize_json_file_name(file_name)
        .ok_or_else(|| "Invalid file name".to_string())?;

    // Always keep it inside data/database
    Ok(library::get_database_dir().join(normalized))
}

#[tauri::command]
pub fn database_json_get(fileName: String) -> CommandResult<Value> {
    let path = match database_file_path(&fileName) {
        Ok(p) => p,
        Err(e) => return CommandResult::error("INVALID_NAME", &e),
    };

    if !path.exists() {
        return CommandResult::success(Value::Null);
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(v) => CommandResult::success(v),
            Err(e) => CommandResult::error("PARSE_FAILED", &e.to_string()),
        },
        Err(e) => CommandResult::error("READ_FAILED", &e.to_string()),
    }
}

#[tauri::command]
pub fn database_json_set(fileName: String, value: Value) -> CommandResult<bool> {
    let path = match database_file_path(&fileName) {
        Ok(p) => p,
        Err(e) => return CommandResult::error("INVALID_NAME", &e),
    };

    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return CommandResult::error("DIR_FAILED", &e.to_string());
        }
    }

    let content = match serde_json::to_string_pretty(&value) {
        Ok(c) => c,
        Err(e) => return CommandResult::error("SERIALIZE_FAILED", &e.to_string()),
    };

    let tmp_path = path.with_extension("json.tmp");
    if let Err(e) = fs::write(&tmp_path, &content) {
        return CommandResult::error("WRITE_FAILED", &e.to_string());
    }
    if let Err(e) = fs::rename(&tmp_path, &path) {
        return CommandResult::error("RENAME_FAILED", &e.to_string());
    }

    CommandResult::success(true)
}
