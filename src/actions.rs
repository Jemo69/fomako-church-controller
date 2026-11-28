//! One-line actions: build VISCA for the selected camera, send async, log.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::discover::{self, ScanEvent, ScanHandle};
use crate::follow;
#[cfg(not(target_arch = "wasm32"))]
use crate::follow::{FollowEvent, FollowSpec};
use crate::model::{App, ScanState};
#[cfg(target_arch = "wasm32")]
use crate::model::ActionReply;
use crate::visca::{self, PtDir};

fn hex_str(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn relay_proto(app: &App) -> (&'static str, u16) {
    match app.current().protocol {
        crate::model::ProtoSer::SonyUdp => ("sony_udp", 52381),
        crate::model::ProtoSer::Tcp => ("tcp", 5678),
    }
}

fn send(app: &mut App, what: &str, payload: Vec<u8>) {
    let cam = app.current().clone();
    let hex = visca::hex(&payload);
    if app.use_relay && !app.relay_url.text.trim().is_empty() {
        let relay = app.relay_url.text.trim().to_string();
        let (proto, port) = relay_proto(app);
        let ip = cam.ip.clone();
        let name = cam.name.clone();
        let what = what.to_string();
        let status = format!("{} → {} ({} via relay): {}", name, what, ip, hex);
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let r = discover::relay_post_visca(&relay, &ip, port, proto, &hex_str(&payload));
            if let Err(e) = r {
                eprintln!("[relay] {} -> {}: {}", what, ip, e);
            }
        });
        #[cfg(target_arch = "wasm32")]
        {
            let hex_payload = hex_str(&payload);
            let slot = app.action_slot.clone();
            let wake = app.action_waker.clone();
            let reply_what = what.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = discover::relay_post_visca_async(&relay, &ip, port, proto, &hex_payload).await;
                if let Ok(mut s) = slot.lock() {
                    *s = Some(ActionReply {
                        what: reply_what,
                        result,
                    });
                }
                if let Some(w) = wake {
                    w();
                }
            });
        }
        app.set_status(status);
        return;
    }
    #[cfg(target_arch = "wasm32")]
    {
        app.set_status(format!(
            "{} → {} ({}): browser can't open camera sockets — set a relay URL below",
            cam.name, what, cam.ip
        ));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let port = cam.protocol.port();
        visca::send_async(cam.ip.clone(), port, cam.protocol.to_protocol(), payload);
        app.set_status(format!("{} → {} ({}): {}", cam.name, what, cam.ip, hex));
    }
}

pub fn drive(app: &mut App, dir: PtDir, label: &str) {
    let cam = app.current().clone();
    let p = visca::pan_tilt_drive(cam.address, app.pan_speed, app.tilt_speed, dir);
    send(app, label, p);
}
pub fn stop(app: &mut App) {
    drive(app, PtDir::Stop, "Stop");
}
pub fn home(app: &mut App) {
    drive(app, PtDir::Home, "Home");
}
pub fn zoom(app: &mut App, tele: bool) {
    let cam = app.current().clone();
    let p = if tele {
        visca::zoom_tele_var(cam.address, app.zoom_speed)
    } else {
        visca::zoom_wide_var(cam.address, app.zoom_speed)
    };
    send(app, if tele { "Zoom In" } else { "Zoom Out" }, p);
}
pub fn zoom_stop(app: &mut App) {
    let a = app.current().address;
    send(app, "Zoom Stop", visca::zoom_stop(a));
}
pub fn focus(app: &mut App, far: bool) {
    let a = app.current().address;
    send(app, if far { "Focus Far" } else { "Focus Near" }, if far { visca::focus_far(a) } else { visca::focus_near(a) });
}
pub fn focus_stop(app: &mut App) {
    let a = app.current().address;
    send(app, "Focus Stop", visca::focus_stop(a));
}
pub fn focus_mode(app: &mut App, mode: u8) {
    let a = app.current().address;
    let (p, l) = match mode {
        0 => (visca::focus_auto(a), "Focus Auto"),
        2 => (visca::focus_one_push(a), "Focus One-Push"),
        _ => (visca::focus_manual(a), "Focus Manual"),
    };
    send(app, l, p);
}
pub fn power(app: &mut App, on: bool) {
    let a = app.current().address;
    send(app, if on { "Power ON" } else { "Standby" }, visca::power(a, on));
}
pub fn preset(app: &mut App, n: u8, op: u8) {
    let a = app.current().address;
    let (p, l) = match op {
        1 => (visca::preset_set(a, n), format!("Preset {} Set", n)),
        2 => (visca::preset_clear(a, n), format!("Preset {} Clear", n)),
        _ => (visca::preset_recall(a, n), format!("Preset {} Recall", n)),
    };
    send(app, &l, p);
}

