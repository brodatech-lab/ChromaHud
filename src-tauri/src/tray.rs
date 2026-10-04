use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::{show_settings, toggle_cpu_cores, toggle_overlay};

/// Kept so the check mark can follow the persisted `showCpuCores` setting.
struct CpuCoresMenuItem(CheckMenuItem<Wry>);

#[tauri::command]
pub fn set_cpu_cores_checked(app: AppHandle, checked: bool) {
    if let Some(item) = app.try_state::<CpuCoresMenuItem>() {
        let _ = item.0.set_checked(checked);
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show/Hide overlay", true, Some("Ctrl+Shift+H"))?;
    let cores = CheckMenuItem::with_id(app, "cores", "Show CPU cores", true, false, Some("Ctrl+Shift+C"))?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, Some("Ctrl+Shift+O"))?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit ChromaHUD", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &cores, &settings, &separator, &quit])?;
    app.manage(CpuCoresMenuItem(cores));

    let mut builder = TrayIconBuilder::with_id("chromahud")
        .tooltip("ChromaHUD")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_overlay(app),
            "cores" => toggle_cpu_cores(app),
            "settings" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_settings(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
