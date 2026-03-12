use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MediaMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pix_fmt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallpaper {
    pub id: String,
    pub title: String,
    pub local_path: String,
    pub thumbnail_path: Option<String>,
    pub resolution: Option<String>,
    pub file_size: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MediaMetadata>,
    pub import_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverHistory {
    pub schema_version: u32,
    pub urls: Vec<String>,
}

impl Default for DiscoverHistory {
    fn default() -> Self {
        Self {
            schema_version: 1,
            urls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryIndex {
    pub schema_version: u32,
    pub wallpapers: Vec<Wallpaper>,
}

impl Default for LibraryIndex {
    fn default() -> Self {
        Self {
            schema_version: 1,
            wallpapers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    pub language: String,
    pub theme_mode: String,
    pub primary_color: Option<String>,
    #[serde(default)]
    pub video_wallpaper: VideoWallpaperSettings,
    #[serde(default)]
    pub current_wallpaper_id: Option<String>,
    #[serde(default = "default_stop_video_on_exit")]
    pub stop_video_on_exit: bool,
    #[serde(default = "default_minimize_to_tray")]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub first_close_handled: bool,
}

fn default_stop_video_on_exit() -> bool {
    true  // 默认退出时停止视频
}

fn default_minimize_to_tray() -> bool {
    true  // 默认关闭时最小化到托盘
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoWallpaperSettings {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub fps: Option<u32>,
    pub crf: Option<u8>,
    #[serde(default)]
    pub bitrate_kbps: Option<u32>,
    #[serde(default = "default_video_wallpaper_hwdec")]
    pub hwdec: bool,
}

fn default_video_wallpaper_hwdec() -> bool {
    true
}

impl Default for VideoWallpaperSettings {
    fn default() -> Self {
        Self {
            max_width: Some(2560),
            max_height: Some(1440),
            fps: Some(30),
            crf: Some(23),
            bitrate_kbps: None,
            hwdec: default_video_wallpaper_hwdec(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            language: "zh-CN".to_string(),
            theme_mode: "dark".to_string(),
            primary_color: Some("#18a058".to_string()),
            video_wallpaper: VideoWallpaperSettings::default(),
            current_wallpaper_id: None,
            stop_video_on_exit: true,
            minimize_to_tray: true,
            first_close_handled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<CommandError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl<T> CommandResult<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(CommandError {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}