/// Turn the camera's built-in AI tracking on or off (manual §5.2 "Tracking").
pub fn apply_tracking(app: &mut App) {
    let a = app.current().address;
    let on = app.track_on;
    let area = app.track_area;
    app.track_status = if on {
        if area {
            "Area tracking ON — camera follows movement inside the configured area".to_string()
        } else {
            "Real-time tracking ON — camera follows the selected person".to_string()
        }
    } else {
        "Off — camera aims manually".to_string()
    };
    send(
        app,
        &format!("AI Tracking {}", if on { "ON" } else { "OFF" }),
        visca::tracking(a, on, area),
    );
}

pub fn toggle_tracking(app: &mut App) {
    app.track_on = !app.track_on;
    apply_tracking(app);
}

/// Switch mode and turn tracking on (mode only applies while tracking runs).
pub fn set_tracking_mode(app: &mut App, area: bool) {
    app.track_area = area;
    app.track_on = true;
    apply_tracking(app);
}

/// Switch to the next person (F4 on the IR remote; not in the VISCA manual,
/// so this only stops and restarts tracking, which makes the camera re-acquire).
pub fn tracking_next_target(app: &mut App) {
    if !app.track_on {
        app.set_status("AI Tracking: turn tracking on first".into());
        return;
    }
    let a = app.current().address;
    let area = app.track_area;
    send(app, "AI Tracking re-acquire", visca::tracking(a, false, area));
    send(app, "AI Tracking re-acquire", visca::tracking(a, true, area));
    app.track_status = "Re-acquiring target…".to_string();
}

