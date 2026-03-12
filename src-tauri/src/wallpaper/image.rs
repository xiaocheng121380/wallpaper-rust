#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "windows")]
use crate::library;

#[cfg(target_os = "windows")]
use std::path::Path;

#[cfg(target_os = "windows")]
use uuid::Uuid;

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
};

pub fn apply_image_wallpaper(path: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // Try GNOME/GTK first
        // Also set for dark mode
        // Try feh for other DEs
        return crate::platform::linux::apply_image_wallpaper(path);
    }

    #[cfg(target_os = "windows")]
    {
        let src = Path::new(path);
        if !src.exists() {
            return Err(format!("File not found: {}", path));
        }

        fn spi_set_wallpaper(p: &str) -> bool {
            let mut wide: Vec<u16> = p.encode_utf16().collect();
            wide.push(0);
            unsafe {
                SystemParametersInfoW(
                    SPI_SETDESKWALLPAPER,
                    0,
                    wide.as_ptr() as *mut _,
                    SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
                ) != 0
            }
        }

        // Fast path: try using the original image path directly.
        // Modern Windows versions accept JPG/PNG paths, and this avoids expensive conversions.
        if spi_set_wallpaper(path) {
            return Ok(());
        }

        // Windows SPI_SETDESKWALLPAPER historically expects BMP for best compatibility.
        // We always convert to a managed BMP path under data/ to avoid relying on registry
        // behavior for non-BMP formats.
        let out_dir = library::get_data_dir().join("wallpaper_applied");
        std::fs::create_dir_all(&out_dir)
            .map_err(|e| format!("Failed to create dir {:?}: {}", out_dir, e))?;

        // Cache per source path: convert only once per image.
        let key = Uuid::new_v5(&Uuid::NAMESPACE_URL, path.as_bytes()).to_string();
        let out_bmp = out_dir.join(format!("{}.bmp", key));

        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !out_bmp.exists() {
            if ext == "bmp" {
                std::fs::copy(src, &out_bmp)
                    .map_err(|e| format!("Failed to copy bmp to {:?}: {}", out_bmp, e))?;
            } else {
                let img = image::ImageReader::open(src)
                    .map_err(|e| format!("Failed to open image: {}", e))?
                    .with_guessed_format()
                    .map_err(|e| format!("Failed to guess image format: {}", e))?
                    .decode()
                    .map_err(|e| format!("Failed to decode image: {}", e))?;
                img.save(&out_bmp)
                    .map_err(|e| format!("Failed to save bmp {:?}: {}", out_bmp, e))?;
            }
        }

        let bmp_str = out_bmp
            .to_str()
            .ok_or_else(|| "Invalid bmp path".to_string())?;

        if !spi_set_wallpaper(bmp_str) {
            return Err("SystemParametersInfoW(SPI_SETDESKWALLPAPER) failed".to_string());
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"tell application "Finder" to set desktop picture to POSIX file "{}""#,
            path
        );

        let result = Command::new("osascript")
            .args(["-e", &script])
            .output();

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to set wallpaper on macOS: {}", e)),
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err("Unsupported platform".to_string())
    }
}
