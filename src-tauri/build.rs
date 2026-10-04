fn main() {
    // PresentMon (ETW) and WMI thermal zones need elevation, so release builds request it via UAC.
    // Debug builds keep the default manifest so `tauri dev` can launch the exe from any terminal;
    // run that terminal as administrator to get FPS and CPU temperature during development.
    let mut attrs = tauri_build::Attributes::new();
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let windows =
            tauri_build::WindowsAttributes::new().app_manifest(include_str!("chromahud.manifest"));
        attrs = attrs.windows_attributes(windows);
    }
    println!("cargo:rerun-if-changed=chromahud.manifest");
    tauri_build::try_build(attrs).expect("failed to run build script");
}
