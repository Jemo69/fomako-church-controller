//! Rune view for the FoMaKo church controller.

use rune::tok::{self, type_};
use rune::{Cx, El, ScrollSpec, TextStyle, col, el, row, text, text_input};

use crate::actions;
use crate::model::{App, Page};
use crate::visca::PtDir;

fn field_style() -> TextStyle {
    TextStyle::new(13.0, 400.0)
}

#[allow(dead_code)]
fn btn_label(label: &str, t: rune::tok::Theme, w: f32, h: f32) -> El {
    el().key(format!("btn-{}", label)).w(w).h(h).center().radius(8.0)
        .fill(t.card).border(1.0, t.card_border)
        .hover(t.hover).press(t.press).focusable(true)
        .child(text(label.to_owned()).font(type_::BODY).color(t.text).a11y_hidden())
}

fn btn<F>(label: &str, key: &str, t: rune::tok::Theme, w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    el().key(key.to_owned()).w(w).h(h).center().radius(8.0)
        .fill(t.card).border(1.0, t.card_border)
        .hover(t.hover).press(t.press).focusable(true)
        .role("button").label(label.to_owned())
        .on_press(f)
        .child(text(label.to_owned()).font(type_::BODY).color(t.text).a11y_hidden())
}

fn accent_btn<F>(label: &str, key: &str, dark: bool, w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    let a = tok::accent(0, dark);
    el().key(key.to_owned()).w(w).h(h).center().radius(8.0)
        .fill(a).hover(a).press(a).focusable(true)
        .role("button").label(label.to_owned())
        .on_press(f)
        .child(text(label.to_owned()).font(type_::BODY).color(rune::Color::WHITE).a11y_hidden())
}

fn accent_btn_tx<F>(label: &str, key: &str, dark: bool, w: f32, h: f32, f: F) -> El
where F: Fn(&mut App, &mut rune::Tx) + 'static {
    let a = tok::accent(0, dark);
    el().key(key.to_owned()).w(w).h(h).center().radius(8.0)
        .fill(a).hover(a).press(a).focusable(true)
        .role("button").label(label.to_owned())
        .on_press_tx(f)
        .child(text(label.to_owned()).font(type_::BODY).color(rune::Color::WHITE).a11y_hidden())
}

fn section_title(t: rune::tok::Theme, title: &str) -> El {
    text(title.to_owned()).key(format!("sec-{}", title)).font(type_::SECTION).color(t.text_secondary)
}

fn stepper(t: rune::tok::Theme, key: &str, label: &str, value: String, on_minus: impl Fn(&mut App) + 'static, on_plus: impl Fn(&mut App) + 'static) -> El {
    row().key(key.to_owned()).w_full().h(34.0).center_y().gap(8.0)
        .child(text(label.to_owned()).font(type_::BODY).color(t.text).w(150.0))
        .child(btn("−", &format!("{}-minus", key), t, 34.0, 28.0, on_minus))
        .child(text(value).font(type_::MONO_CAPTION).color(t.text).w(90.0))
        .child(btn("+", &format!("{}-plus", key), t, 34.0, 28.0, on_plus))
}

fn apply_row<F>(t: rune::tok::Theme, dark: bool, key: &str, label: &str, value: &str, f: F) -> El
where F: Fn(&mut App) + 'static {
    row().key(key.to_owned()).w_full().h(36.0).center_y().gap(8.0)
        .child(text(label.to_owned()).font(type_::BODY).color(t.text).w(150.0))
        .child(text(value.to_owned()).font(type_::LABEL).color(t.text_secondary).w(130.0))
        .child(accent_btn("Apply", &format!("{}-apply", key), dark, 76.0, 28.0, f))
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = tok::theme(s.dark);
    let dark = s.dark;
    col().key("window").w_full().h_full().fill(t.bg)
        .child(topbar(s, t, dark, cx))
        .child(tabbar(s, t, dark))
        .child(
            col().key("content").grow(1.0).shrink().min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(match s.page {
                    Page::Control => control_page(s, t, dark),
                    Page::Image => image_page(s, t, dark),
                    Page::Picture => picture_page(s, t, dark),
                    Page::Cameras => cameras_page(s, t, dark),
                    Page::Help => help_page(s, t, dark),
                })
        )
        .child(statusbar(s, t))
}

fn topbar(s: &App, t: rune::tok::Theme, dark: bool, _cx: &mut Cx<App>) -> El {
    let cam = s.current();
    let title = format!("FoMaKo Church Control  •  {} @ {}  •  {}", cam.name, cam.ip, cam.protocol.label());
    row().key("topbar").w_full().h(52.0).px(16.0).center_y().gap(12.0).fill(t.card).border(0.0, t.card_border)
        .child(
            col().grow(1.0).shrink()
                .child(text("FoMaKo PTZ — Church Controller".to_owned()).font(type_::TITLE).color(t.text))
                .child(text(title).font(type_::LABEL).color(t.text_secondary))
        )
        .child(btn(if dark { "☾ Dark" } else { "☀ Light" }, "theme", t, 84.0, 30.0, |s: &mut App| s.dark = !s.dark))
}

fn tabbar(s: &App, t: rune::tok::Theme, _dark: bool) -> El {
    let mut r = row().key("tabs").w_full().h(42.0).px(12.0).gap(6.0).center_y().fill(t.bg);
    for p in Page::all() {
        let on = s.page == p;
        let label = p.title().to_owned();
        let key = format!("tab-{}", label);
        r = r.child(
            el().key(key).h(30.0).px(14.0).center().radius(15.0)
                .fill(if on { t.control_track } else { t.bg })
                .border(1.0, if on { t.card_border } else { t.bg })
                .hover(t.hover).press(t.press).focusable(true)
                .role("tab").label(label.clone())
                .on_press(move |s: &mut App| s.page = p)
                .child(text(label).font(type_::BODY).color(t.text))
        );
    }
    r
}

