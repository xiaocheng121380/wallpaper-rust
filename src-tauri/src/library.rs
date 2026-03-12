use crate::models::{LibraryIndex, MediaMetadata, Wallpaper, Settings};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;
use chrono::Local;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::sync::{Mutex, OnceLock};
use image::ImageReader;
use serde_json::Value;

#[cfg(target_os = "linux")]
use std::ffi::OsString;

const MIN_VALID_VIDEO_BYTES: u64 = 1024;
const VIDEO_ARTIFACT_SETTLE_RETRIES: usize = 4;
const VIDEO_ARTIFACT_SETTLE_DELAY_MS: u64 = 150;

static VIDEO_ARTIFACT_IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn video_artifact_in_flight() -> &'static Mutex<HashSet<String>> {
    VIDEO_ARTIFACT_IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

struct VideoArtifactGenerationGuard {
    key: String,
}

impl Drop for VideoArtifactGenerationGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = video_artifact_in_flight().lock() {
            guard.remove(&self.key);
        }
    }
}

fn normalize_opt_u32(v: Option<u32>) -> Option<u32> {
    match v {
        Some(0) => None,
        Some(x) => Some(x),
        None => None,
    }
}

fn normalize_opt_u8(v: Option<u8>) -> Option<u8> {
    match v {
        Some(0) => None,
        Some(x) => Some(x),
        None => None,
    }
}

pub fn get_data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("WALLPAPER_DATA_DIR") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(debug_assertions)]
    {
        // 开发态：默认放到项目根目录下的 data/，便于调试和查看生成文件
        // CARGO_MANIFEST_DIR 指向 src-tauri
        return PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("data");
    }

    #[cfg(not(debug_assertions))]
    {
        return std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("data");
    }
}

fn ffprobe_file_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        return "ffprobe.exe";
    }
    #[cfg(not(target_os = "windows"))]
    {
        return "ffprobe";
    }
}

pub fn resolve_bundled_ffprobe() -> Option<PathBuf> {
    let name = ffprobe_file_name();

    #[cfg(debug_assertions)]
    {
        let bundled = Path::new(env!("CARGO_MANIFEST_DIR")).join("bin").join(name);
        if bundled.exists() {
            return Some(bundled);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p1 = dir.join(name);
            if p1.exists() {
                return Some(p1);
            }

            let p2 = dir.join("bin").join(name);
            if p2.exists() {
                return Some(p2);
            }
        }
    }

    None
}

fn is_valid_video_artifact(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() > MIN_VALID_VIDEO_BYTES => metadata,
        _ => return false,
    };

    if metadata.len() <= MIN_VALID_VIDEO_BYTES {
        return false;
    }

    let Some(ffprobe) = resolve_bundled_ffprobe() else {
        return true;
    };

    let mut cmd = Command::new(&ffprobe);
    inject_bundled_tool_env(&mut cmd, &ffprobe);

    match cmd
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_type,width,height",
            "-of",
            "json",
            path.to_str().unwrap_or(""),
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parsed: Result<Value, _> = serde_json::from_str(&stdout);
            match parsed {
                Ok(value) => value
                    .get("streams")
                    .and_then(|streams| streams.as_array())
                    .map(|streams| !streams.is_empty())
                    .unwrap_or(false),
                Err(_) => false,
            }
        }
        Ok(output) => {
            log::warn!(
                "ffprobe 校验视频预览失败：path={} status={} stderr={}",
                path.to_string_lossy(),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
            false
        }
        Err(e) => {
            log::warn!(
                "执行 ffprobe 校验视频预览失败：path={} err={}",
                path.to_string_lossy(),
                e
            );
            false
        }
    }
}

fn ensure_valid_video_artifact(path: &Path) -> bool {
    if is_valid_video_artifact(path) {
        return true;
    }

    if path.exists() {
        log::warn!("检测到无效视频缓存，删除后重建：{}", path.to_string_lossy());
        let _ = fs::remove_file(path);
    }

    false
}

fn acquire_video_artifact_generation_guard(path: &Path) -> Result<VideoArtifactGenerationGuard, String> {
    let key = path.to_string_lossy().to_string();

    loop {
        if has_valid_video_artifact(path) {
            return Err(key);
        }

        let acquired = {
            let mut guard = video_artifact_in_flight()
                .lock()
                .map_err(|_| "video artifact generation lock poisoned".to_string())?;
            guard.insert(key.clone())
        };

        if acquired {
            return Ok(VideoArtifactGenerationGuard { key });
        }

        std::thread::sleep(Duration::from_millis(VIDEO_ARTIFACT_SETTLE_DELAY_MS));
    }
}

fn wait_for_valid_video_artifact(path: &Path) -> bool {
    if is_valid_video_artifact(path) {
        return true;
    }

    for _ in 0..VIDEO_ARTIFACT_SETTLE_RETRIES {
        std::thread::sleep(Duration::from_millis(VIDEO_ARTIFACT_SETTLE_DELAY_MS));
        if is_valid_video_artifact(path) {
            log::info!(
                "视频缓存延迟校验成功：path={}",
                path.to_string_lossy()
            );
            return true;
        }
    }

    false
}

fn ensure_valid_video_artifact_with_wait(path: &Path) -> bool {
    if wait_for_valid_video_artifact(path) {
        return true;
    }

    if path.exists() {
        log::warn!(
            "检测到无效视频缓存（延迟校验后仍失败），删除后重建：{}",
            path.to_string_lossy()
        );
        let _ = fs::remove_file(path);
    }

    false
}

pub fn has_valid_video_artifact(path: &Path) -> bool {
    wait_for_valid_video_artifact(path)
}

