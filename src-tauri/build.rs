fn main() {
    tauri_build::build();

    println!("cargo:rerun-if-changed=native/ffplay_wallpaper.cpp");

    #[cfg(target_os = "windows")]
    {
        cc::Build::new()
            .cpp(true)
            .file("native/ffplay_wallpaper.cpp")
            .flag_if_supported("/std:c++17")
            .compile("ffplay_wallpaper");
    }
}