pub fn apply_ae(app: &mut App) {
    let a = app.current().address;
    send(app, &format!("Exposure {}", app.ae_label()), visca::ae_mode(a, app.ae_byte()));
}
pub fn apply_shutter(app: &mut App) {
    let a = app.current().address;
    send(app, "Shutter", visca::shutter_direct(a, app.shutter));
}
pub fn apply_iris(app: &mut App) {
    let a = app.current().address;
    send(app, "Iris", visca::iris_direct(a, app.iris));
}
pub fn apply_gain_limit(app: &mut App) {
    let a = app.current().address;
    send(app, "Gain Limit", visca::gain_limit(a, app.gain_limit));
}
pub fn apply_bright_lvl(app: &mut App) {
    let a = app.current().address;
    send(app, "Bright Level", visca::bright_direct(a, app.bright_lvl));
}
pub fn apply_expcomp(app: &mut App) {
    let a = app.current().address;
    let on = app.expcomp_on;
    send(app, "ExpComp On/Off", visca::expcomp_onoff(a, on));
    if on {
        let lvl = app.expcomp_lvl;
        send(app, "ExpComp Level", visca::expcomp_direct(a, lvl));
    }
}
pub fn apply_backlight(app: &mut App) {
    let a = app.current().address;
    let on = app.backlight;
    send(app, "Backlight", visca::backlight(a, on));
}
pub fn apply_wdr(app: &mut App) {
    let a = app.current().address;
    send(app, "WDR", visca::wdr(a, app.wdr));
}
pub fn apply_nr2d(app: &mut App) {
    let a = app.current().address;
    send(app, "NR 2D", visca::nr2d(a, app.nr2d));
}
pub fn apply_nr3d(app: &mut App) {
    let a = app.current().address;
    send(app, "NR 3D", visca::nr3d(a, app.nr3d));
}
pub fn apply_flicker(app: &mut App) {
    let a = app.current().address;
    send(app, "Flicker", visca::flicker(a, app.flicker));
}
pub fn apply_gamma(app: &mut App) {
    let a = app.current().address;
    send(app, "Gamma", visca::gamma(a, app.gamma));
}
pub fn apply_lowlight(app: &mut App) {
    let a = app.current().address;
    send(app, "Low-Light", visca::lowlight(a, app.lowlight));
}
pub fn apply_aperture(app: &mut App) {
    let a = app.current().address;
    send(app, "Sharpness/Aperture", visca::aperture(a, app.aperture));
}
pub fn apply_brightness(app: &mut App) {
    let a = app.current().address;
    send(app, "Brightness", visca::brightness_direct(a, app.brightness));
}
pub fn apply_contrast(app: &mut App) {
    let a = app.current().address;
    send(app, "Contrast", visca::contrast_direct(a, app.contrast));
}
pub fn apply_saturation(app: &mut App) {
    let a = app.current().address;
    send(app, "Saturation", visca::saturation(a, app.saturation));
}
pub fn apply_wb(app: &mut App) {
    let a = app.current().address;
    let b = app.wb_byte();
    send(app, &format!("WB {}", app.wb_label()), visca::wb_mode(a, b));
}
pub fn apply_awb_sens(app: &mut App) {
    let a = app.current().address;
    send(app, "AWB Sensitivity", visca::awb_sensitivity(a, app.awb_sens));
}
pub fn apply_r_gain(app: &mut App) {
    let a = app.current().address;
    send(app, "R Gain", visca::rgain(a, app.r_gain));
}
pub fn apply_b_gain(app: &mut App) {
    let a = app.current().address;
    send(app, "B Gain", visca::bgain(a, app.b_gain));
}
pub fn apply_flip(app: &mut App) {
    let a = app.current().address;
    send(app, "Flip", visca::flip(a, app.flip));
}
pub fn apply_bw(app: &mut App) {
    let a = app.current().address;
    send(app, "B/W", visca::picture_bw(a, app.bw));
}

/// Start (or stop) a LAN scan for cameras.
/// Native: raw sockets. With relay set: via relay (also works on web/phones).
/// Needs `tx` to schedule the poll loop that drains results.
pub fn start_scan(app: &mut App, tx: &mut rune::Tx) {
    // Stop a running scan.
    if let Some(scan) = app.scan.as_ref() {
        if scan.running {
            scan.handle.abort.store(true, Ordering::SeqCst);
            if let Some(scan) = app.scan.as_mut() {
                scan.message = "Stopping scan…".to_string();
            }
            tx.wake_after(std::time::Duration::from_millis(100));
            return;
        }
    }
    let raw = app.scan_prefix.text.trim().replace('/', " ");
    let mut prefix = raw.split_whitespace().next().unwrap_or("").to_string();
    // Accept "192.168.1.0" too, but never mangle a bare "x.y.0" subnet.
    {
        let parts: Vec<&str> = prefix.split('.').collect();
        if parts.len() == 4 && parts[3] == "0" {
            prefix = parts[..3].join(".");
        }
    }
    if crate::discover::prefix_of(&format!("{}.1", prefix)).is_none() {
        app.set_status("Scan: enter subnet as 3 numbers, e.g. 192.168.1".into());
        return;
    }
    let relay = if app.use_relay && !app.relay_url.text.trim().is_empty() {
        Some(app.relay_url.text.trim().to_string())
    } else {
        None
    };
    #[cfg(target_arch = "wasm32")]
    if relay.is_none() {
        app.set_status(
            "Scan: browsers can't probe the network — set a relay URL (see below) and scan via relay."
                .into(),
        );
        return;
    }
    let queue: Arc<Mutex<Vec<ScanEvent>>> = Arc::new(Mutex::new(vec![]));
    let handle = Arc::new(ScanHandle {
        done: AtomicBool::new(false),
        abort: AtomicBool::new(false),
        checked: AtomicUsize::new(0),
        total: AtomicUsize::new(254),
    });
    // Rendered before any worker reports.
    handle.total.store(254, Ordering::SeqCst);
    app.scan = Some(ScanState {
        running: true,
        via_relay: relay.is_some(),
        prefix: prefix.clone(),
        done: 0,
        total: 254,
        results: vec![],
        message: if relay.is_some() {
            format!("Scanning {}.1-254 via relay…", prefix)
        } else {
            format!("Scanning {}.1-254…", prefix)
        },
        queue: queue.clone(),
        handle: handle.clone(),
    });
    app.push_log(format!("Scan started on {}.0/24", prefix));
    if let Some(relay) = relay {
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            match discover::relay_get_scan(&relay, &prefix) {
                Ok(found) => {
                    if let Ok(mut q) = queue.lock() {
                        for f in found {
                            q.push(ScanEvent::Found(f));
                        }
                    }
                }
                Err(e) => eprintln!("[relay scan] {}", e),
            }
            handle.checked.store(handle.total.load(Ordering::SeqCst), Ordering::SeqCst);
            handle.done.store(true, Ordering::SeqCst);
        });
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            match discover::relay_get_scan_async(&relay, &prefix).await {
                Ok(found) => {
                    if let Ok(mut q) = queue.lock() {
                        for f in found {
                            q.push(ScanEvent::Found(f));
                        }
                    }
                }
                Err(e) => eprintln!("[relay scan] {}", e),
            }
            handle.checked.store(handle.total.load(Ordering::SeqCst), Ordering::SeqCst);
            handle.done.store(true, Ordering::SeqCst);
        });
    } else {
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            discover::scan_subnet(&prefix, queue, handle);
        });
    }
    tx.wake_after(std::time::Duration::from_millis(100));
}