fn inject_bundled_tool_env(cmd: &mut Command, tool_path: &Path) {
    #[cfg(target_os = "linux")]
    {
        if let Some(dir) = tool_path.parent() {
            let mut parts: Vec<OsString> = Vec::new();
            parts.push(dir.as_os_str().to_os_string());

            let lib_dir = dir.join("lib");
            if lib_dir.exists() {
                parts.push(lib_dir.as_os_str().to_os_string());
            }

            if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
                if !existing.is_empty() {
                    parts.push(existing);
                }
            }

            let mut joined = OsString::new();
            for (i, p) in parts.into_iter().enumerate() {
                if i > 0 {
                    joined.push(":");
                }
                joined.push(p);
            }
            cmd.env("LD_LIBRARY_PATH", joined);
        }
    }
}

pub fn load_settings() -> Settings {
    let path = get_settings_path();

    if !path.exists() {
        return Settings::default();
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(settings) => settings,
            Err(_) => Settings::default(),
        },
        Err(_) => Settings::default(),
    }
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = get_settings_path();
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}

fn video_wallpaper_cache_path(
    source_path: &str,
    max_width: Option<u32>,
    max_height: Option<u32>,
    fps: Option<u32>,
    crf: Option<u8>,
    bitrate_kbps: Option<u32>,
) -> PathBuf {
    let s = format!(
        "{}|w={:?}|h={:?}|fps={:?}|crf={:?}|br_kbps={:?}",
        source_path, max_width, max_height, fps, crf, bitrate_kbps
    );
    let key = Uuid::new_v5(&Uuid::NAMESPACE_URL, s.as_bytes()).to_string();
    get_cache_dir()
        .join("video_wallpaper")
        .join(format!("{}.mp4", key))
}

pub fn get_cached_video_path(source_path: &str, settings: &Settings) -> Option<String> {
    let max_w = normalize_opt_u32(settings.video_wallpaper.max_width);
    let max_h = normalize_opt_u32(settings.video_wallpaper.max_height);
    let fps = normalize_opt_u32(settings.video_wallpaper.fps);
    let crf = normalize_opt_u8(settings.video_wallpaper.crf);
    let bitrate_kbps = normalize_opt_u32(settings.video_wallpaper.bitrate_kbps);

    // Check if transcoding would be needed
    let mut force_h264 = false;
    if is_video_path(source_path) {
        if let Some(meta) = extract_media_metadata(source_path) {
            if let Some(codec) = meta.video_codec {
                let c = codec.trim().to_lowercase();
                if !c.is_empty() && c != "h264" {
                    force_h264 = true;
                }
            }
        }
    }

    // If no transcoding needed, return None (use original)
    if !force_h264
        && max_w.is_none()
        && max_h.is_none()
        && fps.is_none()
        && crf.is_none()
        && bitrate_kbps.is_none()
    {
        return None;
    }

    let cache_path = video_wallpaper_cache_path(source_path, max_w, max_h, fps, crf, bitrate_kbps);
    Some(cache_path.to_string_lossy().to_string())
}

pub fn prepare_video_wallpaper_source(source_path: &str, settings: &Settings) -> Result<String, String> {
    let max_w = normalize_opt_u32(settings.video_wallpaper.max_width);
    let max_h = normalize_opt_u32(settings.video_wallpaper.max_height);
    let fps = normalize_opt_u32(settings.video_wallpaper.fps);
    let crf = normalize_opt_u8(settings.video_wallpaper.crf);
    let bitrate_kbps = normalize_opt_u32(settings.video_wallpaper.bitrate_kbps);

    let mut force_h264 = false;
    if is_video_path(source_path) {
        if let Some(meta) = extract_media_metadata(source_path) {
            if let Some(codec) = meta.video_codec {
                let c = codec.trim().to_lowercase();
                if !c.is_empty() && c != "h264" {
                    force_h264 = true;
                    log::info!("[视频壁纸] 强制转码为 H.264（源编码={})", c);
                }
            }
        }
    }

    if !force_h264
        && max_w.is_none()
        && max_h.is_none()
        && fps.is_none()
        && crf.is_none()
        && bitrate_kbps.is_none()
    {
        log::info!("[视频壁纸] 跳过转码（直接使用原文件）：{:?}", source_path);
        return Ok(source_path.to_string());
    }

    let ffmpeg = resolve_bundled_ffmpeg().ok_or_else(|| "ffmpeg not bundled".to_string())?;

    let out_path = video_wallpaper_cache_path(source_path, max_w, max_h, fps, crf, bitrate_kbps);
    if out_path.exists() {
        log::info!("[视频壁纸] 命中缓存：{:?}", out_path);
        return Ok(out_path.to_string_lossy().to_string());
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
    }

    let mut vf_parts: Vec<String> = Vec::new();
    if max_w.is_some() || max_h.is_some() {
        let w = max_w.map(|v| v.to_string()).unwrap_or_else(|| "iw".to_string());
        let h = max_h.map(|v| v.to_string()).unwrap_or_else(|| "ih".to_string());
        vf_parts.push(format!(
            "scale=w='min(iw,{})':h='min(ih,{})':force_original_aspect_ratio=decrease",
            w, h
        ));
    }
    if let Some(fps) = fps {
        vf_parts.push(format!("fps={}", fps));
    }
    let vf = if vf_parts.is_empty() {
        None
    } else {
        Some(vf_parts.join(","))
    };

    let out_tmp = out_path.with_extension("tmp.mp4");

    log::info!(
        "[视频壁纸] 开始转码：src={:?} out={:?} max_w={:?} max_h={:?} fps={:?} crf={:?}",
        source_path,
        out_path,
        max_w,
        max_h,
        fps,
        crf
    );

    let mut cmd = Command::new(&ffmpeg);
    inject_bundled_tool_env(&mut cmd, &ffmpeg);
    cmd.arg("-y");
    cmd.arg("-i");
    cmd.arg(source_path);
    cmd.arg("-an");
    cmd.arg("-c:v");
    cmd.arg("libx264");
    cmd.arg("-preset");
    cmd.arg("ultrafast");
    cmd.arg("-tune");
    cmd.arg("fastdecode");
    if let Some(br) = bitrate_kbps {
        cmd.arg("-b:v");
        cmd.arg(format!("{}k", br));
        cmd.arg("-maxrate");
        cmd.arg(format!("{}k", br));
        cmd.arg("-bufsize");
        cmd.arg(format!("{}k", br.saturating_mul(2).max(br)));
    } else {
        cmd.arg("-crf");
        cmd.arg(crf.unwrap_or(23).to_string());
    }

    cmd.arg("-f");
    cmd.arg("mp4");
    cmd.arg("-pix_fmt");
    cmd.arg("yuv420p");
    cmd.arg("-movflags");
    cmd.arg("+faststart");
    if let Some(vf) = &vf {
        cmd.arg("-vf");
        cmd.arg(vf);
    }
    cmd.arg(out_tmp.to_string_lossy().to_string());

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {} ({})", e, ffmpeg.to_string_lossy()))?;

    if output.status.success() && out_tmp.exists() {
        let _ = fs::remove_file(&out_path);
        fs::rename(&out_tmp, &out_path).map_err(|e| format!("Failed to move file: {}", e))?;
        return Ok(out_path.to_string_lossy().to_string());
    }

    let _ = fs::remove_file(&out_tmp);

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "ffmpeg failed without output".to_string()
    };

    log::warn!(
        "[视频壁纸] 转码失败：status={} src={:?} out={:?} detail={}",
        output.status,
        source_path,
        out_path,
        detail
    );

    Err(format!("status={} detail={}", output.status, detail))
}

