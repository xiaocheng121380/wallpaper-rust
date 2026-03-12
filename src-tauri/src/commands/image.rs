use crate::library;
use crate::models::{CommandResult, Wallpaper};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn get_thumbnail_base64(id: String, sourcePath: String) -> CommandResult<String> {
    let source = Path::new(&sourcePath);
    if !source.exists() {
        return CommandResult::error("NOT_FOUND", "Source file not found");
    }

    let wallpaper = Wallpaper {
        id: id.clone(),
        title: String::new(),
        local_path: sourcePath.clone(),
        thumbnail_path: None,
        resolution: None,
        file_size: None,
        metadata: None,
        import_time: String::new(),
    };

    let generated_path = match library::generate_thumbnail(&wallpaper) {
        Ok(p) => p,
        Err(e) => return CommandResult::error("GEN_THUMB_FAILED", &e),
    };

    let lower = generated_path.to_lowercase();
    if lower.ends_with(".mp4") || lower.ends_with(".webm") {
        return CommandResult::success(format!("video:{}", generated_path));
    }

    match fs::read(&generated_path) {
        Ok(data) => {
            let base64_data = STANDARD.encode(&data);
            CommandResult::success(format!("data:image/jpeg;base64,{}", base64_data))
        }
        Err(e) => CommandResult::error("READ_FAILED", &e.to_string()),
    }
}

#[tauri::command]
pub fn get_thumbnail_path(id: String, sourcePath: String) -> CommandResult<String> {
    let source = Path::new(&sourcePath);
    if !Path::new(&sourcePath).exists() {
        return CommandResult::error("NOT_FOUND", "Source file not found");
    }

    let wallpaper = Wallpaper {
        id: id.clone(),
        title: String::new(),
        local_path: sourcePath.clone(),
        thumbnail_path: None,
        resolution: None,
        file_size: None,
        metadata: None,
        import_time: String::new(),
    };

    match library::generate_thumbnail(&wallpaper) {
        Ok(p) => {
            let lower = p.to_lowercase();
            if lower.ends_with(".mp4") || lower.ends_with(".webm") {
                CommandResult::success(format!("video:{}", p))
            } else {
                CommandResult::success(p)
            }
        }
        Err(e) => CommandResult::error("GEN_THUMB_FAILED", &e),
    }
}

#[tauri::command]
pub fn get_detail_play_path(id: String, sourcePath: String) -> CommandResult<String> {
    let source = Path::new(&sourcePath);
    if !source.exists() {
        return CommandResult::error("NOT_FOUND", "Source file not found");
    }

    let p = library::get_detail_play_path_for_id(&id);
    if library::has_valid_video_artifact(&p) {
        return CommandResult::success(p.to_string_lossy().to_string());
    }

    CommandResult::error("NOT_READY", "Detail play cache not generated yet")
}

#[tauri::command]
pub fn get_video_base64(filePath: String) -> CommandResult<String> {
    let p = Path::new(&filePath);
    if !p.exists() {
        return CommandResult::error("NOT_FOUND", "Video file not found");
    }

    let mime = match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        _ => "application/octet-stream",
    };

    match fs::read(p) {
        Ok(data) => {
            let base64_data = STANDARD.encode(&data);
            CommandResult::success(format!("data:{};base64,{}", mime, base64_data))
        }
        Err(e) => CommandResult::error("READ_FAILED", &e.to_string()),
    }
}

#[tauri::command]
pub fn get_video_bytes(filePath: String) -> CommandResult<Vec<u8>> {
    let p = Path::new(&filePath);
    if !p.exists() {
        return CommandResult::error("NOT_FOUND", "Video file not found");
    }

    match fs::read(p) {
        Ok(data) => CommandResult::success(data),
        Err(e) => CommandResult::error("READ_FAILED", &e.to_string()),
    }
}