/// Map photo-style -100..+100 controls to the camera's documented ranges.
/// Saturation 4 is 100%; 7 would incorrectly apply 130% at the neutral position.
pub fn picture_edit_targets(bright: i32, contrast: i32, sat: i32, sharp: i32) -> (u8, u8, u8, u8) {
    let b = ((7.0 + bright as f32 * 0.07).round() as i32).clamp(0, 14) as u8;
    let c = ((7.0 + contrast as f32 * 0.07).round() as i32).clamp(0, 14) as u8;
    let s = ((4.0 + sat as f32 * 0.10).round() as i32).clamp(0, 14) as u8;
    let ap = ((8.0 + sharp as f32 * 0.07).round() as i32).clamp(0, 15) as u8;
    (b, c, s, ap)
}

/// Apply picture-edit sliders to the real camera image pipeline.
pub fn apply_picture_edit(app: &mut App) {    let (b, c, s, ap) = picture_edit_targets(app.pic_bright, app.pic_contrast, app.pic_sat, app.pic_sharp);
    app.brightness = b;
    app.contrast = c;
    app.saturation = s;
    app.aperture = ap;
    let a = app.current().address;
    send(app, &format!("Picture Brightness {}", b), visca::brightness_direct(a, b));
    send(app, &format!("Picture Contrast {}", c), visca::contrast_direct(a, c));
    send(app, &format!("Picture Saturation {}", s), visca::saturation(a, s));
    send(app, &format!("Picture Sharpness {}", ap), visca::aperture(a, ap));
}

/// Adopt a reference picture: measure its look and remember it as the target.
pub fn set_look_reference(app: &mut App, label: String, bytes: Vec<u8>) {
    match crate::look::analyze_bytes(&bytes) {
        Ok(stats) => {
            if let Some(cache) = app.preview_cache.as_ref() {
                app.look_image = Some(cache.load(bytes.clone()));
            }
            app.look_bytes = Some(bytes.into());
            app.look_target = Some(stats);
            app.look_label = label;
            app.look_auto = false;
            app.look_status = format!("Reference — {}", crate::look::describe(&stats));
            app.set_status(format!("Match picture loaded: {}", app.look_label.clone()));
        }
        Err(e) => app.set_status(format!("Match picture: {e}")),
    }
}