fn parse_rational_fps(v: &str) -> Option<f64> {
    let s = v.trim();
    if s.is_empty() {
        return None;
    }
    if let Some((a, b)) = s.split_once('/') {
        let num: f64 = a.trim().parse().ok()?;
        let den: f64 = b.trim().parse().ok()?;
        if den == 0.0 {
            return None;
        }
        return Some(num / den);
    }
    s.parse::<f64>().ok()
}

fn extract_media_metadata(local_path: &str) -> Option<MediaMetadata> {
    if local_path.trim().is_empty() {
        return None;
    }
    let p = Path::new(local_path);
    if !p.exists() {
        return None;
    }

    if !is_video_path(local_path) {
        let mut meta = MediaMetadata::default();

        if let Ok((w, h)) = image::image_dimensions(p) {
            meta.width = Some(w);
            meta.height = Some(h);
        }

        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .trim();
        if !ext.is_empty() {
            meta.format_name = Some(ext.to_lowercase());
        }

        if meta.width.is_some() || meta.height.is_some() || meta.format_name.is_some() {
            return Some(meta);
        }
        return None;
    }

    let ffprobe = resolve_bundled_ffprobe()?;
    let mut cmd = Command::new(&ffprobe);
    inject_bundled_tool_env(&mut cmd, &ffprobe);
    let output = cmd
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            local_path,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            log::warn!("ffprobe 执行失败：{}", stderr);
        }
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let root: Value = serde_json::from_str(&stdout).ok()?;

    let mut meta = MediaMetadata::default();

    if let Some(fmt) = root.get("format") {
        if let Some(name) = fmt.get("format_name").and_then(|v| v.as_str()) {
            if !name.trim().is_empty() {
                meta.format_name = Some(name.to_string());
            }
        }
        if let Some(d) = fmt.get("duration").and_then(|v| v.as_str()) {
            meta.duration_sec = d.parse::<f64>().ok();
        } else if let Some(d) = fmt.get("duration").and_then(|v| v.as_f64()) {
            meta.duration_sec = Some(d);
        }
        if let Some(b) = fmt.get("bit_rate").and_then(|v| v.as_str()) {
            meta.bit_rate = b.parse::<u64>().ok();
        } else if let Some(b) = fmt.get("bit_rate").and_then(|v| v.as_u64()) {
            meta.bit_rate = Some(b);
        }
    }

    if let Some(streams) = root.get("streams").and_then(|v| v.as_array()) {
        for s in streams {
            let codec_type = s.get("codec_type").and_then(|v| v.as_str()).unwrap_or("");
            if codec_type == "video" && meta.video_codec.is_none() {
                meta.video_codec = s
                    .get("codec_name")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                meta.pix_fmt = s
                    .get("pix_fmt")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                meta.width = s.get("width").and_then(|v| v.as_u64()).map(|v| v as u32);
                meta.height = s.get("height").and_then(|v| v.as_u64()).map(|v| v as u32);

                if let Some(fr) = s.get("avg_frame_rate").and_then(|v| v.as_str()) {
                    meta.fps = parse_rational_fps(fr);
                } else if let Some(fr) = s.get("r_frame_rate").and_then(|v| v.as_str()) {
                    meta.fps = parse_rational_fps(fr);
                }
            }

            if codec_type == "audio" && meta.audio_codec.is_none() {
                meta.audio_codec = s
                    .get("codec_name")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                meta.sample_rate = s
                    .get("sample_rate")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<u32>().ok());
                meta.channels = s.get("channels").and_then(|v| v.as_u64()).map(|v| v as u32);
            }
        }
    }

    if meta.width.is_some()
        || meta.height.is_some()
        || meta.duration_sec.is_some()
        || meta.video_codec.is_some()
        || meta.audio_codec.is_some()
        || meta.format_name.is_some()
    {
        Some(meta)
    } else {
        None
    }
}

