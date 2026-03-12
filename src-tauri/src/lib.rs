mod commands;
mod library;
mod media_server;
mod models;
mod platform;
mod wallpaper;

use std::path::Path;
use tauri::{Manager, Emitter, menu::{MenuBuilder, MenuItemBuilder}, tray::TrayIconBuilder};
use log::LevelFilter;
use tauri::http::{header, Response, StatusCode};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let level = if cfg!(debug_assertions) {
        LevelFilter::Info
    } else {
        LevelFilter::Warn
    };

    let log_dir = crate::library::get_data_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let log_plugin = tauri_plugin_log::Builder::new()
        .level(level)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Folder {
                path: log_dir,
                file_name: Some("wallcraft".to_string()),
            },
        ))
        .build();

    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }

        // VM / no-GPU friendly defaults: prefer software rendering & decoding.
        // Users can override any of these by setting env vars before launching.
        if std::env::var_os("LIBGL_ALWAYS_SOFTWARE").is_none() {
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        }
        if std::env::var_os("GALLIUM_DRIVER").is_none() {
            std::env::set_var("GALLIUM_DRIVER", "llvmpipe");
        }
        if std::env::var_os("GST_VAAPI_DISABLE").is_none() {
            std::env::set_var("GST_VAAPI_DISABLE", "1");
        }
        if std::env::var_os("GST_GL_DISABLE").is_none() {
            std::env::set_var("GST_GL_DISABLE", "1");
        }
    }

    if let Err(e) = library::ensure_dirs() {
        log::error!("初始化目录失败: {}", e);
    }

    if let Err(e) = library::ensure_default_settings_file() {
        log::warn!("初始化默认设置文件失败: {}", e);
    }

    tauri::Builder::default()
        .plugin(log_plugin)
        .register_uri_scheme_protocol("wallcraft", |_app, request| {
            // URL format (frontend): wallcraft://localhost/<urlencoded-abs-path>
            // We intentionally ignore Range requests and always return 200 with full body
            // to avoid WebKitGTK range/206 quirks on Linux.

            let uri = request.uri();
            let raw_path = uri.path().trim_start_matches('/');
            let decoded = urlencoding::decode(raw_path)
                .map(|s| s.into_owned())
                .unwrap_or_else(|_| raw_path.to_string());

            if decoded.is_empty() {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET,HEAD,OPTIONS")
                    .body(Vec::new())
                    .unwrap();
            }

            if request.method() == "OPTIONS" {
                return Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET,HEAD,OPTIONS")
                    .body(Vec::new())
                    .unwrap();
            }

            let req_path = Path::new(&decoded);
            let data_dir = crate::library::get_data_dir();
            if !req_path.starts_with(&data_dir) {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET,HEAD,OPTIONS")
                    .body(Vec::new())
                    .unwrap();
            }

            if !req_path.exists() {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET,HEAD,OPTIONS")
                    .body(Vec::new())
                    .unwrap();
            }

            let mime = match req_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "webm" => "video/webm",
                "mp4" => "video/mp4",
                "mkv" => "video/x-matroska",
                "mov" => "video/quicktime",
                "avi" => "video/x-msvideo",
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "webp" => "image/webp",
                "gif" => "image/gif",
                _ => "application/octet-stream",
            };

            let meta_len = std::fs::metadata(req_path).map(|m| m.len()).unwrap_or(0);

            if request.method() == "HEAD" {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime)
                    .header(header::CONTENT_LENGTH, meta_len)
                    .header(header::ACCEPT_RANGES, "none")
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET,HEAD,OPTIONS")
                    .body(Vec::new())
                    .unwrap();
            }

            match std::fs::read(req_path) {
                Ok(bytes) => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime)
                    .header(header::CONTENT_LENGTH, bytes.len() as u64)
                    .header(header::ACCEPT_RANGES, "none")
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET,HEAD,OPTIONS")
                    .body(bytes)
                    .unwrap(),
                Err(_) => Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET,HEAD,OPTIONS")
                    .body(Vec::new())
                    .unwrap(),
            }
        })
        .setup(|app| {
            // 创建托盘菜单
            let show_item = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show_item, &quit_item])
                .build()?;

            // 创建托盘图标，使用应用默认图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event {
                        let app_handle = tray.app_handle();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // DevTools 通过环境变量 OPEN_DEVTOOLS=1 控制
            #[cfg(debug_assertions)]
            {
                let open = std::env::var("OPEN_DEVTOOLS")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                if open {
                    if let Some(window) = app.get_webview_window("main") {
                        window.open_devtools();
                    }
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                window.on_webview_event(|event| {
                    log::info!("[WebView事件] {:?}", event);
                });

                // Register cleanup on window close
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        log::info!("[应用] 收到窗口关闭请求");

                        let settings = library::load_settings();

                        // 如果是首次关闭，阻止关闭并触发前端对话框
                        if !settings.first_close_handled {
                            log::info!("[应用] 首次关闭，弹出确认对话框");
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("show-close-dialog", ());
                            }
                            api.prevent_close();
                            return;
                        }

                        // 检查设置：是否最小化到托盘
                        if settings.minimize_to_tray {
                            log::info!("[应用] 根据设置最小化到托盘（不退出）");
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.hide();
                            }
                            api.prevent_close();
                        } else {
                            log::info!("[应用] 退出程序");
                            #[cfg(target_os = "windows")]
                            {
                                // Check settings to see if we should stop video on exit
                                if settings.stop_video_on_exit {
                                    log::info!("[应用] 退出时停止视频壁纸");
                                    if let Err(e) = wallpaper::video::stop_video_wallpaper(&app_handle) {
                                        log::warn!("[应用] 退出时停止视频壁纸失败：{}", e);
                                    }
                                } else {
                                    log::info!("[应用] 退出时保持视频壁纸运行（stop_video_on_exit=false）");
                                }
                            }
                        }
                    }
                });
            }

            std::thread::spawn(|| {
                if let Err(e) = library::initialize_library() {
                    log::warn!("初始化壁纸库失败：{}", e);
                }
                if let Err(e) = library::prewarm_video_previews() {
                    log::warn!("预热视频预览失败：{}", e);
                }
                if let Err(e) = library::prewarm_detail_plays() {
                    log::warn!("预热详情播放缓存失败：{}", e);
                }
            });

            #[cfg(target_os = "linux")]
            media_server::start();
            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 当尝试启动第二个实例时，显示并聚焦主窗口
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::library_list,
            commands::library_get,
            commands::library_import,
            commands::library_remove,
            commands::library_update_title,
            commands::settings_get,
            commands::settings_update,
            commands::cache_get_size,
            commands::cache_clear,
            commands::discover_history_get,
            commands::discover_history_update,
            commands::database_json_get,
            commands::database_json_set,
            commands::download_url_to_downloads,
            commands::system_get_platform,
            commands::system_get_data_dir,
            commands::system_get_log_dir,
            commands::system_get_media_base_url,
            commands::system_open_path,
            commands::system_get_screen_resolution,
            commands::thumbnail_get,
            commands::get_thumbnail_base64,
            commands::get_thumbnail_path,
            commands::get_detail_play_path,
            commands::get_video_base64,
            commands::get_video_bytes,
            wallpaper::wallpaper_apply,
            wallpaper::wallpaper_stop,
            commands::window_handle_first_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