/// Keep the Picture Edit sliders in step with values the matcher wrote.
fn sync_picture_sliders(app: &mut App) {
    app.pic_bright = (((app.brightness as f32 - 7.0) / 0.07).round() as i32).clamp(-100, 100);
    app.pic_contrast = (((app.contrast as f32 - 7.0) / 0.07).round() as i32).clamp(-100, 100);
    app.pic_sat = (((app.saturation as f32 - 4.0) / 0.10).round() as i32).clamp(-100, 100);
    app.pic_sharp = (((app.aperture as f32 - 8.0) / 0.07).round() as i32).clamp(-100, 100);
}

/// One closed-loop step toward the reference look. True when settings moved.
pub fn match_look(app: &mut App) -> bool {
    let Some(target) = app.look_target else {
        app.set_status("Match picture: load a picture first".into());
        return false;
    };
    let Some(live) = app.look_live else {
        app.set_status("Match picture: waiting for a live frame".into());
        return false;
    };
    let a = app.current().address;
    let cur = crate::look::Settings {
        brightness: app.brightness,
        contrast: app.contrast,
        saturation: app.saturation,
        aperture: app.aperture,
    };
    let (next, mut changed) = crate::look::nudge(&target, &live, cur);
    let mut parts: Vec<String> = Vec::new();
    if next.brightness != cur.brightness {
        app.brightness = next.brightness;
        send(app, "Look brightness", visca::brightness_direct(a, next.brightness));
        parts.push(format!("brightness {}", next.brightness));
    }
    if next.contrast != cur.contrast {
        app.contrast = next.contrast;
        send(app, "Look contrast", visca::contrast_direct(a, next.contrast));
        parts.push(format!("contrast {}", next.contrast));
    }
    if next.saturation != cur.saturation {
        app.saturation = next.saturation;
        send(app, "Look saturation", visca::saturation(a, next.saturation));
        parts.push(format!("saturation {}", next.saturation));
    }
    if next.aperture != cur.aperture {
        app.aperture = next.aperture;
        send(app, "Look sharpness", visca::aperture(a, next.aperture));
        parts.push(format!("sharpness {}", next.aperture));
    }
    if app.look_color {
        if app.wb_mode != 5 {
            app.wb_mode = 5;
            send(app, "Look WB Manual", visca::wb_mode(a, 5));
            parts.push("WB manual".to_string());
            changed = true;
        }
        let (r, b, colour_changed) = crate::look::nudge_color(target.warmth, live.warmth, app.r_gain, app.b_gain);
        if colour_changed {
            app.r_gain = r;
            app.b_gain = b;
            send(app, "Look R gain", visca::rgain(a, r));
            send(app, "Look B gain", visca::bgain(a, b));
            parts.push(format!("colour R{r} B{b}"));
            changed = true;
        }
    }
    if changed {
        sync_picture_sliders(app);
        app.look_status = format!("Adjusted {}", parts.join(", "));
        app.set_status(format!("Match picture → {}", app.look_status.clone()));
    } else {
        app.look_status = "Live look already matches the reference".to_string();
    }
    changed
}

pub fn toggle_look_auto(app: &mut App) {
    if app.look_auto {
        app.look_auto = false;
        app.set_status("Auto match OFF — camera settings stay where they are".into());
        return;
    }
    if app.look_target.is_none() {
        app.set_status("Auto match: load a reference picture first".into());
        return;
    }
    app.look_auto = true;
    app.look_next_at = 0.0;
    app.set_status("Auto match ON — the app keeps tuning brightness, contrast, saturation and sharpness".into());
}