fn statusbar(s: &App, t: rune::tok::Theme) -> El {
    let last = s.log.last().cloned().unwrap_or_default();
    col().key("status").w_full().pt(8.0).pb(10.0).px(16.0).gap(4.0).fill(t.card)
        .child(text(format!("Status: {}", s.status)).font(type_::LABEL).color(t.text))
        .child(text(format!("Log: {}", last)).font(type_::MONO_CAPTION).color(t.text_secondary))
}

// ---------------------------------------------------------------- control

/// Live camera picture (ffmpeg snapshots, or relay snapshots on web).
/// Fixed 16:9 box so framing never distorts.
fn preview_pane(s: &App, t: rune::tok::Theme) -> El {
    let live = s.preview_image.as_ref().and_then(|img| img.size().map(|_| img));
    let mut box_el = el().key("preview-box").w(480.0).h(270.0).center().radius(8.0)
        .fill(t.card).border(1.0, t.card_border);
    if let Some(img) = live {
        box_el = box_el.child(
            el().w_full().h_full().radius(8.0).child(
                el().key("preview-img").w_full().h_full().pixels(img.pixels())
            )
        );
    }
    let mut pane = col().key("preview").w_full().gap(6.0)
        .child(row().w_full().gap(8.0).center_y()
            .child(section_title(t, "Live preview"))
            .child(text(s.preview_status.clone()).font(type_::LABEL).color(t.text_secondary).grow(1.0))
            .child(btn(if s.preview_on { "Pause" } else { "Resume" }, "preview-toggle", t, 80.0, 28.0,
                |s: &mut App| { s.preview_on = !s.preview_on; })))
        .child(row().w_full().center().child(box_el));
    if s.preview_image.is_none() {
        pane = pane.child(text("Pointing the camera? Presets recall the exact framing — this picture is ~1 frame/sec, just for aiming.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary));
    }
    pane
}

fn control_page(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    col().key("control").w_full().px(16.0).py(14.0).gap(14.0)
        .child(camera_switcher(s, t))
        .child(preview_pane(s, t))
        .child(row().w_full().gap(14.0)
            .child(ptz_pad(t))
            .child(col().gap(10.0).w(300.0)
                .child(section_title(t, "Zoom"))
                .child(row().gap(8.0)
                    .child(btn("Wide −", "zoom-out", t, 86.0, 34.0, |s: &mut App| actions::zoom(s, false)))
                    .child(btn("Stop", "zoom-stop", t, 64.0, 34.0, actions::zoom_stop))
                    .child(btn("Tele +", "zoom-in", t, 86.0, 34.0, |s: &mut App| actions::zoom(s, true))))
                .child(section_title(t, "Focus"))
                .child(row().gap(8.0)
                    .child(btn("Near", "focus-near", t, 70.0, 32.0, |s: &mut App| actions::focus(s, false)))
                    .child(btn("Stop", "focus-stop", t, 60.0, 32.0, actions::focus_stop))
                    .child(btn("Far", "focus-far", t, 70.0, 32.0, |s: &mut App| actions::focus(s, true))))
                .child(row().gap(8.0)
                    .child(btn("Auto", "af-auto", t, 70.0, 30.0, |s: &mut App| actions::focus_mode(s, 0)))
                    .child(btn("Manual", "af-man", t, 70.0, 30.0, |s: &mut App| actions::focus_mode(s, 1)))
                    .child(btn("1-Push", "af-push", t, 70.0, 30.0, |s: &mut App| actions::focus_mode(s, 2))))
                .child(section_title(t, "Power"))
                .child(row().gap(8.0)
                    .child(accent_btn("Power ON", "power-on", dark, 110.0, 32.0, |s: &mut App| actions::power(s, true)))
                    .child(btn("Standby", "power-off", t, 100.0, 32.0, |s: &mut App| actions::power(s, false))))
            )
            .child(col().gap(8.0).w(320.0)
                .child(section_title(t, "Speeds"))
                .child(stepper(t, "pan-speed", "Pan speed", format!("{} (1-24)", s.pan_speed),
                    |s: &mut App| s.pan_speed = s.pan_speed.saturating_sub(1).max(1),
                    |s: &mut App| s.pan_speed = (s.pan_speed + 1).min(24)))
                .child(stepper(t, "tilt-speed", "Tilt speed", format!("{} (1-20)", s.tilt_speed),
                    |s: &mut App| s.tilt_speed = s.tilt_speed.saturating_sub(1).max(1),
                    |s: &mut App| s.tilt_speed = (s.tilt_speed + 1).min(20)))
                .child(stepper(t, "zoom-speed", "Zoom speed", format!("{} (0-7)", s.zoom_speed),
                    |s: &mut App| s.zoom_speed = s.zoom_speed.saturating_sub(1),
                    |s: &mut App| s.zoom_speed = (s.zoom_speed + 1).min(7)))
                .child(section_title(t, "Presets (0-254, remote 0-9)"))
                .child(stepper(t, "preset-sel", "Preset No.", format!("{}", s.preset_sel),
                    |s: &mut App| s.preset_sel = s.preset_sel.saturating_sub(1),
                    |s: &mut App| s.preset_sel = s.preset_sel.saturating_add(1).min(254)))
                .child(row().gap(8.0)
                    .child(btn("Recall", "preset-recall", t, 86.0, 32.0, |s: &mut App| { let n = s.preset_sel; actions::preset(s, n, 0); }))
                    .child(accent_btn("Set", "preset-set", dark, 70.0, 32.0, |s: &mut App| { let n = s.preset_sel; actions::preset(s, n, 1); }))
                    .child(btn("Clear", "preset-clear", t, 70.0, 32.0, |s: &mut App| { let n = s.preset_sel; actions::preset(s, n, 2); })))
                .child(preset_grid(t))
            )
        )
        .child(hint_card(t, "Tip: click an arrow to jog. Press Stop to halt. Home returns to center. Preset 0 is the power-on position if saved. Pan 40°/s, Tilt 30°/s, ±0.1° accuracy."))
}

fn camera_switcher(s: &App, t: rune::tok::Theme) -> El {
    let mut r = row().key("cams").w_full().h(40.0).gap(8.0).center_y();
    r = r.child(text("Camera:".to_owned()).font(type_::BODY).color(t.text_secondary));
    for (i, cam) in s.cameras.iter().enumerate() {
        let on = i == s.selected;
        let label = format!("{} ({})", cam.name, cam.ip);
        r = r.child(
            el().key(format!("cam-{}", i)).h(30.0).px(12.0).center().radius(15.0)
                .fill(if on { tok::accent(0, s.dark) } else { t.card })
                .border(1.0, t.card_border)
                .hover(t.hover).press(t.press).focusable(true)
                .role("tab").label(label.clone())
                .on_press(move |s: &mut App| { s.selected = i; s.set_status(format!("Selected {}", s.current().name.clone())); })
                .child(text(if on { format!("● {}", cam.name) } else { cam.name.clone() })
                    .font(type_::BODY)
                    .color(if on { rune::Color::WHITE } else { t.text }))
        );
    }
    r
}

fn ptz_pad(t: rune::tok::Theme) -> El {
    let b = |label: &'static str, key: &'static str, dir: PtDir, tip: &'static str| {
        btn(label, key, t, 64.0, 44.0, move |s: &mut App| actions::drive(s, dir, tip))
    };
    col().key("ptz").gap(6.0)
        .child(section_title(t, "Pan / Tilt (drag-free jog)"))
        .child(row().gap(6.0)
            .child(b("↖", "pt-nw", PtDir::UpLeft, "Up-Left"))
            .child(b("↑", "pt-n", PtDir::Up, "Up"))
            .child(b("↗", "pt-ne", PtDir::UpRight, "Up-Right")))
        .child(row().gap(6.0)
            .child(b("←", "pt-w", PtDir::Left, "Left"))
            .child(btn("HOME", "pt-home", t, 64.0, 44.0, actions::home))
            .child(b("→", "pt-e", PtDir::Right, "Right")))
        .child(row().gap(6.0)
            .child(b("↙", "pt-sw", PtDir::DownLeft, "Down-Left"))
            .child(b("↓", "pt-s", PtDir::Down, "Down"))
            .child(b("↘", "pt-se", PtDir::DownRight, "Down-Right")))
        .child(row().gap(6.0)
            .child(accent_btn("STOP", "pt-stop", true, 204.0, 34.0, actions::stop)))
}

fn preset_grid(t: rune::tok::Theme) -> El {
    let mut c = col().key("preset-grid").gap(6.0);
    c = c.child(text("Quick recall 0-9:".to_owned()).font(type_::LABEL).color(t.text_secondary));
    for r in 0..2 {
        let mut row_el = row().key(format!("pr-{}", r)).gap(6.0);
        for col_i in 0..5 {
            let n = (r * 5 + col_i) as u8;
            row_el = row_el.child(btn(&n.to_string(), &format!("q-{}", n), t, 44.0, 30.0,
                move |s: &mut App| actions::preset(s, n, 0)));
        }
        c = c.child(row_el);
    }
    c
}

fn hint_card(t: rune::tok::Theme, msg: &str) -> El {
    el().key(format!("hint-{}", msg.len())).w_full().padding(12.0).radius(8.0).fill(t.card).border(1.0, t.card_border)
        .child(text(msg.to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
}

// ---------------------------------------------------------------- image

fn image_page(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    col().key("image").w_full().px(16.0).py(14.0).gap(12.0)
        .child(text(format!("Image settings → {} @ {}", s.current().name, s.current().ip)).font(type_::TITLE).color(t.text))
        .child(text("Each row sends real VISCA to the camera. Values follow the FoMaKo manual §5.2.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
                .child(section_title(t, "Exposure"))
                .child(exposure_row(s, t, dark))
                .child(stepper(t, "shutter", "Shutter", format!("0x{:02X}", s.shutter),
                    |s: &mut App| s.shutter = s.shutter.saturating_sub(1),
                    |s: &mut App| s.shutter = s.shutter.saturating_add(1).min(20)))
                .child(apply_row(t, dark, "shutter-a", "Shutter direct", "VISCA 4A", actions::apply_shutter))
                .child(stepper(t, "iris", "Iris", format!("0x{:02X}", s.iris),
                    |s: &mut App| s.iris = s.iris.saturating_sub(1),
                    |s: &mut App| s.iris = s.iris.saturating_add(1).min(20)))
                .child(apply_row(t, dark, "iris-a", "Iris direct", "VISCA 4B", actions::apply_iris))
                .child(stepper(t, "gain", "Gain limit", format!("{}", s.gain_limit),
                    |s: &mut App| s.gain_limit = s.gain_limit.saturating_sub(1),
                    |s: &mut App| s.gain_limit = s.gain_limit.saturating_add(1).min(15)))
                .child(apply_row(t, dark, "gain-a", "Gain limit", "VISCA 2C", actions::apply_gain_limit))
                .child(stepper(t, "brightlvl", "Bright level", format!("{}", s.bright_lvl),
                    |s: &mut App| s.bright_lvl = s.bright_lvl.saturating_sub(1),
                    |s: &mut App| s.bright_lvl = s.bright_lvl.saturating_add(1).min(23)))
                .child(apply_row(t, dark, "brightlvl-a", "Bright", "VISCA 4D", actions::apply_bright_lvl))
                .child(section_title(t, "Backlight / WDR / NR"))
                .child(apply_row(t, dark, "blc", "Backlight", if s.backlight { "ON" } else { "OFF" }, |s: &mut App| { s.backlight = !s.backlight; actions::apply_backlight(s); }))
                .child(stepper(t, "wdr", "WDR level", format!("{}", s.wdr),
                    |s: &mut App| s.wdr = s.wdr.saturating_sub(1),
                    |s: &mut App| s.wdr = s.wdr.saturating_add(1).min(7)))
                .child(apply_row(t, dark, "wdr-a", "WDR", "VISCA 51", actions::apply_wdr))
                .child(stepper(t, "nr2d", "NR 2D (0-7)", format!("{}", s.nr2d),
                    |s: &mut App| s.nr2d = s.nr2d.saturating_sub(1),
                    |s: &mut App| s.nr2d = s.nr2d.saturating_add(1).min(7)))
                .child(stepper(t, "nr3d", "NR 3D (0-8)", format!("{}", s.nr3d),
                    |s: &mut App| s.nr3d = s.nr3d.saturating_sub(1),
                    |s: &mut App| s.nr3d = s.nr3d.saturating_add(1).min(8)))
                .child(apply_row(t, dark, "nr-a", "Noise reduction", "VISCA 53/54", actions::apply_nr))
                .child(section_title(t, "White balance / color"))
                .child(wb_row(s, t, dark))
                .child(stepper(t, "rgain", "R Gain", format!("{}", s.r_gain),
                    |s: &mut App| s.r_gain = s.r_gain.saturating_sub(1),
                    |s: &mut App| s.r_gain = s.r_gain.saturating_add(1)))
                .child(stepper(t, "bgain", "B Gain", format!("{}", s.b_gain),
                    |s: &mut App| s.b_gain = s.b_gain.saturating_sub(1),
                    |s: &mut App| s.b_gain = s.b_gain.saturating_add(1)))
                .child(apply_row(t, dark, "gains", "R/B Gain", "VISCA 43/44", actions::apply_gains))
                .child(section_title(t, "Picture (brightness / contrast / saturation / sharpness)"))
                .child(stepper(t, "brightness", "Brightness", format!("{}", s.brightness),
                    |s: &mut App| s.brightness = s.brightness.saturating_sub(1),
                    |s: &mut App| s.brightness = (s.brightness + 1).min(14)))
                .child(apply_row(t, dark, "brightness-a", "Brightness", "VISCA A1", actions::apply_brightness))
                .child(stepper(t, "contrast", "Contrast", format!("{}", s.contrast),
                    |s: &mut App| s.contrast = s.contrast.saturating_sub(1),
                    |s: &mut App| s.contrast = (s.contrast + 1).min(14)))
                .child(apply_row(t, dark, "contrast-a", "Contrast", "VISCA A2", actions::apply_contrast))
                .child(stepper(t, "sat", "Saturation 0-14", format!("{} (60-200%)", s.saturation),
                    |s: &mut App| s.saturation = s.saturation.saturating_sub(1),
                    |s: &mut App| s.saturation = (s.saturation + 1).min(14)))
                .child(apply_row(t, dark, "sat-a", "Saturation", "VISCA 49", actions::apply_saturation))
                .child(stepper(t, "aperture", "Sharpness 0-15", format!("{}", s.aperture),
                    |s: &mut App| s.aperture = s.aperture.saturating_sub(1),
                    |s: &mut App| s.aperture = (s.aperture + 1).min(15)))
                .child(apply_row(t, dark, "aperture-a", "Sharpness", "VISCA 42", actions::apply_aperture))
                .child(section_title(t, "Misc"))
                .child(stepper(t, "gamma", "Gamma 0-4", format!("{}", s.gamma),
                    |s: &mut App| s.gamma = s.gamma.saturating_sub(1),
                    |s: &mut App| s.gamma = (s.gamma + 1).min(4)))
                .child(apply_row(t, dark, "gamma-a", "Gamma", "VISCA 5B", actions::apply_gamma))
                .child(apply_row(t, dark, "flicker", "Flicker", match s.flicker { 1 => "50Hz", 2 => "60Hz", _ => "OFF" }, |s: &mut App| { s.flicker = (s.flicker + 1) % 3; actions::apply_flicker(s); }))
                .child(apply_row(t, dark, "lowlight", "Low-light", if s.lowlight { "ON" } else { "OFF" }, |s: &mut App| { s.lowlight = !s.lowlight; actions::apply_lowlight(s); }))
                .child(apply_row(t, dark, "flip", "Flip", match s.flip { 1 => "H", 2 => "V", 3 => "HV", _ => "OFF" }, |s: &mut App| { s.flip = (s.flip + 1) % 4; actions::apply_flip(s); }))
                .child(apply_row(t, dark, "bw", "B/W mode", if s.bw { "B&W" } else { "Color" }, |s: &mut App| { s.bw = !s.bw; actions::apply_bw(s); }))
}

fn exposure_row(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    let modes = ["Auto", "Manual", "Shutter", "Iris", "Bright"];
    let mut r = row().key("ae-row").w_full().gap(6.0).center_y();
    r = r.child(text(format!("Mode: {}", s.ae_label())).font(type_::BODY).color(t.text).w(150.0));
    for (i, m) in modes.iter().enumerate() {
        let on = s.ae_mode as usize == i;
        let label = m.to_string();
        r = r.child(
            el().key(format!("ae-{}", i)).h(28.0).px(10.0).center().radius(14.0)
                .fill(if on { tok::accent(0, dark) } else { t.card })
                .hover(t.hover).focusable(true)
                .on_press(move |s: &mut App| { s.ae_mode = i as u8; actions::apply_ae(s); })
                .child(text(label).font(type_::LABEL).color(if on { rune::Color::WHITE } else { t.text }))
        );
    }
    r = r.child(accent_btn("Send", "ae-send", dark, 64.0, 28.0, actions::apply_ae));
    r
}

fn wb_row(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    let modes = ["Auto", "3000K", "4000K", "1-Push", "Manual", "6500K"];
    let map = [0u8, 1, 2, 3, 5, 6];
    let mut r = row().key("wb-row").w_full().gap(6.0).center_y();
    r = r.child(text(format!("WB: {}", s.wb_label())).font(type_::BODY).color(t.text).w(150.0));
    for (idx, m) in modes.iter().enumerate() {
        let v = map[idx];
        let on = s.wb_mode == v;
        let label = m.to_string();
        r = r.child(
            el().key(format!("wb-{}", v)).h(26.0).px(8.0).center().radius(13.0)
                .fill(if on { tok::accent(0, dark) } else { t.card })
                .hover(t.hover).focusable(true)
                .on_press(move |s: &mut App| { s.wb_mode = v; actions::apply_wb(s); })
                .child(text(label).font(type_::LABEL).color(if on { rune::Color::WHITE } else { t.text }))
        );
    }
    r
}

// ---------------------------------------------------------------- picture edit

fn picture_page(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    col().key("picture").w_full().px(16.0).py(14.0).gap(12.0)
        .child(text("Picture Edit — preview, then Apply to Camera".to_owned()).font(type_::TITLE).color(t.text))
        .child(text("Tune the look like a photo editor. The preview below simulates the change live. Apply sends Brightness/Contrast/Saturation/Sharpness (VISCA A1/A2/49/42) to the selected camera.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        .child(preview_pane(s, t))
        .child(col().w_full().gap(16.0)
            .child(col().w_full().gap(8.0)
                .child(section_title(t, "Adjust"))
                .child(pic_slider(t, "pic-b", "Brightness", s.pic_bright))
                .child(pic_slider(t, "pic-c", "Contrast", s.pic_contrast))
                .child(pic_slider(t, "pic-s", "Saturation", s.pic_sat))
                .child(pic_slider(t, "pic-sh", "Sharpness", s.pic_sharp))
                .child(row().gap(8.0)
                    .child(accent_btn("Apply to Camera", "pic-apply", dark, 150.0, 34.0, actions::apply_picture_edit))
                    .child(btn("Reset", "pic-reset", t, 80.0, 34.0, |s: &mut App| {
                        s.pic_bright = 0; s.pic_contrast = 0; s.pic_sat = 0; s.pic_sharp = 0;
                        s.set_status("Picture sliders reset (camera unchanged)".into());
                    })))
                .child(hint_card(t, "Mapping: ±100 → Bright 0-14, Contrast 0-14, Sat 0-14 (60-200%), Sharp 0-15. Center = camera 7/7/7/8."))
            )
            .child(col().w_full().gap(8.0)
                .child(section_title(t, "Live simulated preview"))
                .child(preview_bars(s, t))
                .child(preview_gray(s, t))
                .child(text(format!("Will send: Bright {}  Contrast {}  Sat {}  Sharp {}",
                    ((7.0 + s.pic_bright as f32 * 0.07).round() as i32).clamp(0,14),
                    ((7.0 + s.pic_contrast as f32 * 0.07).round() as i32).clamp(0,14),
                    ((7.0 + s.pic_sat as f32 * 0.07).round() as i32).clamp(0,14),
                    ((8.0 + s.pic_sharp as f32 * 0.07).round() as i32).clamp(0,15)
                )).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
            )
        )
}

fn pic_slider(t: rune::tok::Theme, key: &'static str, label: &str, v: i32) -> El {
    let minus_key = format!("{}-minus", key);
    let plus_key = format!("{}-plus", key);
    let k = key.to_owned();
    let l = label.to_owned();
    row().key(k).w_full().h(34.0).center_y().gap(8.0)
        .child(text(l).font(type_::BODY).color(t.text).w(110.0))
        .child(btn("−", &minus_key, t, 34.0, 28.0, move |s: &mut App| {
            match key {
                "pic-b" => s.pic_bright = (s.pic_bright - 5).max(-100),
                "pic-c" => s.pic_contrast = (s.pic_contrast - 5).max(-100),
                "pic-s" => s.pic_sat = (s.pic_sat - 5).max(-100),
                _ => s.pic_sharp = (s.pic_sharp - 5).max(-100),
            }
        }))
        .child(text(format!("{:+}", v)).font(type_::MONO_CAPTION).color(t.text).w(50.0))
        .child(btn("+", &plus_key, t, 34.0, 28.0, move |s: &mut App| {
            match key {
                "pic-b" => s.pic_bright = (s.pic_bright + 5).min(100),
                "pic-c" => s.pic_contrast = (s.pic_contrast + 5).min(100),
                "pic-s" => s.pic_sat = (s.pic_sat + 5).min(100),
                _ => s.pic_sharp = (s.pic_sharp + 5).min(100),
            }
        }))
        .child(el().w(120.0).h(8.0).radius(4.0).fill(t.control_track)
            .child(el().w((60.0 + v as f32 * 0.6).clamp(0.0, 120.0)).h(8.0).radius(4.0).fill(tok::accent(0, true))))
}

fn adjust_color(c: (f32, f32, f32), bright: i32, contrast: i32, sat: i32) -> rune::Color {
    // simple photo math in 0..1
    let b = bright as f32 / 100.0 * 0.35;
    let co = 1.0 + contrast as f32 / 100.0 * 0.8;
    let sa = (1.0 + sat as f32 / 100.0 * 0.9).clamp(0.0, 2.5);
    let mut rgb = [c.0, c.1, c.2];
    for v in rgb.iter_mut() {
        *v = ((*v - 0.5) * co + 0.5 + b).clamp(0.0, 1.0);
    }
    let l = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
    for v in rgb.iter_mut() {
        *v = (l + (*v - l) * sa).clamp(0.0, 1.0);
    }
    rune::Color { r: rgb[0], g: rgb[1], b: rgb[2], a: 1.0 }
}

fn preview_bars(s: &App, t: rune::tok::Theme) -> El {
    let cols = [
        ("R", (0.85, 0.15, 0.15)),
        ("G", (0.15, 0.70, 0.20)),
        ("B", (0.15, 0.35, 0.90)),
        ("Y", (0.90, 0.80, 0.15)),
        ("C", (0.15, 0.80, 0.85)),
        ("M", (0.75, 0.20, 0.70)),
    ];
    let mut r = row().key("bars").w_full().h(90.0).gap(6.0);
    for (name, c) in cols {
        let adj = adjust_color(c, s.pic_bright, s.pic_contrast, s.pic_sat);
        let sharp_border = if s.pic_sharp > 40 { 3.0 } else if s.pic_sharp > 0 { 2.0 } else { 0.0 };
        r = r.child(
            el().key(format!("bar-{}", name)).grow(1.0).h(90.0).radius(8.0)
                .fill(adj).border(sharp_border, t.text)
                .child(col().w_full().h_full().center()
                    .child(text(name.to_owned()).font(type_::TITLE).color(rune::Color::WHITE)))
        );
    }
    r
}

fn preview_gray(s: &App, _t: rune::tok::Theme) -> El {
    let mut r = row().key("gray").w_full().h(36.0).gap(4.0);
    for i in 0..8 {
        let g = i as f32 / 7.0;
        let adj = adjust_color((g, g, g), s.pic_bright, s.pic_contrast, 0);
        r = r.child(el().key(format!("g-{}", i)).grow(1.0).h(36.0).radius(4.0).fill(adj));
    }
    r
}

// ---------------------------------------------------------------- cameras

fn cameras_page(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    col().key("cameras").w_full().px(16.0).py(14.0).gap(12.0)
        .child(text("Cameras — switch, add, remove".to_owned()).font(type_::TITLE).color(t.text))
        .child(text("Stored in fomako-cameras.json next to the app (USB-stick portable). Default: admin/admin. FoMaKo LCD shows the IP.".to_owned()).font(type_::LABEL).color(t.text_secondary))
        .child(camera_list(s, t))
        .child(creds_section(s, t))
        .child(scan_section(s, t, dark))
        .child(relay_section(s, t, dark))
        .child(add_form(s, t, dark))
        .child(stream_help(s, t))
}

fn camera_list(s: &App, t: rune::tok::Theme) -> El {
    let mut c = col().key("cam-list").w_full().gap(6.0);
    c = c.child(section_title(t, &format!("Saved ({}). Tap Select to switch.", s.cameras.len())));
    for (i, cam) in s.cameras.iter().enumerate() {
        let on = i == s.selected;
        let name = cam.name.clone();
        let ip = cam.ip.clone();
        c = c.child(
            row().key(format!("camrow-{}", i)).w_full().h(44.0).px(12.0).gap(10.0).center_y()
                .radius(8.0).fill(if on { t.control_track } else { t.card }).border(1.0, t.card_border)
                .child(text(format!("{} #{}  {}  {}:{}", if on { "●" } else { "○" }, cam.address, name, ip, cam.protocol.port()))
                    .font(type_::BODY).color(t.text).grow(1.0))
                .child(btn("Select", &format!("sel-{}", i), t, 70.0, 28.0, move |s: &mut App| {
                    s.selected = i;
                    s.set_status(format!("Selected {}", s.current().name.clone()));
                }))
                .child(btn("Test", &format!("test-{}", i), t, 60.0, 28.0, move |s: &mut App| {
                    let cam = s.cameras.get(i).cloned().unwrap_or_default();
                    let payload = crate::visca::pan_tilt_drive(cam.address, 8, 8, crate::visca::PtDir::Stop);
                    let hex = crate::visca::hex(&payload);
                    match crate::visca::send_blocking(&cam.ip, cam.protocol.port(), cam.protocol.to_protocol(), &payload) {
                        Ok(msg) => s.set_status(format!("Test {} OK: {} [{}]", cam.name, msg, hex)),
                        Err(e) => s.set_status(format!("Test {} FAILED {}: {}", cam.name, cam.ip, e)),
                    }
                }))
        );
    }
    if s.cameras.len() > 1 {
        c = c.child(row().gap(8.0)
            .child(btn("Remove selected", "cam-remove", t, 140.0, 30.0, |s: &mut App| {
                if s.cameras.len() > 1 {
                    s.cameras.remove(s.selected);
                    s.selected = 0;
                    s.save();
                    s.set_status("Camera removed".into());
                }
            })));
    }
    c
}

fn creds_section(s: &App, t: rune::tok::Theme) -> El {
    // Edit box mirrors the selected camera; Save writes it back.
    let (name, user) = {
        let c = s.current();
        (c.name.clone(), c.username.clone())
    };
    col().key("creds").w_full().gap(8.0)
        .child(section_title(t, &format!("Login for {} (RTSP/web; VISCA needs none)", name)))
        .child(row().w_full().gap(8.0).center_y()
            .child(text("User:".to_owned()).font(type_::BODY).color(t.text).w(60.0))
            .child(text_input(&s.user_edit, field_style()).single_line().key("cred-user").w(130.0)
                .placeholder(user.clone(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.user_edit))
            .child(text("Pass:".to_owned()).font(type_::BODY).color(t.text).w(50.0))
            .child(text_input(&s.pass_edit, field_style()).single_line().secure().key("cred-pass").w(130.0)
                .placeholder("••••••".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.pass_edit))
            .child(btn("Save login", "cred-save", t, 110.0, 30.0, move |s: &mut App| {
                let u = s.user_edit.text.trim().to_owned();
                let p = s.pass_edit.text.trim().to_owned();
                if let Some(c) = s.cameras.get_mut(s.selected) {
                    if !u.is_empty() {
                        c.username = u;
                    }
                    if !p.is_empty() {
                        c.password = p;
                    }
                    s.user_edit = rune::TextEdit::new("");
                    s.pass_edit = rune::TextEdit::new("");
                    s.save();
                    s.set_status(format!("Login saved for {}", s.current().name.clone()));
                }
            })))
}

fn scan_section(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    let (running, done, total, msg) = match &s.scan {
        Some(sc) => (sc.running, sc.done, sc.total, sc.message.clone()),
        None => (false, 0, 254, String::new()),
    };
    let mut c = col().key("scan").w_full().gap(8.0)
        .child(section_title(t, "Find cameras on my network (auto)"))
        .child(text("Phone/PC and camera must join the same WiFi/LAN. Desktop scans directly; the browser build scans through the relay below. Cameras on another network? Add the IP manually underneath.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        .child(row().w_full().gap(8.0).center_y()
            .child(text("Subnet:".to_owned()).font(type_::BODY).color(t.text).w(70.0))
            .child(text_input(&s.scan_prefix, field_style()).single_line().key("subnet-field").w(130.0)
                .placeholder("192.168.1".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.scan_prefix))
            .child(text(".1–254".to_owned()).font(type_::BODY).color(t.text_secondary))
            .child(accent_btn_tx(if running { "Stop" } else { "Scan network" }, "scan-btn", dark, 140.0, 32.0,
                move |s: &mut App, tx: &mut rune::Tx| crate::actions::start_scan(s, tx))))
        .child(text(if running {
            format!("Scanning… {}/{} — cameras appear below as found.", done, total)
        } else if msg.is_empty() {
            "Not scanned yet. Tip: FoMaKo default lives on 192.168.5.x.".to_owned()
        } else {
            msg
        }).w_full().wrap().font(type_::LABEL).color(t.text_secondary));
    if let Some(sc) = &s.scan {
        for (i, r) in sc.results.iter().enumerate() {
            let ip = r.ip.clone();
            let line = format!("{} — {}", r.ip, r.detail);
            c = c.child(
                row().key(format!("found-{}", i)).w_full().h(40.0).px(12.0).gap(10.0).center_y()
                    .radius(8.0).fill(t.card).border(1.0, t.card_border)
                    .child(text(line).font(type_::BODY).color(t.text).grow(1.0))
                    .child(btn("Add", &format!("add-found-{}", i), t, 64.0, 28.0, move |s: &mut App| {
                        if s.cameras.iter().any(|c| c.ip == ip) {
                            s.set_status(format!("{} is already saved", ip));
                            return;
                        }
                        let sc = match &s.scan { Some(sc) => sc, None => return };
                        let found = match sc.results.get(i) { Some(r) => r.clone(), None => return };
                        let proto = if found.udp52381 {
                            crate::model::ProtoSer::SonyUdp
                        } else {
                            crate::model::ProtoSer::Tcp
                        };
                        let addr = (s.cameras.len() as u8 + 1).min(7);
                        s.cameras.push(crate::model::CameraConfig {
                            name: format!("Camera {}", found.ip),
                            ip: found.ip.clone(),
                            protocol: proto,
                            address: addr,
                            username: "admin".into(),
                            password: "admin".into(),
                        });
                        s.selected = s.cameras.len() - 1;
                        s.save();
                        s.set_status(format!("Added {}", found.ip));
                    }))
            );
        }
    }
    c
}

fn relay_section(s: &App, t: rune::tok::Theme, _dark: bool) -> El {
    col().key("relay").w_full().gap(8.0)
        .child(section_title(t, "Phone / browser control (LAN relay)"))
        .child(text("Browsers can't open camera sockets, so the phone build talks to a tiny relay on your LAN PC (python3 scripts/visca-relay.py --serve-web), which forwards to the cameras. Turn it on, paste the relay URL, then Scan / control works from a phone browser.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        .child(row().w_full().gap(8.0).center_y()
            .child(btn(if s.use_relay { "Relay: ON" } else { "Relay: OFF" }, "relay-toggle", t, 110.0, 30.0, |s: &mut App| {
                s.use_relay = !s.use_relay;
                s.set_status(if s.use_relay { "Relay ON — commands go via relay URL".into() } else { "Relay OFF — direct sockets".into() });
            }))
            .child(text_input(&s.relay_url, field_style()).single_line().key("relay-field").grow(1.0)
                .placeholder("http://192.168.1.10:8080".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.relay_url)))
}

fn add_form(s: &App, t: rune::tok::Theme, dark: bool) -> El {
    col().key("add").w_full().gap(8.0)
        .child(section_title(t, "Add camera"))
        .child(row().w_full().gap(8.0).center_y()
            .child(text("Name:".to_owned()).font(type_::BODY).color(t.text).w(60.0))
            .child(text_input(&s.name_edit, field_style()).single_line().key("name-field").w(200.0)
                .placeholder("Pulpit".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.name_edit)))
        .child(row().w_full().gap(8.0).center_y()
            .child(text("IP:".to_owned()).font(type_::BODY).color(t.text).w(60.0))
            .child(text_input(&s.ip_edit, field_style()).single_line().key("ip-field").w(200.0)
                .placeholder("192.168.5.163".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.ip_edit))
            .child(text("Protocol: tap to toggle (UDP 52381 recommended)".to_owned()).font(type_::LABEL).color(t.text_secondary)))
        .child(row().w_full().gap(8.0).center_y()
            .child(text("Login:".to_owned()).font(type_::BODY).color(t.text).w(60.0))
            .child(text_input(&s.user_edit, field_style()).single_line().key("user-field").w(120.0)
                .placeholder("admin".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.user_edit))
            .child(text(" /".to_owned()).font(type_::BODY).color(t.text_secondary))
            .child(text_input(&s.pass_edit, field_style()).single_line().secure().key("pass-field").w(120.0)
                .placeholder("admin".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.pass_edit))
            .child(text("(RTSP/web login; VISCA needs none)".to_owned()).font(type_::LABEL).color(t.text_secondary)))
        .child(row().gap(8.0)
            .child(accent_btn("Add Sony UDP :52381", "add-udp", dark, 180.0, 32.0, |s: &mut App| {
                let name = if s.name_edit.text.trim().is_empty() { "Camera".into() } else { s.name_edit.text.trim().to_owned() };
                let ip = s.ip_edit.text.trim().to_owned();
                if ip.is_empty() { s.set_status("Enter an IP first".into()); return; }
                let addr = (s.cameras.len() as u8 + 1).min(7);
                let username = s.user_edit.text.trim().to_owned();
                let password = s.pass_edit.text.trim().to_owned();
                let username = if username.is_empty() { "admin".into() } else { username };
                let password = if password.is_empty() { "admin".into() } else { password };
                s.cameras.push(crate::model::CameraConfig { name, ip, protocol: crate::model::ProtoSer::SonyUdp, address: addr, username, password });
                s.selected = s.cameras.len() - 1;
                s.save();
                s.set_status(format!("Added {}", s.current().name.clone()));
            }))
            .child(btn("Add TCP :5678", "add-tcp", t, 150.0, 32.0, |s: &mut App| {
                let name = if s.name_edit.text.trim().is_empty() { "Camera".into() } else { s.name_edit.text.trim().to_owned() };
                let ip = s.ip_edit.text.trim().to_owned();
                if ip.is_empty() { s.set_status("Enter an IP first".into()); return; }
                let addr = (s.cameras.len() as u8 + 1).min(7);
                let username = s.user_edit.text.trim().to_owned();
                let password = s.pass_edit.text.trim().to_owned();
                let username = if username.is_empty() { "admin".into() } else { username };
                let password = if password.is_empty() { "admin".into() } else { password };
                s.cameras.push(crate::model::CameraConfig { name, ip, protocol: crate::model::ProtoSer::Tcp, address: addr, username, password });
                s.selected = s.cameras.len() - 1;
                s.save();
                s.set_status(format!("Added {}", s.current().name.clone()));
            })))
}

fn stream_help(s: &App, t: rune::tok::Theme) -> El {
    let cam = s.current();
    let ip = cam.ip.clone();
    col().key("streams").w_full().gap(4.0)
        .child(section_title(t, "Streams (open in VLC / OBS / vMix)"))
        .child(text(format!("Main:  rtsp://{}:554/live/av0", ip)).font(type_::MONO_CAPTION).color(t.text))
        .child(text(format!("Sub:   rtsp://{}:554/live/av1", ip)).font(type_::MONO_CAPTION).color(t.text))
        .child(text(format!("RTMP:  rtmp://{}:1935/live/av0", ip)).font(type_::MONO_CAPTION).color(t.text))
        .child(text(format!("Web UI: http://{}  ({} / {})", ip, s.current().username, "•••")).font(type_::MONO_CAPTION).color(t.text))
        .child(hint_card(t, "Video inside this app is control-only: FoMaKo streams via RTSP/RTMP/SRT/NDI (manual §4.3). Keep VLC open beside the controller, or paste the RTSP into OBS as a Media Source."))
}

// ---------------------------------------------------------------- help

fn help_page(s: &App, t: rune::tok::Theme, _dark: bool) -> El {
    let _ = s;
    col().key("help").w_full().px(16.0).py(14.0).gap(10.0)
        .child(text("Help — church setup in 5 minutes".to_owned()).font(type_::TITLE).color(t.text))
        .child(help_card(t, "1. Wire it",
            "HDMI → TV to see menus (press MENU on the IR remote). LAN → same router/switch as this PC. Power on: camera sweeps left, zooms far→near, then returns to Preset 0 if saved. LCD shows its IP (default 192.168.5.163 if no DHCP)."))
        .child(help_card(t, "2. Lock the IP",
            "MENU → Network → turn DHCP OFF → confirm (Home). Otherwise the IP moves after a reboot and this app loses it. User admin, password admin (restore with * # Manual on the remote)."))
        .child(help_card(t, "3. Control protocol",
            "Use Sony VISCA-over-IP UDP 52381 (recommended, steady) or IP-VISCA TCP 5678. Onvif is TCP 2000. This app sends Sony VISCA by default: 81 01 06 01 … FF wrapped in 01 00 + length + sequence. No serial cable needed."))
        .child(help_card(t, "4. Sunday workflow",
            "Save framings as presets: frame pulpit → set Preset Sel 1 → Set. Choir → 2 → Set. Wide → 3 → Set. During service tap Recall 1/2/3 (quick grid 0-9 on Control page). Keep Pan 10-16 / Tilt 10-14 for smooth moves."))
        .child(help_card(t, "5. Picture",
            "If the image is dark/washed: Image page → Exposure Full Auto first, then tune Picture Edit → Apply to Camera. White balance Auto indoors; R/B Gain only in Manual WB. Backlight ON for backlit pulpit, WDR 1-3 for windows."))
        .child(help_card(t, "Manual",
            "FoMaKo NDI/SDI/HDMI PTZ User Manual V3.0 (PDF): https://fomako.net/uploads/20230817/fd1bfd181b89ae798bee1f7ed260a7b3.pdf — §§3 remote+menu, 4 network/streaming, 5 VISCA/Pelco lists. IR: * # 7 shows IP, # # # clears presets, * # 6 factory reset."))
        .child(hint_card(t, "Built with Rune (Rust GPU UI, anomalyco/rune). Runs on Windows / macOS / Linux. Config fomako-cameras.json sits beside the app — copy it to every booth PC."))
}

fn help_card(t: rune::tok::Theme, title: &str, body: &str) -> El {
    col().key(format!("help-{}", title)).w_full().padding(12.0).gap(4.0).radius(8.0).fill(t.card).border(1.0, t.card_border)
        .child(text(title.to_owned()).font(type_::BODY).color(t.text))
        .child(text(body.to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
}
