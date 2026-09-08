mod audio;

/// Compact UTC timestamp for the panic log (no chrono dependency).
fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    fmt_utc(secs)
}

/// Format unix seconds as YYYY-MM-DD HH:MM:SS UTC.
pub fn fmt_utc(secs: u64) -> String {
    let secs = secs;
    let days = secs / 86_400;
    let rem = secs % 86_400;
    // Civil-from-days algorithm (Howard Hinnant) for a readable date.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60
    )
}
mod commands;
mod content;
mod display;
mod intelligence;
mod session;
mod stt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Native crashes (audio drivers, whisper) bypass Rust's panic hook;
    // install a Windows exception filter that logs them too.
    crate::stt::install_crash_handler();

    // Never die silently: persist any panic so a machine that flashes and
    // closes leaves a trail in %APPDATA%\BibleLive\panic.log.
    std::panic::set_hook(Box::new(|info| {
        let msg = format!(
            "[{}] {}
",
            chrono_like_timestamp(),
            info
        );
        eprintln!("PANIC: {msg}");
        if let Ok(appdata) = std::env::var("APPDATA") {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("{appdata}/BibleLive/panic.log"))
            {
                let _ = f.write_all(msg.as_bytes());
            }
        }
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin({
            use tauri_plugin_global_shortcut::{Builder, ShortcutState};
            Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    // Plugin normalizes to lowercase "control+alt+x".
                    let combo = shortcut.into_string().to_lowercase();
                    use tauri::Manager;
                    let mgr = app.state::<crate::display::DisplayManager>();
                    let active = mgr.active_display();
                    match combo.as_str() {
                        "control+alt+1" | "control+alt+2" | "control+alt+3"
                        | "control+alt+4" | "control+alt+5" | "control+alt+digit1"
                        | "control+alt+digit2" | "control+alt+digit3"
                        | "control+alt+digit4" | "control+alt+digit5"
                        | "control+alt+numpad1" | "control+alt+numpad2"
                        | "control+alt+numpad3" | "control+alt+numpad4"
                        | "control+alt+numpad5" => {
                            let digits = combo
                                .trim_start_matches("control+alt+")
                                .trim_start_matches("digit")
                                .trim_start_matches("numpad");
                            let slot: u8 = digits.parse().unwrap_or(1);
                            mgr.set_active_display(slot);
                            crate::display::emit_all_slots(app, &mgr);
                        }
                        "control+alt+right" | "control+alt+arrowright" => {
                            mgr.step(active, 1);
                            crate::display::emit_slot(app, active, &mgr);
                        }
                        "control+alt+left" | "control+alt+arrowleft" => {
                            mgr.step(active, -1);
                            crate::display::emit_slot(app, active, &mgr);
                        }
                        "control+alt+b" => {
                            let blank = !mgr.slots()[(active - 1) as usize].blank;
                            mgr.set_blank(active, blank);
                            crate::display::emit_slot(app, active, &mgr);
                        }
                        _ => {
                            eprintln!("[hotkey] unmatched: {combo}");
                        }
                    }
                })
                .build()
        })
        .manage(content::ContentStore::open().expect("failed to open content database"))
        .manage(std::sync::Arc::new(session::ServiceState::new()))
        .manage(audio::CaptureManager::new())
        .manage(commands::SttHolder::default())
        .manage(display::DisplayManager::new())
        .setup(|app| {
            use tauri::Manager;
            use tauri_plugin_global_shortcut::GlobalShortcutExt;

            // Per-slot display styles persisted in the settings table.
            {
                let store = app.state::<crate::content::ContentStore>();
                app.state::<crate::display::DisplayManager>()
                    .load_styles(&store);
            }

            // Global hotkeys — work even when the operator window is not
            // focused (e.g. the display output has focus during a service).
            // Non-fatal: if any combo is already claimed by other software,
            // register what we can and keep running — a hotkey conflict must
            // never stop the app from starting.
            match app.global_shortcut().register_multiple([
                "Ctrl+Alt+1",
                "Ctrl+Alt+2",
                "Ctrl+Alt+3",
                "Ctrl+Alt+4",
                "Ctrl+Alt+5",
                "Ctrl+Alt+Numpad1",
                "Ctrl+Alt+Numpad2",
                "Ctrl+Alt+Numpad3",
                "Ctrl+Alt+Numpad4",
                "Ctrl+Alt+Numpad5",
                "Ctrl+Alt+Right",
                "Ctrl+Alt+Left",
                "Ctrl+Alt+B",
            ]) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[startup] global hotkeys unavailable: {e}");
                    let _ = app.global_shortcut().register_multiple([
                        "Ctrl+Alt+Right",
                        "Ctrl+Alt+Left",
                        "Ctrl+Alt+B",
                    ]);
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            use tauri::Manager;
            // Closing the operator window closes the whole app — display
            // outputs are children of this session and must never outlive it.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    window.app_handle().exit(0);
                }
            }
            // A display output closing (Esc, or programmatically) refreshes
            // the operator's slot card.
            if let tauri::WindowEvent::Destroyed = event {
                if window.label().starts_with("display-") {
                    let app = window.app_handle();
                    let slot: u8 = window
                        .label()
                        .trim_start_matches("display-")
                        .parse()
                        .unwrap_or(0);
                    if (1..=5).contains(&slot) {
                        let mgr = app.state::<crate::display::DisplayManager>();
                        crate::display::emit_slot(app, slot, &mgr);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::app_status,
            commands::library_stats,
            commands::list_content,
            commands::get_content,
            commands::create_content,
            commands::update_content,
            commands::delete_content,
            commands::search_content,
            commands::import_text_content,
            commands::import_file_content,
            commands::list_audio_devices,
            commands::get_voice_config,
            commands::set_voice_config,
            commands::get_listen_mode,
            commands::set_listen_mode,
            commands::listening_status,
            commands::start_listening,
            commands::stop_listening,
            commands::audio_test,
            commands::list_suggestions,
            commands::respond_suggestion,
            commands::model_status,
            commands::run_voice_diagnostics,
            commands::list_monitors,
            commands::get_display_slots,
            commands::set_slot_monitor,
            commands::set_slot_mode,
            commands::set_slot_blank,
            commands::open_slot_output,
            commands::close_slot_output,
            commands::set_slot_scripture,
            commands::set_slot_section,
            commands::set_slot_media,
            commands::slot_step,
            commands::set_slot_style,
            commands::set_active_display,
            commands::move_slot_output_to_next_monitor,
            commands::cycle_output_focus,
            commands::start_service_session,
            commands::end_service_session,
            commands::current_service_session,
            commands::list_service_sessions,
            commands::get_service_session_items,
            commands::delete_service_session,
            commands::set_all_displays_blank,
            commands::list_display_profiles,
            commands::save_display_profile,
            commands::apply_display_profile,
            commands::delete_display_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running BibleLive");
}