/// Load a picture from disk (the looks folder buttons or the path field).
#[cfg(not(target_arch = "wasm32"))]
pub fn load_look_file(app: &mut App, path: &str) {
    let label = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    match std::fs::read(path) {
        Ok(bytes) => set_look_reference(app, label, bytes),
        Err(e) => app.set_status(format!("Match picture: {path}: {e}")),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn load_look_file(app: &mut App, _path: &str) {
    app.set_status("Match picture: loading pictures needs the desktop app".into());
}

#[cfg(not(target_arch = "wasm32"))]
pub fn refresh_look_files(app: &mut App) {
    app.look_files = crate::look::list_pictures("looks");
    app.set_status(format!(
        "Match picture: {} picture(s) in ./looks{}",
        app.look_files.len(),
        if app.look_files.is_empty() {
            " — drop image files there and press Refresh"
        } else {
            ""
        }
    ));
}

#[cfg(target_arch = "wasm32")]
pub fn refresh_look_files(_app: &mut App) {}

pub fn clear_look(app: &mut App) {
    app.look_target = None;
    app.look_live = None;
    app.look_label.clear();
    app.look_image = None;
    app.look_bytes = None;
    app.look_auto = false;
    app.look_status = "Load a picture and the app tunes the camera to match it".to_string();
    app.set_status("Match picture reference cleared (camera unchanged)".into());
}

/// Called from the app poll: while Auto match is on, take a step every ~2.5 s.
pub fn poll_look(app: &mut App, clock: f64) -> bool {
    if !app.look_auto || app.look_target.is_none() || clock < app.look_next_at {
        return false;
    }
    app.look_next_at = clock + 2.5;
    if app.look_live.is_none() {
        let msg = "Auto match: waiting for a live frame".to_string();
        if app.look_status != msg {
            app.look_status = msg;
            return true;
        }
        return false;
    }
    match_look(app);
    true
}

// ---------------------------------------------------------------- cross-camera follow

fn follow_pair(app: &App) -> Option<(usize, usize)> {
    if app.cameras.len() < 2 {
        return None;
    }
    let l = app.follow_leader.min(app.cameras.len() - 1);
    let f = app.follow_follower.min(app.cameras.len() - 1);
    (l != f && app.cameras[l].ip != app.cameras[f].ip).then_some((l, f))
}

pub fn follow_stop(app: &mut App) {
    app.follow_gen.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut slot) = app.follow_slot.lock() {
        slot.clear();
    }
    if app.follow_on {
        app.follow_status = "Stopped — leader aim is no longer sent to the follower.".to_string();
    }
    app.follow_on = false;
}

pub fn follow_toggle(app: &mut App) {
    if app.follow_on {
        follow_stop(app);
    } else {
        follow_start(app);
    }
}

fn follow_start(app: &mut App) {
    let Some((l, f)) = follow_pair(app) else {
        app.follow_status = "Pick two different cameras (different IPs) first.".to_string();
        return;
    };
    if !app.follow_ready {
        app.follow_status =
            "Calibrate both cameras on one shared landmark first (Start uses the saved offset).".to_string();
        return;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (l, f);
        app.follow_status = "Cross-camera follow needs the desktop app (camera sockets).".to_string();
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if app.selected == f && app.track_on {
            app.track_on = false;
            app.track_status = "Off — driven by cross-camera follow".to_string();
        }
        let leader = app.cameras[l].clone();
        let follower = app.cameras[f].clone();
        let spec = FollowSpec {
            leader: leader.clone(),
            follower: follower.clone(),
            pan_offset: app.follow_pan_offset,
            tilt_offset: app.follow_tilt_offset,
            pan_speed: app.pan_speed,
            tilt_speed: app.tilt_speed,
        };
        app.follow_gen.fetch_add(1, Ordering::SeqCst);
        let gen = app.follow_gen.load(Ordering::SeqCst);
        follow::start(
            spec,
            gen,
            app.follow_gen.clone(),
            app.follow_slot.clone(),
            app.follow_waker.clone(),
        );
        app.follow_on = true;
        app.follow_status = follow::following_note(&leader.name, &follower.name);
        if let Err(error) = follow::save_pair(
            &app.cameras,
            l,
            f,
            app.follow_pan_offset,
            app.follow_tilt_offset,
        ) {
            app.push_log(format!("Follow offset not saved: {}", error));
        }
    }
}

/// Point both cameras at one landmark first, then calibrate.
pub fn follow_calibrate(app: &mut App) {
    follow_stop(app);
    let Some((l, f)) = follow_pair(app) else {
        app.follow_status = "Pick two different cameras (different IPs) first.".to_string();
        return;
    };
    app.follow_ready = false;
    app.follow_status = "Calibrating — keep both cameras on the shared landmark…".to_string();
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (l, f);
        app.follow_status = "Cross-camera follow needs the desktop app (camera sockets).".to_string();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if app.selected == f && app.track_on {
            app.track_on = false;
            app.track_status = "Off — driven by cross-camera follow".to_string();
        }
        let leader = app.cameras[l].clone();
        let follower = app.cameras[f].clone();
        let slot = app.follow_slot.clone();
        let wake = app.follow_waker.clone();
        let gen = app.follow_gen.load(Ordering::SeqCst);
        // Two camera round-trips can take seconds on an unreachable TCP camera;
        // keep the UI thread free and report through the follow slot.
        std::thread::spawn(move || {
            let event = match follow::calibrate(&leader, &follower) {
                Ok((pan, tilt)) => FollowEvent::Calibrated { gen, pan, tilt },
                Err(msg) => FollowEvent::CalibrationError { gen, msg },
            };
            if let Ok(mut s) = slot.lock() {
                s.push(event);
            }
            if let Some(w) = wake {
                w();
            }
        });
    }
}