pub fn get_default_wallpapers_dir() -> PathBuf {
    // 静态壁纸资源目录（用户可直接放文件进去）
    // 结构：data/wallpapers/*
    let p = get_data_dir().join("wallpapers");
    if !p.exists() {
        if let Err(e) = fs::create_dir_all(&p) {
            log::warn!(
                "创建默认壁纸目录失败 {:?}: {}",
                p,
                e
            );
        }
    }
    p
}

pub fn get_library_dir() -> PathBuf {
    get_data_dir().join("library").join("wallpapers")
}

pub fn get_database_dir() -> PathBuf {
    get_data_dir().join("database")
}

pub fn get_thumbnails_dir() -> PathBuf {
    get_data_dir().join("thumbnails")
}

pub fn get_previews_dir() -> PathBuf {
    get_data_dir().join("previews")
}

pub fn get_plays_dir() -> PathBuf {
    get_data_dir().join("plays")
}

pub fn get_thumbnail_path_for_id(id: &str) -> PathBuf {
    let safe = sanitize_file_stem(id);
    get_thumbnails_dir().join(format!("{}.jpg", safe))
}

pub fn get_preview_path_for_id(id: &str) -> PathBuf {
    let safe = sanitize_file_stem(id);
    #[cfg(target_os = "windows")]
    {
        return get_previews_dir().join(format!("{}.mp4", safe));
    }

    #[cfg(not(target_os = "windows"))]
    {
        return get_previews_dir().join(format!("{}.webm", safe));
    }
}

pub fn get_detail_play_path_for_id(id: &str) -> PathBuf {
    let safe = sanitize_file_stem(id);
    get_plays_dir().join(format!("{}.webm", safe))
}

