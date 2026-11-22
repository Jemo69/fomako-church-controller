//! Wasm entrypoint for the browser build (phone + desktop browsers).
//! Same views as the native app; camera traffic goes through the LAN
//! relay (`scripts/visca-relay.py`), since browsers can't open raw sockets.

#[path = "actions.rs"]
pub mod actions;
#[path = "discover.rs"]
pub mod discover;
#[path = "model.rs"]
pub mod model;
#[path = "preview.rs"]
pub mod preview;
#[path = "view.rs"]
pub mod view;
#[path = "visca.rs"]
pub mod visca;

#[cfg(target_arch = "wasm32")]
use model::App;
#[cfg(target_arch = "wasm32")]
use rune::{AppSpec, TextEdit};

/// Percent-decode a URL-hash value (no new deps).
#[cfg(target_arch = "wasm32")]
fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.as_bytes().iter();
    while let Some(&b) = it.next() {
        if b == b'%' {
            let mut h = || it.next().copied().unwrap_or(b'0');
            let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push((hex(h()) << 4 | hex(h())) as char);
        } else if b == b'+' {
            out.push(' ');
        } else {
            out.push(b as char);
        }
    }
    out
}

/// Booth QR codes link straight into a working setup, e.g.
/// `http://pc:8080/#relay=http%3A%2F%2Fpc%3A8080&subnet=192.168.1`.
#[cfg(target_arch = "wasm32")]
fn init_from_url() -> App {
    let mut s = App::new();
    let hash = web_sys::window()
        .and_then(|w| w.location().hash().ok())
        .unwrap_or_default();
    // user=/pass= may come after cam=; hold them until the loop ends.
    let mut pending_user: Option<String> = None;
    let mut pending_pass: Option<String> = None;
    let mut pending_cam: Option<String> = None;
    for kv in hash.trim_start_matches('#').split('&') {
        let mut it = kv.splitn(2, '=');
        match (it.next(), it.next()) {
            (Some("relay"), Some(v)) => {
                let v = url_decode(v);
                if !v.is_empty() {
                    s.relay_url = TextEdit::new(v);
                    s.use_relay = true;
                }
            }
            (Some("subnet"), Some(v)) => {
                if crate::discover::prefix_of(&format!("{}.1", v)).is_some() {
                    s.scan_prefix = TextEdit::new(v.to_string());
                }
            }
            (Some("autostart"), Some("scan")) => {
                s.autostart_scan = true;
            }
            (Some("cam"), Some(v)) => {
                // Booth QR can pre-configure everything, e.g.
                // #relay=http%3A%2F%2Fpc%3A8080&cam=192.168.5.163&user=admin&pass=secret&subnet=192.168.5&autostart=scan
                let v = url_decode(v);
                if crate::discover::prefix_of(&format!("{}.1", v)).is_some()
                    && !s.cameras.iter().any(|c| c.ip == v)
                {
                    pending_cam = Some(v);
                }
            }
            (Some("user"), Some(v)) => {
                pending_user = Some(url_decode(v));
            }
            (Some("pass"), Some(v)) => {
                pending_pass = Some(url_decode(v));
            }
            _ => {}
        }
    }
    if let Some(ip) = pending_cam {
        let addr = (s.cameras.len() as u8 + 1).min(7);
        s.cameras.push(crate::model::CameraConfig {
            name: format!("Camera {}", ip),
            ip,
            protocol: crate::model::ProtoSer::SonyUdp,
            address: addr,
            username: pending_user.unwrap_or_else(|| "admin".into()),
            password: pending_pass.unwrap_or_else(|| "admin".into()),
        });
        s.selected = s.cameras.len() - 1;
    }
    s
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    rune::run(AppSpec {
        scenes: &[
            ("control", |s: &mut App| s.page = model::Page::Control),
            ("image", |s: &mut App| s.page = model::Page::Image),
            ("picture", |s: &mut App| s.page = model::Page::Picture),
            ("cameras", |s: &mut App| s.page = model::Page::Cameras),
            ("help", |s: &mut App| s.page = model::Page::Help),
            ("scan", |s: &mut App| s.page = model::Page::Cameras),
        ],
        setup: Some(setup),
        poll: Some(poll),
        ..rune::AppSpec::new(
            "FoMaKo Church Controller",
            (390.0, 760.0),
            init_from_url,
            view::view,
        )
    });
}

#[cfg(target_arch = "wasm32")]
fn setup(s: &mut App, waker: rune::Waker) {
    s.preview_cache = Some(rune::ImageCache::new(waker.clone()));
    s.preview_waker = Some(waker);
}

#[cfg(target_arch = "wasm32")]
fn poll(s: &mut App, clock: f64, tx: &mut rune::Tx) -> bool {
    if s.autostart_scan {
        s.autostart_scan = false;
        s.page = model::Page::Cameras;
        actions::start_scan(s, tx);
    }
    let mut changed = s.scan_poll(tx);
    changed |= preview::poll(s, clock, tx);
    changed
}