pub fn follow_adjust(app: &mut App, pan_delta: i32, tilt_delta: i32) {
    if !app.follow_ready {
        app.follow_status =
            "Calibrate both cameras on one shared landmark before nudging the offset.".to_string();
        return;
    }
    let was_running = app.follow_on;
    app.follow_pan_offset = (app.follow_pan_offset + pan_delta).clamp(-follow::OFFSET_MAX, follow::OFFSET_MAX);
    app.follow_tilt_offset = (app.follow_tilt_offset + tilt_delta).clamp(-follow::OFFSET_MAX, follow::OFFSET_MAX);
    if was_running {
        // Restart the loop so the new offset takes effect now.
        follow_stop(app);
        follow_start(app);
    } else {
        app.follow_status = format!(
            "Offset pan {:+}, tilt {:+} steps — fine-tune while following.",
            app.follow_pan_offset, app.follow_tilt_offset
        );
        if let Some((l, f)) = follow_pair(app) {
            if let Err(error) = follow::save_pair(
                &app.cameras,
                l,
                f,
                app.follow_pan_offset,
                app.follow_tilt_offset,
            ) {
                app.push_log(format!("Follow offset not saved: {}", error));
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn poll_action_replies(app: &mut App) -> bool {
    let reply = app
        .action_slot
        .lock()
        .map(|mut slot| slot.take())
        .unwrap_or(None);
    let Some(reply) = reply else {
        return false;
    };
    match reply.result {
        Ok(msg) => app.set_status(format!("{}: {}", reply.what, msg)),
        Err(error) => app.set_status(format!("{} FAILED: {}", reply.what, error)),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::picture_edit_targets;

    #[test]
    fn neutral_picture_edit_is_actually_neutral() {
        assert_eq!(picture_edit_targets(0, 0, 0, 0), (7, 7, 4, 8));
    }

    #[test]
    fn picture_edit_clamps_to_camera_ranges() {
        assert_eq!(picture_edit_targets(-100, -100, -100, -100), (0, 0, 0, 1));
        assert_eq!(picture_edit_targets(100, 100, 100, 100), (14, 14, 14, 15));
    }

    #[test]
    fn reference_picture_is_measured_and_disarms_auto_match() {
        let mut app = crate::model::App::new();
        let img = image::RgbImage::from_fn(24, 24, |_, _| image::Rgb([12, 200, 40]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("encode png");
        super::set_look_reference(&mut app, "green.png".into(), bytes.into_inner());
        let stats = app.look_target.expect("reference measured");
        assert!(stats.saturation > 0.5, "{}", stats.saturation);
        assert!(stats.warmth < 0.0);
        assert!(!app.look_auto, "a new reference does not start auto match");
        assert_eq!(app.look_label, "green.png");
    }
}