fn sanitize_file_stem(stem: &str) -> String {
    // Windows 文件名禁用字符：<>:"/\\|?* 以及控制字符
    let mut out = String::with_capacity(stem.len());
    for ch in stem.chars() {
        let invalid = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || ch.is_control();
        if invalid {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let out = out.trim().trim_end_matches('.').to_string();
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

pub fn get_video_preview_path_for_id(id: &str) -> PathBuf {
    get_preview_path_for_id(id)
}

pub fn ensure_detail_play_webm(id: &str, source_path: &str) -> Result<String, String> {
    let ffmpeg = resolve_bundled_ffmpeg().ok_or_else(|| "ffmpeg not bundled".to_string())?;
    let out_path = get_detail_play_path_for_id(id);

    if ensure_valid_video_artifact(&out_path) {
        return Ok(out_path.to_string_lossy().to_string());
    }

    let _generation_guard = match acquire_video_artifact_generation_guard(&out_path) {
        Ok(guard) => guard,
        Err(existing) => return Ok(existing),
    };

    if ensure_valid_video_artifact(&out_path) {
        return Ok(out_path.to_string_lossy().to_string());
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
    }

    log::info!(
        "生成详情播放缓存：开始 id={} source={} out={}",
        id,
        source_path,
        out_path.to_string_lossy()
    );

    let tmp_path = {
        out_path.with_extension("tmp.webm")
    };
    let _ = fs::remove_file(&tmp_path);

    let vf = "scale=-2:'min(1080,ih)',fps=30";
    let mut cmd = Command::new(&ffmpeg);
    inject_bundled_tool_env(&mut cmd, &ffmpeg);
    let output = cmd
        .args([
            "-y",
            "-i",
            source_path,
            "-vf",
            vf,
            "-an",
            "-c:v",
            "libvpx",
            "-b:v",
            "1800k",
            "-maxrate",
            "2000k",
            "-bufsize",
            "4000k",
            "-deadline",
            "realtime",
            "-cpu-used",
            "4",
            "-pix_fmt",
            "yuv420p",
            "-f",
            "webm",
            tmp_path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if output.status.success() && tmp_path.exists() {
        if wait_for_valid_video_artifact(&tmp_path) {
            let _ = fs::remove_file(&out_path);
            let _ = fs::rename(&tmp_path, &out_path);
            log::info!(
                "生成详情播放缓存：成功 id={} out={}",
                id,
                out_path.to_string_lossy()
            );
            if ensure_valid_video_artifact_with_wait(&out_path) {
                return Ok(out_path.to_string_lossy().to_string());
            }
        } else {
            log::warn!(
                "生成详情播放缓存：tmp 校验失败 id={} tmp={}",
                id,
                tmp_path.to_string_lossy()
            );
            let _ = fs::remove_file(&tmp_path);
        }
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "ffmpeg failed without output".to_string()
    };

    log::warn!(
        "生成详情播放缓存：失败 id={} status={} detail={}",
        id,
        output.status,
        detail
    );
    let _ = fs::remove_file(&tmp_path);
    Err(format!("status={} detail={}", output.status, detail))
}

pub fn prewarm_video_previews() -> Result<(), String> {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        return Ok(());
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
    let mut index = load_index()?;
    let mut changed = false;

    for w in index.wallpapers.iter_mut() {
        if !is_video_path(&w.local_path) {
            continue;
        }

        let desired = get_video_preview_path_for_id(&w.id);
        let current = w.thumbnail_path.clone().unwrap_or_default();

        // 迁移：如果旧的预览存在但命名不是新规则
        // 注意：预览格式可能变化（例如 mp4 -> webm），此时不能简单 rename（会造成扩展名与容器不匹配）。
        if !current.is_empty() {
            let current_path = PathBuf::from(&current);
            if ensure_valid_video_artifact(&current_path) {
                let desired_str = desired.to_string_lossy().to_string();
                if current != desired_str {
                    if ensure_valid_video_artifact(&desired) {
                        // 目标已存在，直接指向新命名
                        w.thumbnail_path = Some(desired_str);
                        changed = true;
                        continue;
                    }

                    // 目标不存在：生成新预览（而不是 rename 旧文件）
                    match generate_video_preview(&w.id, &w.local_path) {
                        Ok(p) => {
                            w.thumbnail_path = Some(p);
                            changed = true;
                            let _ = fs::remove_file(&current_path);
                            continue;
                        }
                        Err(e) => {
                            log::warn!("生成预览失败：id={} err={}", w.id, e);
                            // 保留旧路径（如果存在）
                            continue;
                        }
                    }
                } else {
                    // 已是新命名且存在
                    continue;
                }
            }
        }

        // 生成缺失预览
        match generate_video_preview(&w.id, &w.local_path) {
            Ok(p) => {
                w.thumbnail_path = Some(p);
                changed = true;
            }
            Err(e) => {
                log::warn!("生成预览失败：id={} err={}", w.id, e);
            }
        }
    }

    if changed {
        save_index(&index)?;
    }

    Ok(())
    }
}

pub fn prewarm_detail_plays() -> Result<(), String> {
    #[cfg(not(target_os = "linux"))]
    {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        use std::sync::mpsc;
        use std::sync::{Arc, Mutex};

        log::info!(
            "预热详情播放缓存：开始 data_dir={} wallpapers_dir={} plays_dir={}",
            get_data_dir().to_string_lossy(),
            get_default_wallpapers_dir().to_string_lossy(),
            get_plays_dir().to_string_lossy()
        );

        let index = load_index()?;
        let total_wallpapers = index.wallpapers.len();
        let mut tasks: Vec<(String, String)> = Vec::new();
        let mut video_count: usize = 0;
        let mut existing_count: usize = 0;
        let mut sample_logged: usize = 0;
        for w in index.wallpapers.iter() {
            if !is_video_path(&w.local_path) {
                continue;
            }

            video_count += 1;
            let out_path = get_detail_play_path_for_id(&w.id);
            if ensure_valid_video_artifact(&out_path) {
                existing_count += 1;
                continue;
            }

            if sample_logged < 5 {
                log::info!(
                    "预热详情播放缓存：入队 id={} local={} out={}",
                    w.id,
                    w.local_path,
                    out_path.to_string_lossy()
                );
                sample_logged += 1;
            }
            tasks.push((w.id.clone(), w.local_path.clone()));
        }

        log::info!(
            "预热详情播放缓存：索引统计 wallpapers={} 视频={} 现有播放={} 缺失播放={}",
            total_wallpapers,
            video_count,
            existing_count,
            tasks.len()
        );

        // 回退：如果 index 中没有视频，或者没有缺失任务，则按 data/wallpapers 扫描视频文件。
        if tasks.is_empty() {
            let mut scanned_videos: usize = 0;
            let mut enqueued: usize = 0;
            let wallpapers_dir = get_default_wallpapers_dir();
            if let Ok(entries) = fs::read_dir(&wallpapers_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if !p.is_file() {
                        continue;
                    }
                    let local_path = p.to_string_lossy().to_string();
                    if !is_video_path(&local_path) {
                        continue;
                    }

                    scanned_videos += 1;
                    let id = index
                        .wallpapers
                        .iter()
                        .find(|w| w.local_path == local_path)
                        .map(|w| w.id.clone())
                        .unwrap_or_else(|| resource_id_for_path(&p));

                    let out_path = get_detail_play_path_for_id(&id);
                    if ensure_valid_video_artifact(&out_path) {
                        continue;
                    }

                    if sample_logged < 5 {
                        log::info!(
                            "预热详情播放缓存：入队(回退扫描) id={} local={} out={}",
                            id,
                            local_path,
                            out_path.to_string_lossy()
                        );
                        sample_logged += 1;
                    }

                    tasks.push((id, local_path));
                    enqueued += 1;
                }
            }

            log::info!(
                "预热详情播放缓存：回退扫描 scanned_videos={} enqueued_missing_plays={}",
                scanned_videos,
                enqueued
            );
        }

        let total = tasks.len();
        if total == 0 {
            log::info!("预热详情播放缓存：无任务");
            return Ok(());
        }

        let max_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1);
        let workers = std::cmp::min(total, std::cmp::min(max_workers, 4));

        log::info!(
            "预热详情播放缓存：tasks={} workers={}",
            total,
            workers
        );

        let (tx, rx) = mpsc::channel::<(String, String)>();
        for t in tasks {
            let _ = tx.send(t);
        }
        drop(tx);

        let rx = Arc::new(Mutex::new(rx));

        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let rx = Arc::clone(&rx);
            handles.push(std::thread::spawn(move || {
                loop {
                    let next = {
                        let guard = match rx.lock() {
                            Ok(g) => g,
                            Err(_) => return,
                        };
                        guard.recv()
                    };

                    let (id, local_path) = match next {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                    if let Err(e) = ensure_detail_play_webm(&id, &local_path) {
                        log::warn!("预热详情播放缓存：失败 id={} err={}", id, e);
                    }
                }
            }));
        }

        for h in handles {
            let _ = h.join();
        }

        log::info!("预热详情播放缓存：完成");
        Ok(())
    }
}

pub fn get_cache_dir() -> PathBuf {
    get_data_dir().join("cache")
}

fn resource_id_for_path(p: &Path) -> String {
    // 用文件名生成稳定 ID：目录调整后也能保持一致（避免整个库重复）
    let s = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let safe = Uuid::new_v5(&Uuid::NAMESPACE_URL, s.as_bytes()).to_string();
    format!("res_{}", safe)
}

fn title_for_path(p: &Path) -> String {
    p.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string()
}

fn is_video_path(p: &str) -> bool {
    let ext = Path::new(p)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let video_formats = ["mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v"];
    video_formats.contains(&ext.as_str())
}

fn ffmpeg_file_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        return "ffmpeg.exe";
    }
    #[cfg(not(target_os = "windows"))]
    {
        return "ffmpeg";
    }
}

