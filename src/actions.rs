//! One-line actions: build VISCA for the selected camera, send async, log.

use crate::model::App;
use crate::visca::{self, PtDir};

fn send(app: &mut App, what: &str, payload: Vec<u8>) {
    let cam = app.current().clone();
    let hex = visca::hex(&payload);
    let port = cam.protocol.port();
    visca::send_async(cam.ip.clone(), port, cam.protocol.to_protocol(), payload);
    app.set_status(format!("{} → {} ({}): {}", cam.name, what, cam.ip, hex));
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
    // op: 0 recall, 1 set, 2 clear
    let a = app.current().address;
    let (p, l) = match op {
        1 => (visca::preset_set(a, n), format!("Preset {} Set", n)),
        2 => (visca::preset_clear(a, n), format!("Preset {} Clear", n)),
        _ => (visca::preset_recall(a, n), format!("Preset {} Recall", n)),
    };
    send(app, &l, p);
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
pub fn apply_nr(app: &mut App) {
    let a = app.current().address;
    let (d2, d3) = (app.nr2d, app.nr3d);
    send(app, "NR 2D", visca::nr2d(a, d2));
    send(app, "NR 3D", visca::nr3d(a, d3));
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
pub fn apply_gains(app: &mut App) {
    let a = app.current().address;
    let (r, b) = (app.r_gain, app.b_gain);
    send(app, "R Gain", visca::rgain(a, r));
    send(app, "B Gain", visca::bgain(a, b));
}
pub fn apply_flip(app: &mut App) {
    let a = app.current().address;
    send(app, "Flip", visca::flip(a, app.flip));
}
pub fn apply_bw(app: &mut App) {
    let a = app.current().address;
    send(app, "B/W", visca::picture_bw(a, app.bw));
}

/// Apply picture-edit sliders to the real camera image pipeline.
/// Maps -100..+100 preview sliders onto camera ranges, then sends.
pub fn apply_picture_edit(app: &mut App) {
    // Map preview -100..100 to camera ranges:
    // brightness 0..14 (center 7), contrast 0..14, saturation 0..14, aperture 0..15
    let b = ((7.0 + app.pic_bright as f32 * 0.07).round() as i32).clamp(0, 14) as u8;
    let c = ((7.0 + app.pic_contrast as f32 * 0.07).round() as i32).clamp(0, 14) as u8;
    let s = ((7.0 + app.pic_sat as f32 * 0.07).round() as i32).clamp(0, 14) as u8;
    let ap = ((8.0 + app.pic_sharp as f32 * 0.07).round() as i32).clamp(0, 15) as u8;
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
