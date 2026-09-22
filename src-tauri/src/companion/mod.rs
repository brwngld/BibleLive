//! Phone companion — a browser-first remote over the church LAN.
//!
//! The desktop app serves a small mobile page (no app store, nothing to
//! install): the operator's phone opens `http://<pc-ip>:<port>/` on the
//! same Wi-Fi and gets the display strip, suggestion SHOW/IGNORE, blank
//! controls and the announcement box. Access is gated by a 4-digit PIN
//! shown next to the URL in the app. v1 is remote control only — the
//! phone-microphone audio path is a later increment.

use crate::commands::{self, NotifyPayload};
use crate::content::ContentStore;
use crate::display::{self, DisplayManager};
use crate::session::ServiceState;
use serde::Serialize;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// How long the server waits between polls for a stop request.
const TICK: Duration = Duration::from_millis(80);
const PORTS: [u16; 6] = [8787, 8788, 8789, 8790, 8791, 8792];

/// Managed handle: which server (if any) is running and how to stop it.
#[derive(Default)]
pub struct CompanionHandle {
    stop: parking_lot::Mutex<Option<Arc<AtomicBool>>>,
}

impl CompanionHandle {
    pub fn is_running(&self) -> bool {
        self.stop.lock().is_some()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionInfo {
    pub running: bool,
    pub url: Option<String>,
    pub pin: Option<String>,
}

/// Best-effort LAN IP: ask the OS which local interface would route to the
/// internet (no packet is actually sent for UDP "connect").
fn lan_ip() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let addr = sock.local_addr().ok()?;
    Some(addr.ip().to_string())
}

fn pin_from_store(store: &ContentStore) -> String {
    if let Ok(Some(saved)) = store.get_setting("companion_pin") {
        if saved.len() == 4 && saved.chars().all(|c| c.is_ascii_digit()) {
            return saved;
        }
    }
    // 4 digits from the clock — good enough for a LAN-only gate that the
    // operator hands out personally.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 + d.as_secs())
        .unwrap_or(0);
    let pin = format!("{:04}", nanos % 10_000);
    let _ = store.set_setting("companion_pin", &pin);
    pin
}

/// Start (or restart) the companion server on its own thread.
pub fn start(app: &AppHandle, handle: &CompanionHandle, store: &ContentStore) -> Result<CompanionInfo, String> {
    stop(handle);
    let pin = pin_from_store(store);
    let stop = Arc::new(AtomicBool::new(false));

    let mut server = None;
    let mut port = 0u16;
    for p in PORTS {
        match tiny_http::Server::http(("0.0.0.0", p)) {
            Ok(s) => {
                server = Some(s);
                port = p;
                break;
            }
            Err(_) => continue,
        }
    }
    let Some(server) = server else {
        return Err("no free port in 8787-8792 for the companion server".into());
    };
    let ip = lan_ip().unwrap_or_else(|| "127.0.0.1".into());
    let url = format!("http://{ip}:{port}/");

    let app_thread = app.clone();
    let stop_thread = stop.clone();
    let pin_thread = pin.clone();
    std::thread::spawn(move || {
        serve(server, app_thread, stop_thread, pin_thread);
    });

    *handle.stop.lock() = Some(stop);
    Ok(CompanionInfo { running: true, url: Some(url), pin: Some(pin) })
}

pub fn stop(handle: &CompanionHandle) {
    if let Some(flag) = handle.stop.lock().take() {
        flag.store(true, Ordering::Relaxed);
    }
}

fn authorized(url: &str, pin: &str) -> bool {
    url.splitn(2, '?')
        .nth(1)
        .map(|q| q.split('&').any(|kv| kv == format!("pin={pin}")))
        .unwrap_or(false)
}

fn serve(server: tiny_http::Server, app: AppHandle, stop: Arc<AtomicBool>, pin: String) {
    while !stop.load(Ordering::Relaxed) {
        match server.recv_timeout(TICK) {
            Ok(Some(request)) => {
                handle_request(request, &app, &pin);
            }
            Ok(None) => {} // timeout tick — re-check stop flag
            Err(e) => {
                eprintln!("[companion] recv error: {e}");
            }
        }
    }
    println!("[companion] stopped");
}

fn handle_request(mut request: tiny_http::Request, app: &AppHandle, pin: &str) {
    let url = request.url().to_string();
    let method = request.method().clone();
    let path = url.split('?').next().unwrap_or("/").to_string();

    // The page itself is public (it only asks for the PIN); data and
    // commands require the pin query parameter.
    if !(method == tiny_http::Method::Get && path == "/") && !authorized(&url, pin) {
        let _ = request.respond(
            tiny_http::Response::from_string(r#"{"ok":false,"error":"bad pin"}"#)
                .with_status_code(403)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap(),
                ),
        );
        return;
    }

    match (&method, path.as_str()) {
        (tiny_http::Method::Get, "/") => {
            let _ = request.respond(
                tiny_http::Response::from_string(include_str!("page.html")).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                        .unwrap(),
                ),
            );
        }
        (tiny_http::Method::Get, "/state") => {
            let json = snapshot(app);
            respond_json(request, &json);
        }
        (tiny_http::Method::Post, "/cmd") => {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let reply = run_command(app, &body);
            respond_json(request, &reply);
        }
        _ => {
            let _ = request.respond(tiny_http::Response::empty(404));
        }
    }
}