pub fn resolve_bundled_ffmpeg() -> Option<PathBuf> {
    let name = ffmpeg_file_name();

    #[cfg(debug_assertions)]
    {
        let bundled = Path::new(env!("CARGO_MANIFEST_DIR")).join("bin").join(name);
        if bundled.exists() {
            return Some(bundled);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p1 = dir.join(name);
            if p1.exists() {
                return Some(p1);
            }

            let p2 = dir.join("bin").join(name);
            if p2.exists() {
                return Some(p2);
            }
        }
    }

    None
}

pub fn generate_video_preview(id: &str, source_path: &str) -> Result<String, String> {
    let ffmpeg = resolve_bundled_ffmpeg().ok_or_else(|| "ffmpeg not bundled".to_string())?;
    let preview_path = get_video_preview_path_for_id(id);

    if ensure_valid_video_artifact(&preview_path) {
        return Ok(preview_path.to_string_lossy().to_string());
    }

    let _generation_guard = match acquire_video_artifact_generation_guard(&preview_path) {
        Ok(guard) => guard,
        Err(existing) => return Ok(existing),
    };

    if ensure_valid_video_artifact(&preview_path) {
        return Ok(preview_path.to_string_lossy().to_string());
    }

    if let Some(parent) = preview_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create dir: {}", e))?;
    }

    let vf = "scale=-2:'min(1080,ih)',fps=30";

    let mut cmd = Command::new(&ffmpeg);
    inject_bundled_tool_env(&mut cmd, &ffmpeg);

    let tmp_path = {
        #[cfg(target_os = "windows")]
        {
            preview_path.with_extension("tmp.mp4")
        }
        #[cfg(not(target_os = "windows"))]
        {
            preview_path.with_extension("tmp.webm")
        }
    };
    let _ = fs::remove_file(&tmp_path);

    #[cfg(target_os = "windows")]
    let output = cmd
        .args([
            "-y",
            "-i",
            source_path,
            "-vf",
            vf,
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "fastdecode",
            "-b:v",
            "1800k",
            "-maxrate",
            "2000k",
            "-bufsize",
            "4000k",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
            tmp_path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    #[cfg(not(target_os = "windows"))]
    let output = cmd
        .args([
            "-y",
            "-i",
            source_path,
            "-vf",
            vf,
            "-an",  // 无音频
            "-c:v",
            "libvpx", // VP8
            "-b:v",
            "1800k",
            "-maxrate",
            "2000k",
            "-bufsize",
            "4000k",
            "-deadline",
            "realtime",
            "-cpu-used",
            "4",
            "-pix_fmt",
            "yuv420p",
            "-f",
            "webm",
            tmp_path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if output.status.success() && tmp_path.exists() {
        if wait_for_valid_video_artifact(&tmp_path) {
            let _ = fs::remove_file(&preview_path);
            let _ = fs::rename(&tmp_path, &preview_path);
            if ensure_valid_video_artifact_with_wait(&preview_path) {
                return Ok(preview_path.to_string_lossy().to_string());
            }
        } else {
            log::warn!(
                "生成视频预览：tmp 校验失败 id={} tmp={}",
                id,
                tmp_path.to_string_lossy()
            );
            let _ = fs::remove_file(&tmp_path);
        }
    }

    let _ = fs::remove_file(&tmp_path);

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "ffmpeg failed without output".to_string()
    };

    Err(format!("status={} detail={}", output.status, detail))
}

pub fn get_index_path() -> PathBuf {
    get_database_dir().join("library.json")
}

fn get_legacy_index_path() -> PathBuf {
    get_data_dir().join("library").join("index.json")
}

pub fn initialize_library() -> Result<(), String> {
    ensure_dirs()?;

    // 兼容一次性迁移旧路径：data/library/index.json -> data/database/library.json
    let new_path = get_index_path();
    let old_path = get_legacy_index_path();
    if !new_path.exists() && old_path.exists() {
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
        }
        match fs::rename(&old_path, &new_path) {
            Ok(_) => log::info!("迁移库索引：{:?} -> {:?}", old_path, new_path),
            Err(e) => {
                log::warn!("重命名旧索引失败，改用复制：{}", e);
                fs::copy(&old_path, &new_path)
                    .map_err(|e| format!("Failed to copy index to new location: {}", e))?;
            }
        }
    }

    let mut index = load_index()?;
    let mut changed = false;

    let resources_dir = get_default_wallpapers_dir();
    if !resources_dir.exists() {
        return Ok(());
    }

    let image_formats = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "ico", "svg"];
    let video_formats = ["mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v"];

    let mut found_resource_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Ok(entries) = fs::read_dir(&resources_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // 忽略 Windows 快捷方式等非资源文件
            if path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .eq_ignore_ascii_case("lnk")
            {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let is_image = image_formats.contains(&ext.as_str());
            let is_video = video_formats.contains(&ext.as_str());
            if !is_image && !is_video {
                continue;
            }

            let title = title_for_path(&path);
            let file_size = fs::metadata(&path).map(|m| m.len()).ok();
            let resolution = if is_image { get_image_resolution(&path) } else { None };
            let local_path = path.to_string_lossy().to_string();
            let is_video = is_video_path(&local_path);

            // 先检查该文件路径是否已被导入（使用随机 UUID 的条目）
            // 如果已存在，跳过，避免重复添加 res_* 条目
            if index.wallpapers.iter().any(|w| w.local_path == local_path) {
                continue;
            }

            let id = resource_id_for_path(&path);
            found_resource_ids.insert(id.clone());

            // 通过 res_* ID 查找，确保默认壁纸能正确加载
            match index.wallpapers.iter_mut().find(|w| w.id == id) {
                Some(w) => {
                    if w.local_path != local_path {
                        w.local_path = local_path;
                        changed = true;
                    }
                    if w.title != title {
                        w.title = title;
                        changed = true;
                    }
                    if w.file_size != file_size {
                        w.file_size = file_size;
                        changed = true;
                    }
                    if w.resolution != resolution {
                        w.resolution = resolution;
                        changed = true;
                    }
                }
                None => {
                    index.wallpapers.push(Wallpaper {
                        id,
                        title,
                        local_path: local_path.clone(),
                        thumbnail_path: None,
                        resolution: if is_video {
                            None
                        } else {
                            get_image_resolution(&PathBuf::from(&local_path))
                        },
                        file_size: fs::metadata(&local_path).map(|m| m.len()).ok(),
                        metadata: extract_media_metadata(&local_path),
                        import_time: "内置".to_string(),
                    });
                    changed = true;
                }
            }
        }
    }

    // 清理：只清理资源库(res_*)中已不存在的文件，用户导入的不要动
    let before = index.wallpapers.len();
    index.wallpapers.retain(|w| {
        if w.id.starts_with("res_") {
            return found_resource_ids.contains(&w.id) && Path::new(&w.local_path).exists();
        }
        true
    });
    if index.wallpapers.len() != before {
        changed = true;
    }

    if changed {
        save_index(&index)?;
    }

    Ok(())
}

