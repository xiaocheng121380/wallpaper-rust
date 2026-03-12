use crate::library;
use crate::models::{CommandResult, Wallpaper};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn is_supported_media_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    if path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .eq_ignore_ascii_case("lnk")
    {
        return false;
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let image_formats = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "ico", "svg"];
    let video_formats = ["mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v"];
    image_formats.contains(&ext.as_str()) || video_formats.contains(&ext.as_str())
}

fn collect_media_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }

            if is_supported_media_file(&p) {
                out.push(p);
            }
        }
    }

    out
}

#[tauri::command]
pub fn library_list() -> CommandResult<Vec<Wallpaper>> {
    match library::load_all_wallpapers() {
        Ok(wallpapers) => CommandResult::success(wallpapers),
        Err(e) => CommandResult::error("LOAD_FAILED", &e),
    }
}

#[tauri::command]
pub fn library_get(id: String) -> CommandResult<Wallpaper> {
    match library::load_index() {
        Ok(index) => {
            match index.wallpapers.iter().find(|w| w.id == id) {
                Some(w) => CommandResult::success(w.clone()),
                None => CommandResult::error("NOT_FOUND", "Wallpaper not found"),
            }
        }
        Err(e) => CommandResult::error("LOAD_FAILED", &e),
    }
}

#[tauri::command]
pub fn library_import(paths: Vec<String>) -> CommandResult<Vec<Wallpaper>> {
    if let Err(e) = library::ensure_dirs() {
        return CommandResult::error("DIR_FAILED", &e);
    }

    let mut index = match library::load_index() {
        Ok(i) => i,
        Err(e) => return CommandResult::error("LOAD_FAILED", &e),
    };

    let mut imported = Vec::new();

    for input in paths {
        let p = PathBuf::from(&input);
        let mut targets: Vec<PathBuf> = Vec::new();

        if p.is_dir() {
            targets.extend(collect_media_files(&p));
        } else if is_supported_media_file(&p) {
            targets.push(p);
        } else {
            log::warn!("跳过不支持的导入路径：{}", input);
            continue;
        }

        for file in targets {
            let file_str = file.to_string_lossy().to_string();
            match library::import_file(&file_str) {
                Ok(mut wallpaper) => {
                    if let Ok(thumb) = library::generate_thumbnail(&wallpaper) {
                        wallpaper.thumbnail_path = Some(thumb);
                    }
                    imported.push(wallpaper.clone());
                    index.wallpapers.push(wallpaper);
                }
                Err(e) => {
                    log::warn!("导入失败：{}，原因：{}", file_str, e);
                }
            }
        }
    }

    if let Err(e) = library::save_index(&index) {
        return CommandResult::error("SAVE_FAILED", &e);
    }

    CommandResult::success(imported)
}

#[tauri::command]
pub fn library_remove(id: String) -> CommandResult<bool> {
    let mut index = match library::load_index() {
        Ok(i) => i,
        Err(e) => return CommandResult::error("LOAD_FAILED", &e),
    };

    let removed = match index.wallpapers.iter().find(|w| w.id == id) {
        Some(w) => w.clone(),
        None => return CommandResult::error("NOT_FOUND", "Wallpaper not found"),
    };

    let original_len = index.wallpapers.len();
    index.wallpapers.retain(|w| w.id != id);

    if index.wallpapers.len() == original_len {
        return CommandResult::error("NOT_FOUND", "Wallpaper not found");
    }

    // 删除原始壁纸文件
    if !removed.local_path.is_empty() {
        let p = Path::new(&removed.local_path);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
    }

    // 删除缩略图/预览文件（优先按记录里的 thumbnail_path 删除，其次按 id 推导删除）
    if let Some(tp) = removed.thumbnail_path.clone() {
        let tp = tp.trim();
        let tp = tp.strip_prefix("video:").unwrap_or(tp);
        if !tp.is_empty() {
            let p = Path::new(tp);
            if p.exists() {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    let thumb_path = library::get_thumbnail_path_for_id(&id);
    if thumb_path.exists() {
        let _ = std::fs::remove_file(&thumb_path);
    }

    let preview_path = library::get_video_preview_path_for_id(&id);
    if preview_path.exists() {
        let _ = std::fs::remove_file(&preview_path);
    }

    let play_path = library::get_detail_play_path_for_id(&id);
    if play_path.exists() {
        let _ = std::fs::remove_file(&play_path);
    }

    // 兼容旧结构：删除 data/library/wallpapers/<id>/
    let wallpaper_dir = library::get_library_dir().join(&id);
    if wallpaper_dir.exists() {
        let _ = std::fs::remove_dir_all(&wallpaper_dir);
    }

    if let Err(e) = library::save_index(&index) {
        return CommandResult::error("SAVE_FAILED", &e);
    }

    CommandResult::success(true)
}

#[tauri::command]
pub fn library_update_title(id: String, title: String) -> CommandResult<Wallpaper> {
    let mut index = match library::load_index() {
        Ok(i) => i,
        Err(e) => return CommandResult::error("LOAD_FAILED", &e),
    };

    let wallpaper = index.wallpapers.iter_mut().find(|w| w.id == id);

    match wallpaper {
        Some(w) => {
            w.title = title;
            let updated = w.clone();

            if let Err(e) = library::save_index(&index) {
                return CommandResult::error("SAVE_FAILED", &e);
            }

            CommandResult::success(updated)
        }
        None => CommandResult::error("NOT_FOUND", "Wallpaper not found"),
    }
}

#[tauri::command]
pub fn thumbnail_get(id: String) -> CommandResult<String> {
    let thumb_path = library::get_thumbnail_path_for_id(&id);

    if thumb_path.exists() {
        CommandResult::success(thumb_path.to_string_lossy().to_string())
    } else {
        CommandResult::error("NOT_FOUND", "Thumbnail not found")
    }
}