fn respond_json(request: tiny_http::Request, json: &str) {
    let _ = request.respond(
        tiny_http::Response::from_string(json.to_string()).with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        ),
    );
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateSnapshot<'a> {
    ok: bool,
    slots: &'a [display::SlotView],
    suggestions: Vec<crate::intelligence::Suggestion>,
}

fn snapshot(app: &AppHandle) -> String {
    let slots = app.state::<DisplayManager>().slots();
    let service = app.state::<Arc<ServiceState>>();
    let snap = StateSnapshot {
        ok: true,
        slots: &slots,
        suggestions: service
            .pending_suggestions()
            .into_iter()
            .filter(|s| s.status == "pending")
            .collect(),
    };
    serde_json::to_string(&snap).unwrap_or_else(|_| r#"{"ok":false}"#.into())
}

/// Execute one remote command. Kept intentionally small: the same safety
/// rules as the desktop UI apply (LOCK is respected by the engine itself).
fn run_command(app: &AppHandle, body: &str) -> String {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Cmd {
        action: String,
        slot: Option<u8>,
        delta: Option<i32>,
        #[serde(default)]
        value: Option<serde_json::Value>,
        id: Option<String>,
        text: Option<String>,
        duration_ms: Option<u64>,
    }
    let Ok(cmd) = serde_json::from_str::<Cmd>(body) else {
        return r#"{"ok":false,"error":"bad command"}"#.into();
    };
    let mgr = app.state::<DisplayManager>();
    let service = app.state::<Arc<ServiceState>>();

    match cmd.action.as_str() {
        "step" => {
            let Some(slot) = cmd.slot else { return err("slot required") };
            if (1..=5).contains(&slot) {
                if mgr.step(slot, cmd.delta.unwrap_or(1)) {
                    display::emit_slot(app, slot, mgr.inner());
                }
                ok()
            } else {
                err("slot out of range")
            }
        }
        "blank" => {
            let Some(slot) = cmd.slot else { return err("slot required") };
            let blank = cmd.value.and_then(|v| v.as_bool()).unwrap_or(true);
            if (1..=5).contains(&slot) {
                mgr.set_blank(slot, blank);
                display::emit_slot(app, slot, mgr.inner());
                ok()
            } else {
                err("slot out of range")
            }
        }
        "blankAll" => {
            let blank = cmd.value.and_then(|v| v.as_bool()).unwrap_or(true);
            for slot in 1..=5u8 {
                mgr.set_blank(slot, blank);
                display::emit_slot(app, slot, mgr.inner());
            }
            ok()
        }
        "setActive" => {
            let Some(slot) = cmd.slot else { return err("slot required") };
            if (1..=5).contains(&slot) {
                mgr.set_active_display(slot);
                display::emit_all_slots(app, mgr.inner());
                ok()
            } else {
                err("slot out of range")
            }
        }
        "respond" => {
            let Some(id) = cmd.id else { return err("id required") };
            let show = cmd.value.and_then(|v| v.as_bool()).unwrap_or(true);
            if show {
                // Project onto the ACTIVE display, like the desktop SHOW.
                let slot = mgr.active_display();
                let suggestion = service
                    .pending_suggestions()
                    .into_iter()
                    .find(|s| s.id == id);
                let Some(s) = suggestion else { return err("suggestion not found") };
                let store = app.state::<ContentStore>();
                let result = tauri::async_runtime::block_on(commands::project_suggestion(
                    app,
                    mgr.inner(),
                    store.inner(),
                    service.inner(),
                    slot,
                    &s,
                ));
                if let Err(e) = result {
                    return err(&e);
                }
            }
            if let Some(updated) =
                service.resolve_suggestion(&id, if show { "shown" } else { "ignored" })
            {
                commands::emit_suggestion(app, service.inner(), updated);
                ok()
            } else {
                err("suggestion not found")
            }
        }
        "notify" => {
            let Some(slot) = cmd.slot else { return err("slot required") };
            let text = cmd.text.unwrap_or_default();
            if !(1..=5).contains(&slot) {
                return err("slot out of range");
            }
            if text.trim().is_empty() {
                return err("empty announcement");
            }
            let _ = app.emit(
                "display-notify",
                NotifyPayload {
                    slot,
                    id: commands::notify_id(),
                    text: text.trim().to_string(),
                    duration_ms: cmd.duration_ms.unwrap_or(30_000),
                },
            );
            ok()
        }
        _ => err(&format!("unknown action: {}", cmd.action)),
    }
}

fn ok() -> String {
    r#"{"ok":true}"#.into()
}
fn err(msg: &str) -> String {
    format!(r#"{{"ok":false,"error":"{msg}"}}"#)
}

#[cfg(test)]
mod tests {
    use super::authorized;

    /// The PIN gate: any path with the right pin= passes; wrong/missing pin
    /// never does.
    #[test]
    fn pin_gate() {
        assert!(authorized("/state?pin=1234", "1234"));
        assert!(authorized("/cmd?pin=1234&x=1", "1234"));
        assert!(!authorized("/state?pin=9999", "1234"));
        assert!(!authorized("/state", "1234"));
        assert!(!authorized("/state?pin=12345", "1234"), "no prefix matches");
        assert!(!authorized("/state?in=1234", "1234"), "partial keys rejected");
    }
}
