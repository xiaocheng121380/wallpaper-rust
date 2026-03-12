use crate::library;
use crate::models::CommandResult;
use std::fs;
use std::path::Path;

fn dir_size_bytes(p: &Path) -> Result<u64, String> {
    if !p.exists() {
        return Ok(0);
    }

    let meta = fs::symlink_metadata(p).map_err(|e| e.to_string())?;
    if meta.is_file() {
        return Ok(meta.len());
    }

    if !meta.is_dir() {
        return Ok(0);
    }

    let mut total: u64 = 0;
    let entries = fs::read_dir(p).map_err(|e| e.to_string())?;
    for ent in entries {
        let ent = ent.map_err(|e| e.to_string())?;
        let child = ent.path();
        total = total.saturating_add(dir_size_bytes(&child)?);
    }
    Ok(total)
}

#[tauri::command]
pub fn cache_get_size() -> CommandResult<u64> {
    let cache_dir = library::get_cache_dir();
    match dir_size_bytes(&cache_dir) {
        Ok(v) => CommandResult::success(v),
        Err(e) => CommandResult::error("CACHE_SIZE_FAILED", &e),
    }
}

#[tauri::command]
pub fn cache_clear() -> CommandResult<bool> {
    let cache_dir = library::get_cache_dir();

    if !cache_dir.exists() {
        return CommandResult::success(true);
    }

    let entries = match fs::read_dir(&cache_dir) {
        Ok(v) => v,
        Err(e) => return CommandResult::error("CACHE_CLEAR_FAILED", &e.to_string()),
    };

    for ent in entries {
        let ent = match ent {
            Ok(v) => v,
            Err(e) => return CommandResult::error("CACHE_CLEAR_FAILED", &e.to_string()),
        };
        let p = ent.path();
        let res = if p.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
        if let Err(e) = res {
            return CommandResult::error(
                "CACHE_CLEAR_FAILED",
                &format!(
                    "Failed to remove {:?}: {}. If video wallpaper is running, stop it first.",
                    p, e
                ),
            );
        }
    }

    CommandResult::success(true)
}