pub fn get_settings_path() -> PathBuf {
    get_database_dir().join("settings.json")
}

pub fn ensure_default_settings_file() -> Result<(), String> {
    let path = get_settings_path();
    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            // empty file -> rewrite
        } else if serde_json::from_str::<Settings>(&content).is_ok() {
            // valid -> keep
            return Ok(());
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
    }
    let content = serde_json::to_string_pretty(&Settings::default())
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, &content).map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}

pub fn ensure_dirs() -> Result<(), String> {
    let dirs = vec![
        get_data_dir(),
        get_data_dir().join("wallpapers"),
        get_data_dir().join("translations"),
        get_database_dir(),
        get_library_dir(),
        get_thumbnails_dir(),
        get_previews_dir(),
        get_plays_dir(),
        get_data_dir().join("downloads"),
        get_data_dir().join("cache"),
        get_data_dir().join("logs"),
    ];

    for dir in dirs {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir {:?}: {}", dir, e))?;
    }
    Ok(())
}

pub fn load_index() -> Result<LibraryIndex, String> {
    let path = get_index_path();
    if !path.exists() {
        return Ok(LibraryIndex::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read index: {}", e))?;

    // 容错：文件存在但内容为空 / JSON 损坏时，按空库处理，避免启动初始化失败
    let trimmed = content.trim();
    if trimmed.is_empty() {
        log::warn!("library.json 为空，回退为默认索引：{:?}", path);
        return Ok(LibraryIndex::default());
    }

    match serde_json::from_str(&content) {
        Ok(v) => Ok(v),
        Err(e) => {
            log::warn!(
                "解析 library.json 失败，回退为默认索引：path={:?}, err={}",
                path,
                e
            );
            Ok(LibraryIndex::default())
        }
    }
}

pub fn save_index(index: &LibraryIndex) -> Result<(), String> {
    let path = get_index_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create dir: {}", e))?;
    }

    let tmp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Failed to serialize index: {}", e))?;

    fs::write(&tmp_path, &content)
        .map_err(|e| format!("Failed to write tmp file: {}", e))?;

    fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Failed to rename file: {}", e))?;

    Ok(())
}

pub fn import_file(source_path: &str) -> Result<Wallpaper, String> {
    let source = PathBuf::from(source_path);

    if !source.exists() {
        return Err(format!("File not found: {}", source_path));
    }

    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let image_formats = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "ico", "svg"];
    let video_formats = ["mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v"];

    let is_image = image_formats.contains(&ext.as_str());
    let is_video = video_formats.contains(&ext.as_str());

    if !is_image && !is_video {
        return Err(format!("Unsupported format: {}", ext));
    }

    let id = Uuid::new_v4().to_string();
    let source_stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");

    let dest_dir = get_default_wallpapers_dir();
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create wallpapers dir: {}", e))?;

    let safe_stem = sanitize_file_stem(source_stem);

    let ts_tail4 = (Local::now().timestamp_millis().unsigned_abs() % 10_000) as u32;
    let ts_tail4 = format!("{:04}", ts_tail4);

    let mut dest_path = dest_dir.join(format!("{}-{}.{}", safe_stem, ts_tail4, ext));
    if dest_path.exists() {
        let mut i = 1u32;
        loop {
            let candidate = dest_dir.join(format!("{}-{}-{}.{}", safe_stem, ts_tail4, i, ext));
            if !candidate.exists() {
                dest_path = candidate;
                break;
            }
            i += 1;
        }
    }

    let title = dest_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string();

    fs::copy(&source, &dest_path)
        .map_err(|e| format!("Failed to copy file: {}", e))?;

    let file_size = fs::metadata(&dest_path)
        .map(|m| m.len())
        .ok();

    let mut resolution = if is_image { get_image_resolution(&dest_path) } else { None };
    let metadata = extract_media_metadata(dest_path.to_string_lossy().as_ref());

    if resolution.is_none() {
        if let Some(m) = metadata.as_ref() {
            if let (Some(w), Some(h)) = (m.width, m.height) {
                resolution = Some(format!("{}×{}", w, h));
            }
        }
    }

    let wallpaper = Wallpaper {
        id,
        title,
        local_path: dest_path.to_string_lossy().to_string(),
        thumbnail_path: None,
        resolution,
        file_size,
        metadata,
        import_time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    Ok(wallpaper)
}

fn get_image_resolution(path: &PathBuf) -> Option<String> {
    image::image_dimensions(path)
        .map(|(w, h)| format!("{}×{}", w, h))
        .ok()
}

pub fn generate_thumbnail(wallpaper: &Wallpaper) -> Result<String, String> {
    let source = PathBuf::from(&wallpaper.local_path);
    let thumb_path = get_thumbnail_path_for_id(&wallpaper.id);

    if is_video_path(&wallpaper.local_path) {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            return generate_video_preview(&wallpaper.id, &wallpaper.local_path);
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            // Non-Linux: don't generate previews; use original video as thumbnail.
            return Ok(wallpaper.local_path.clone());
        }
    }

    if thumb_path.exists() {
        return Ok(thumb_path.to_string_lossy().to_string());
    }

    if let Some(parent) = thumb_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create dir: {}", e))?;
    }

    let img = ImageReader::open(&source)
        .map_err(|e| format!("Failed to open image: {}", e))?
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess image format: {}", e))?
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let thumb = img.thumbnail(320, 180);

    thumb.save(&thumb_path)
        .map_err(|e| format!("Failed to save thumbnail: {}", e))?;

    Ok(thumb_path.to_string_lossy().to_string())
}

