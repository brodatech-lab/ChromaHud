mod metrics;
mod tray;

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use metrics::fps::{FpsTracker, PRESENTMON_ARGS};

pub const OVERLAY_LABEL: &str = "overlay";
pub const SETTINGS_LABEL: &str = "settings";

/// Handle to the PresentMon sidecar so it can be killed when the app exits.
struct PresentMonProcess(Mutex<Option<CommandChild>>);

pub fn toggle_overlay(app: &AppHandle) {
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let visible = overlay.is_visible().unwrap_or(false);
        let _ = if visible { overlay.hide() } else { overlay.show() };
    }
}

pub fn show_settings(app: &AppHandle) {
    if let Some(settings) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = settings.unminimize();
        let _ = settings.show();
        let _ = settings.set_focus();
    }
}

/// The per-core panel lives in the frontend settings; the overlay flips and persists it.
pub fn toggle_cpu_cores(app: &AppHandle) {
    let _ = app.emit_to(OVERLAY_LABEL, "toggle-cpu-cores", ());
}

#[tauri::command]
fn set_keep_game(tracker: tauri::State<'_, Arc<FpsTracker>>, enabled: bool) {
    tracker.set_keep_target(enabled);
}

/// Stretches the overlay over the primary monitor and makes it ignore all mouse input.
fn setup_overlay(app: &AppHandle) -> tauri::Result<()> {
    let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) else {
        return Ok(());
    };

    if let Some(monitor) = overlay.primary_monitor()? {
        let position = monitor.position();
        let size = monitor.size();
        overlay.set_position(PhysicalPosition::new(position.x, position.y))?;
        overlay.set_size(PhysicalSize::new(size.width, size.height))?;
    }

    overlay.set_ignore_cursor_events(true)?;
    overlay.set_always_on_top(true)?;
    overlay.show()?;
    Ok(())
}

fn start_presentmon(app: &AppHandle, tracker: Arc<FpsTracker>) {
    let spawned = app
        .shell()
        .sidecar("presentmon")
        .map(|command| command.args(PRESENTMON_ARGS))
        .and_then(|command| command.spawn());

    let (mut events, child) = match spawned {
        Ok(spawned) => spawned,
        Err(error) => {
            eprintln!("ChromaHUD: failed to start PresentMon, FPS disabled: {error}");
            return;
        }
    };

    *app.state::<PresentMonProcess>().0.lock().unwrap() = Some(child);

    tauri::async_runtime::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                CommandEvent::Stdout(line) => tracker.ingest_line(&String::from_utf8_lossy(&line)),
                CommandEvent::Stderr(line) => {
                    eprintln!("PresentMon: {}", String::from_utf8_lossy(&line).trim_end())
                }
                CommandEvent::Terminated(status) => {
                    tracker.mark_stopped();
                    eprintln!("PresentMon exited: {:?}", status.code);
                    break;
                }
                _ => {}
            }
        }
    });
}

fn shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let toggle = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyH);
    let settings = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyO);
    let cores = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyC);

    tauri_plugin_global_shortcut::Builder::new()
        .with_shortcuts([toggle, settings, cores])
        .expect("valid global shortcuts")
        .with_handler(move |app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if shortcut == &toggle {
                toggle_overlay(app);
            } else if shortcut == &settings {
                show_settings(app);
            } else if shortcut == &cores {
                toggle_cpu_cores(app);
            }
        })
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(position) = args.iter().position(|a| a == "--dump-metrics") {
        let samples = args.get(position + 1).and_then(|n| n.parse().ok()).unwrap_or(5);
        metrics::dump(samples);
        return;
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(shortcut_plugin())
        .manage(PresentMonProcess(Mutex::new(None)))
        .manage(metrics::SystemInfoState::default())
        .manage(Arc::new(FpsTracker::default()))
        .invoke_handler(tauri::generate_handler![
            metrics::get_system_info,
            tray::set_cpu_cores_checked,
            set_keep_game
        ])
        .setup(|app| {
            let handle = app.handle();
            setup_overlay(handle)?;
            tray::create(handle)?;

            let tracker = app.state::<Arc<FpsTracker>>().inner().clone();
            start_presentmon(handle, tracker.clone());
            metrics::spawn_collector(handle.clone(), tracker);
            Ok(())
        })
        .on_window_event(|window, event| {
            // The settings window is reused: closing it only hides it.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == SETTINGS_LABEL {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building ChromaHUD");

    app.run(|app, event| match event {
        // Keep running in the tray when no window is visible.
        RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(child) = app.state::<PresentMonProcess>().0.lock().unwrap().take() {
                let _ = child.kill();
            }
        }
        _ => {}
    });
}
