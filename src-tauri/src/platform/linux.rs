use tauri::AppHandle;

pub fn apply_image_wallpaper(path: &str) -> Result<(), String> {
    // GNOME
    let result = std::process::Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.background",
            "picture-uri",
            &format!("file://{}", path),
        ])
        .output();

    if result.is_ok() {
        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.background",
                "picture-uri-dark",
                &format!("file://{}", path),
            ])
            .output();
        return Ok(());
    }

    let result = std::process::Command::new("feh")
        .args(["--bg-fill", path])
        .output();

    if result.is_ok() {
        return Ok(());
    }

    Err("Failed to set wallpaper on Linux".to_string())
}

pub fn apply_video_wallpaper(_app: &AppHandle, path: &str) -> Result<(), String> {
    let _ = path;
    Err("Video wallpaper is not supported on Linux GNOME Wayland yet".to_string())
}

pub fn stop_video_wallpaper(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}