pub fn scan_default_wallpapers() -> Vec<Wallpaper> {
    let default_dir = get_default_wallpapers_dir();
    log::info!("扫描默认壁纸目录：{:?}", default_dir);
    let mut wallpapers = Vec::new();

    if !default_dir.exists() {
        log::warn!("默认壁纸目录不存在：{:?}", default_dir);
        return wallpapers;
    }

    let image_formats = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "ico", "svg"];
    let video_formats = ["mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v"];

    if let Ok(entries) = fs::read_dir(&default_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let is_image = image_formats.contains(&ext.as_str());
            let is_video = video_formats.contains(&ext.as_str());

            if !is_image && !is_video {
                continue;
            }

            let title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string();

            let file_size = fs::metadata(&path).map(|m| m.len()).ok();
            let resolution = if is_image { get_image_resolution(&path) } else { None };

            let id = format!("default_{}", title.replace(' ', "_").to_lowercase());

            let wallpaper = Wallpaper {
                id,
                title,
                local_path: path.to_string_lossy().to_string(),
                thumbnail_path: None,
                resolution,
                file_size,
                metadata: extract_media_metadata(path.to_string_lossy().as_ref()),
                import_time: "内置".to_string(),
            };

            wallpapers.push(wallpaper);
        }
    }

    log::info!("扫描到默认壁纸数量：{}", wallpapers.len());
    wallpapers
}

pub fn load_all_wallpapers() -> Result<Vec<Wallpaper>, String> {
    let mut index = load_index()?;

    // 兼容：如果索引为空但资源目录存在（比如用户删了 json），仍然补齐资源条目
    if index.wallpapers.is_empty() {
        let _ = initialize_library();
        index = load_index()?;
    }

    // 对齐 + 生成：thumbnail_path 按 id 对齐。
    // 若 thumbnail_path 缺失/无效，则尽量生成缩略图/预览并写回索引（避免列表长期为 null）。
    let mut changed = false;
    for w in index.wallpapers.iter_mut() {
        // 补齐 metadata / resolution
        let need_meta = w.metadata.is_none();
        let need_res = w.resolution.is_none();
        if (need_meta || need_res) && Path::new(&w.local_path).exists() {
            if need_meta {
                w.metadata = extract_media_metadata(&w.local_path);
                if w.metadata.is_some() {
                    changed = true;
                }
            }
            if need_res {
                if let Some(m) = w.metadata.as_ref() {
                    if let (Some(wd), Some(hd)) = (m.width, m.height) {
                        w.resolution = Some(format!("{}×{}", wd, hd));
                        changed = true;
                    }
                }
            }
        }

        let current = w.thumbnail_path.clone().unwrap_or_default();
        let current_ok = if current.is_empty() {
            false
        } else if is_video_path(&w.local_path) {
            ensure_valid_video_artifact(Path::new(&current))
        } else {
            Path::new(&current).exists()
        };
        if current_ok {
            continue;
        }

        // 先按 id 推导检查是否已存在
        if is_video_path(&w.local_path) {
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            {
                let expected = get_video_preview_path_for_id(&w.id);
                if ensure_valid_video_artifact(&expected) {
                    w.thumbnail_path = Some(expected.to_string_lossy().to_string());
                    changed = true;
                    continue;
                }
            }

            #[cfg(not(any(target_os = "linux", target_os = "windows")))]
            {
                // Non-Linux: video previews are not generated; use original video as thumbnail.
                w.thumbnail_path = Some(w.local_path.clone());
                changed = true;
                continue;
            }
        } else {
            let expected = get_thumbnail_path_for_id(&w.id);
            if expected.exists() {
                w.thumbnail_path = Some(expected.to_string_lossy().to_string());
                changed = true;
                continue;
            }
        }

        // 再尝试生成
        if Path::new(&w.local_path).exists() {
            match generate_thumbnail(w) {
                Ok(p) => {
                    w.thumbnail_path = Some(p);
                    changed = true;
                }
                Err(e) => {
                    log::warn!(
                        "生成缩略图失败：id={}, local_path={}, err={}",
                        w.id,
                        w.local_path,
                        e
                    );
                }
            }
        } else {
            log::warn!(
                "跳过生成缩略图（本地文件不存在）：id={}, local_path={}",
                w.id,
                w.local_path
            );
        }
    }

    if changed {
        save_index(&index)?;
    }

    index.wallpapers.sort_by(|a, b| b.import_time.cmp(&a.import_time));
    Ok(index.wallpapers)
}
