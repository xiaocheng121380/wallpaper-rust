use crate::library;
use crate::models::CommandResult;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn sanitize_file_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        let invalid = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || ch.is_control();
        if invalid {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let out = out.trim().trim_end_matches('.').to_string();
    if out.is_empty() {
        "download".to_string()
    } else {
        out
    }
}

fn file_name_from_url(raw_url: &str) -> Result<String, String> {
    let u = url::Url::parse(raw_url).map_err(|e| format!("Invalid url: {}", e))?;
    let last = u
        .path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|s| !s.is_empty())
        .unwrap_or("download");

    Ok(sanitize_file_name(last))
}

#[tauri::command]
pub fn download_url_to_downloads(url: String) -> CommandResult<String> {
    let downloads_dir = library::get_data_dir().join("downloads");
    if let Err(e) = fs::create_dir_all(&downloads_dir) {
        return CommandResult::error("DIR_FAILED", &e.to_string());
    }

    let file_name = match file_name_from_url(&url) {
        Ok(v) => v,
        Err(e) => return CommandResult::error("INVALID_URL", &e),
    };

    let dest = downloads_dir.join(&file_name);

    let resp = match reqwest::blocking::get(&url) {
        Ok(r) => r,
        Err(e) => return CommandResult::error("REQUEST_FAILED", &e.to_string()),
    };

    if !resp.status().is_success() {
        return CommandResult::error("HTTP_FAILED", &format!("HTTP {}", resp.status()));
    }

    let bytes = match resp.bytes() {
        Ok(b) => b,
        Err(e) => return CommandResult::error("READ_BODY_FAILED", &e.to_string()),
    };

    // If file exists, add suffix to avoid overwriting.
    let mut final_path = dest.clone();
    if final_path.exists() {
        let stem = Path::new(&file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("download");
        let ext = Path::new(&file_name).extension().and_then(|s| s.to_str());

        for i in 1..1000u32 {
            let candidate = if let Some(ext) = ext {
                downloads_dir.join(format!("{}-{}.{}", stem, i, ext))
            } else {
                downloads_dir.join(format!("{}-{}", stem, i))
            };
            if !candidate.exists() {
                final_path = candidate;
                break;
            }
        }
    }

    let mut f = match File::create(&final_path) {
        Ok(v) => v,
        Err(e) => return CommandResult::error("CREATE_FAILED", &e.to_string()),
    };

    if let Err(e) = f.write_all(&bytes) {
        return CommandResult::error("WRITE_FAILED", &e.to_string());
    }

    CommandResult::success(final_path.to_string_lossy().to_string())
}
