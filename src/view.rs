//! Rune view for the FoMaKo church controller.

use rune::tok::{self, type_};
use rune::{Cx, El, ScrollSpec, TextStyle, col, el, row, text, text_input};

use crate::actions;
use crate::model::{App, Page};
use crate::update::{self, UpdatePhase};
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

fn btn_labeled<F>(visible: &str, a11y_label: &str, key: &str, t: rune::tok::Theme, w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    el().key(key.to_owned()).w(w).h(h).center().radius(8.0)
        .fill(t.card).border(1.0, t.card_border)
        .hover(t.hover).press(t.press).focusable(true)
        .role("button").label(a11y_label.to_owned())
        .on_press(f)
        .child(text(visible.to_owned()).font(type_::BODY).color(t.text).a11y_hidden())
}

fn btn<F>(label: &str, key: &str, t: rune::tok::Theme, w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    btn_labeled(label, label, key, t, w, h, f)
}

fn accent_btn_labeled<F>(visible: &str, a11y_label: &str, key: &str, dark: bool, w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    let a = tok::accent(0, dark);
    el().key(key.to_owned()).w(w).h(h).center().radius(8.0)
        .fill(a).hover(a).press(a).focusable(true)
        .role("button").label(a11y_label.to_owned())
        .on_press(f)
        .child(text(visible.to_owned()).font(type_::BODY).color(rune::Color::WHITE).a11y_hidden())
}

fn btn_flex<F>(visible: &str, a11y_label: &str, key: &str, t: rune::tok::Theme, min_w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    el().key(key.to_owned()).grow(1.0).min_w(min_w).h(h).px(8.0).center().radius(8.0)
        .fill(t.card).border(1.0, t.card_border)
        .hover(t.hover).press(t.press).focusable(true)
        .role("button").label(a11y_label.to_owned())
        .on_press(f)
        .child(text(visible.to_owned()).font(type_::BODY).color(t.text).a11y_hidden())
}

fn accent_btn_flex<F>(visible: &str, a11y_label: &str, key: &str, dark: bool, min_w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    let a = tok::accent(0, dark);
    el().key(key.to_owned()).grow(1.0).min_w(min_w).h(h).px(8.0).center().radius(8.0)
        .fill(a).hover(a).press(a).focusable(true)
        .role("button").label(a11y_label.to_owned())
        .on_press(f)
        .child(text(visible.to_owned()).font(type_::BODY).color(rune::Color::WHITE).a11y_hidden())
}

fn accent_btn<F>(label: &str, key: &str, dark: bool, w: f32, h: f32, f: F) -> El
where F: Fn(&mut App) + 'static {
    accent_btn_labeled(label, label, key, dark, w, h, f)
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

/// Quiet button that opens a link in the system browser.
fn link_btn(label: &str, a11y: &str, key: &str, url: &'static str, t: rune::tok::Theme, h: f32) -> El {
    el().key(key.to_owned()).h(h).px(12.0).center().radius(8.0)
        .fill(t.card).border(1.0, t.card_border)
        .hover(t.hover).press(t.press).focusable(true)
        .role("button").label(a11y.to_owned())
        .on_press_tx(move |_s: &mut App, tx: &mut rune::Tx| {
            let _ = tx.open_url(url);
        })
        .child(text(label.to_owned()).font(type_::BODY).color(t.text).a11y_hidden())
}

fn section_title(t: rune::tok::Theme, title: &str) -> El {
    text(title.to_owned()).key(format!("sec-{}", title)).role("heading").font(type_::SECTION).color(t.text_secondary)
}

fn stepper(t: rune::tok::Theme, touch: bool, key: &str, label: &str, value: String, on_minus: impl Fn(&mut App) + 'static, on_plus: impl Fn(&mut App) + 'static) -> El {
    let bw = if touch { 44.0 } else { 34.0 };
    let bh = if touch { 44.0 } else { 28.0 };
    row().key(key.to_owned()).w_full().min_h(bh + 6.0).py(2.0).center_y().gap(8.0).flex_wrap()
        .child(text(label.to_owned()).font(type_::BODY).color(t.text).grow(1.0).min_w(96.0).wrap())
        .child(btn_labeled("−", &format!("Decrease {}", label), &format!("{}-minus", key), t, bw, bh, on_minus))
        .child(text(value).font(type_::MONO_CAPTION).color(t.text).min_w(64.0))
        .child(btn_labeled("+", &format!("Increase {}", label), &format!("{}-plus", key), t, bw, bh, on_plus))
}

fn apply_row<F>(t: rune::tok::Theme, dark: bool, touch: bool, key: &str, label: &str, value: &str, f: F) -> El
where F: Fn(&mut App) + 'static {
    let bh = if touch { 44.0 } else { 28.0 };
    row().key(key.to_owned()).w_full().min_h(bh + 8.0).py(2.0).center_y().gap(8.0).flex_wrap()
        .child(text(label.to_owned()).font(type_::BODY).color(t.text).grow(1.0).min_w(104.0).wrap())
        .child(text(value.to_owned()).font(type_::LABEL).color(t.text_secondary).min_w(72.0))
        .child(accent_btn_labeled("Apply", &format!("Apply {}", label), &format!("{}-apply", key), dark, 76.0, bh, f))
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = tok::theme(s.dark);
    let dark = s.dark;
    let touch = cx.touch_controls();
    cx.web_focus();
    let safe = cx.safe_area();
    col().key("window").w_full().h_full().fill(t.bg).pt(safe[0]).pb(safe[2])
        .child(topbar(s, t, dark, touch, cx))
        .child(tabbar(s, t, dark, touch))
        .child(
            col().key("content").grow(1.0).shrink().min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(match s.page {
                    Page::Control => control_page(s, t, dark, touch),
                    Page::Image => image_page(s, t, dark, touch),
                    Page::Picture => picture_page(s, t, dark, touch),
                    Page::Cameras => cameras_page(s, t, dark, touch),
                    Page::Help => help_page(s, t, dark, touch),
                })
        )
        .child(statusbar(s, t))
}

fn topbar(s: &App, t: rune::tok::Theme, dark: bool, touch: bool, _cx: &mut Cx<App>) -> El {
    let cam = s.current();
    let title = format!("FoMaKo Church Control  •  {} @ {}  •  {}", cam.name, cam.ip, cam.protocol.label());
    let bh = if touch { 42.0 } else { 30.0 };
    let mut bar = row().key("topbar").w_full().min_h(52.0).py(8.0).px(16.0).center_y().gap(10.0).flex_wrap().fill(t.card).border(0.0, t.card_border)
        .child(
            col().grow(1.0).shrink().min_w(160.0)
                .child(text("FoMaKo PTZ — Church Controller".to_owned()).w_full().wrap().role("heading").font(type_::TITLE).color(t.text))
                .child(text(title).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        );
    // A newer build is waiting: keep it visible without stealing focus.
    match s.update_phase {
        UpdatePhase::Available => {
            bar = bar.child(btn_labeled(
                &format!("⬆ Update v{}", s.update_version),
                "A newer version is available — open Help to install it",
                "update-badge", t, 132.0, bh,
                |s: &mut App| s.page = Page::Help,
            ));
        }
        UpdatePhase::Ready => {
            bar = bar.child(btn_labeled(
                "⬆ Restart to update",
                "The new version is installed — open Help to restart",
                "update-badge", t, 150.0, bh,
                |s: &mut App| s.page = Page::Help,
            ));
        }
        _ => {}
    }
    bar.child(btn_labeled(
        if dark { "☾ Dark" } else { "☀ Light" },
        if dark { "Switch to light theme" } else { "Switch to dark theme" },
        "theme", t, 88.0, bh, |s: &mut App| s.dark = !s.dark
    ))
}

fn tabbar(s: &App, t: rune::tok::Theme, _dark: bool, touch: bool) -> El {
    let th = if touch { 42.0 } else { 30.0 };
    let mut r = row().key("tabs").w_full().min_h(42.0).py(6.0).px(12.0).gap(6.0).center_y().flex_wrap().fill(t.bg);
    for p in Page::all() {
        let on = s.page == p;
        let label = p.title().to_owned();
        let key = format!("tab-{}", label);
        r = r.child(
            el().key(key).h(th).px(14.0).center().radius(th * 0.5)
                .fill(if on { t.control_track } else { t.bg })
                .border(1.0, if on { t.card_border } else { t.bg })
                .hover(t.hover).press(t.press).focusable(true)
                .role("tab").checked(on).label(label.clone())
                .on_press(move |s: &mut App| s.page = p)
                .child(text(label).font(type_::BODY).color(t.text).a11y_hidden())
        );
    }
    r
}

fn statusbar(s: &App, t: rune::tok::Theme) -> El {
    let last = s.log.last().cloned().unwrap_or_default();
    col().key("status").w_full().pt(8.0).pb(10.0).px(16.0).gap(4.0).fill(t.card)
        .child(text(format!("Status: {}", s.status)).w_full().wrap().role("status").live_region(rune::LiveRegion::Polite).font(type_::LABEL).color(t.text))
        .child(text(format!("Log: {}", last)).w_full().wrap().font(type_::MONO_CAPTION).color(t.text_secondary))
}

// ---------------------------------------------------------------- control

/// Live camera picture (ffmpeg snapshots, or relay snapshots on web).
/// Fixed 16:9 box so framing never distorts.
fn preview_pane(s: &App, t: rune::tok::Theme, touch: bool) -> El {
    #[cfg(not(target_arch = "wasm32"))]
    let live = s.preview_image.as_ref().and_then(|img| img.size().map(|_| img));
    let has_picture = s.preview_ready;
    let bh = if touch { 42.0 } else { 28.0 };
    let mut box_el = el().key("preview-box").w_full().max_w(520.0).max_h(292.0).aspect_ratio(16.0 / 9.0).center().radius(8.0)
        .fill(t.card).border(1.0, t.card_border);
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(img) = live {
        box_el = box_el.child(
            el().w_full().h_full().radius(8.0).child(
                el().key("preview-img").w_full().h_full().pixels(img.pixels())
            )
        );
    }
    // Web: report the box rect so the JPEG canvas overlay can sit exactly on it.
    #[cfg(target_arch = "wasm32")]
    let box_el = box_el.on_drawn_layout(|s: &mut App, rect| s.preview_rect = Some(rect));
    let mut pane = col().key("preview").w_full().gap(6.0)
        .child(row().w_full().gap(8.0).center_y().flex_wrap()
            .child(section_title(t, "Live preview"))
            .child(text(s.preview_status.clone()).font(type_::LABEL).color(t.text_secondary).grow(1.0).min_w(120.0).wrap())
            .child(btn_labeled(
                if s.preview_on { "Pause" } else { "Resume" },
                if s.preview_on { "Pause live preview" } else { "Resume live preview" },
                "preview-toggle", t, 84.0, bh,
                |s: &mut App| { s.preview_on = !s.preview_on; }
            )))
        .child(row().w_full().center().child(box_el));
    if !has_picture {
        pane = pane.child(text("Pointing the camera? Presets recall the exact framing — the preview streams at up to 30 fps, just for aiming.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary));
    }
    pane
}

fn control_page(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    let bh = if touch { 44.0 } else { 32.0 };
    col().key("control").w_full().px(16.0).py(14.0).gap(14.0)
        .child(camera_switcher(s, t, touch))
        .child(preview_pane(s, t, touch))
        .child(row().w_full().gap(14.0).flex_wrap()
            .child(ptz_pad(t, touch))
            .child(col().gap(10.0).grow(1.0).min_w(240.0)
                .child(section_title(t, "Zoom"))
                .child(row().w_full().gap(8.0).flex_wrap()
                    .child(btn_flex("Wide −", "Zoom wide out", "zoom-out", t, 72.0, bh, |s: &mut App| actions::zoom(s, false)))
                    .child(btn_flex("Stop", "Stop zoom", "zoom-stop", t, 56.0, bh, actions::zoom_stop))
                    .child(btn_flex("Tele +", "Zoom telephoto in", "zoom-in", t, 72.0, bh, |s: &mut App| actions::zoom(s, true))))
                .child(section_title(t, "Focus"))
                .child(row().w_full().gap(8.0).flex_wrap()
                    .child(btn_flex("Near", "Focus near", "focus-near", t, 64.0, bh, |s: &mut App| actions::focus(s, false)))
                    .child(btn_flex("Stop", "Stop focus", "focus-stop", t, 56.0, bh, actions::focus_stop))
                    .child(btn_flex("Far", "Focus far", "focus-far", t, 64.0, bh, |s: &mut App| actions::focus(s, true))))
                .child(row().w_full().gap(8.0).flex_wrap()
                    .child(btn_flex("Auto", "Auto focus", "af-auto", t, 64.0, bh, |s: &mut App| actions::focus_mode(s, 0)))
                    .child(btn_flex("Manual", "Manual focus", "af-man", t, 68.0, bh, |s: &mut App| actions::focus_mode(s, 1)))
                    .child(btn_flex("1-Push", "One-push focus", "af-push", t, 68.0, bh, |s: &mut App| actions::focus_mode(s, 2))))
                .child(section_title(t, "Power"))
                .child(row().w_full().gap(8.0).flex_wrap()
                    .child(accent_btn_flex("Power ON", "Power ON", "power-on", dark, 96.0, bh, |s: &mut App| actions::power(s, true)))
                    .child(btn_flex("Standby", "Standby", "power-off", t, 88.0, bh, |s: &mut App| actions::power(s, false))))
            )
            .child(col().gap(8.0).grow(1.0).min_w(240.0)
                .child(section_title(t, "Speeds"))
                .child(stepper(t, touch, "pan-speed", "Pan speed", format!("{} (1-24)", s.pan_speed),
                    |s: &mut App| s.pan_speed = s.pan_speed.saturating_sub(1).max(1),
                    |s: &mut App| s.pan_speed = (s.pan_speed + 1).min(24)))
                .child(stepper(t, touch, "tilt-speed", "Tilt speed", format!("{} (1-20)", s.tilt_speed),
                    |s: &mut App| s.tilt_speed = s.tilt_speed.saturating_sub(1).max(1),
                    |s: &mut App| s.tilt_speed = (s.tilt_speed + 1).min(20)))
                .child(stepper(t, touch, "zoom-speed", "Zoom speed", format!("{} (0-7)", s.zoom_speed),
                    |s: &mut App| s.zoom_speed = s.zoom_speed.saturating_sub(1),
                    |s: &mut App| s.zoom_speed = (s.zoom_speed + 1).min(7)))
                .child(section_title(t, "Presets (0-254, remote 0-9)"))
                .child(stepper(t, touch, "preset-sel", "Preset No.", format!("{}", s.preset_sel),
                    |s: &mut App| s.preset_sel = s.preset_sel.saturating_sub(1),
                    |s: &mut App| s.preset_sel = s.preset_sel.saturating_add(1).min(254)))
                .child(row().w_full().gap(8.0).flex_wrap()
                    .child(btn_flex("Recall", "Recall selected preset", "preset-recall", t, 72.0, bh, |s: &mut App| { let n = s.preset_sel; actions::preset(s, n, 0); }))
                    .child(accent_btn_flex("Set", "Save current position to selected preset", "preset-set", dark, 64.0, bh, |s: &mut App| { let n = s.preset_sel; actions::preset(s, n, 1); }))
                    .child(btn_flex("Clear", "Clear selected preset", "preset-clear", t, 64.0, bh, |s: &mut App| { let n = s.preset_sel; actions::preset(s, n, 2); })))
                .child(preset_grid(t, touch))
            )
        )
        .child(hint_card(t, "Tip: click an arrow to jog. Press Stop to halt. Home returns to center. Preset 0 is the power-on position if saved. Pan 40°/s, Tilt 30°/s, ±0.1° accuracy."))
}

fn camera_switcher(s: &App, t: rune::tok::Theme, touch: bool) -> El {
    let ch = if touch { 40.0 } else { 30.0 };
    let mut r = row().key("cams").w_full().min_h(40.0).py(4.0).gap(8.0).center_y().flex_wrap();
    r = r.child(text("Camera:".to_owned()).font(type_::BODY).color(t.text_secondary));
    for (i, cam) in s.cameras.iter().enumerate() {
        let on = i == s.selected;
        let label = format!("{} ({})", cam.name, cam.ip);
        r = r.child(
            el().key(format!("cam-{}", i)).h(ch).px(12.0).center().radius(ch * 0.5)
                .fill(if on { tok::accent(0, s.dark) } else { t.card })
                .border(1.0, t.card_border)
                .hover(t.hover).press(t.press).focusable(true)
                .role("tab").checked(on).label(label.clone())
                .on_press(move |s: &mut App| { s.selected = i; s.set_status(format!("Selected {}", s.current().name.clone())); })
                .child(text(if on { format!("● {}", cam.name) } else { cam.name.clone() })
                    .font(type_::BODY)
                    .color(if on { rune::Color::WHITE } else { t.text })
                    .a11y_hidden())
        );
    }
    r
}

fn ptz_pad(t: rune::tok::Theme, touch: bool) -> El {
    let bh = if touch { 52.0 } else { 44.0 };
    let stop_h = if touch { 46.0 } else { 34.0 };
    let b = |label: &'static str, a11y: &'static str, key: &'static str, dir: PtDir, tip: &'static str| {
        btn_flex(label, a11y, key, t, 52.0, bh, move |s: &mut App| actions::drive(s, dir, tip))
    };
    col().key("ptz").grow(1.0).min_w(196.0).max_w(340.0).gap(6.0)
        .child(section_title(t, "Pan / Tilt (drag-free jog)"))
        .child(row().w_full().gap(6.0)
            .child(b("↖", "Pan Up-Left", "pt-nw", PtDir::UpLeft, "Up-Left"))
            .child(b("↑", "Tilt Up", "pt-n", PtDir::Up, "Up"))
            .child(b("↗", "Pan Up-Right", "pt-ne", PtDir::UpRight, "Up-Right")))
        .child(row().w_full().gap(6.0)
            .child(b("←", "Pan Left", "pt-w", PtDir::Left, "Left"))
            .child(btn_flex("HOME", "Return camera to Home", "pt-home", t, 52.0, bh, actions::home))
            .child(b("→", "Pan Right", "pt-e", PtDir::Right, "Right")))
        .child(row().w_full().gap(6.0)
            .child(b("↙", "Pan Down-Left", "pt-sw", PtDir::DownLeft, "Down-Left"))
            .child(b("↓", "Tilt Down", "pt-s", PtDir::Down, "Down"))
            .child(b("↘", "Pan Down-Right", "pt-se", PtDir::DownRight, "Down-Right")))
        .child(row().w_full().gap(6.0)
            .child(accent_btn_flex("STOP", "Stop pan and tilt", "pt-stop", true, 168.0, stop_h, actions::stop)))
}

fn preset_grid(t: rune::tok::Theme, touch: bool) -> El {
    let ph = if touch { 44.0 } else { 30.0 };
    let mut c = col().key("preset-grid").w_full().gap(6.0);
    c = c.child(text("Quick recall 0-9:".to_owned()).font(type_::LABEL).color(t.text_secondary));
    for r in 0..2 {
        let mut row_el = row().key(format!("pr-{}", r)).w_full().gap(6.0);
        for col_i in 0..5 {
            let n = (r * 5 + col_i) as u8;
            row_el = row_el.child(btn_flex(&n.to_string(), &format!("Recall preset {}", n), &format!("q-{}", n), t, 36.0, ph,
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

fn image_page(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    col().key("image").w_full().px(16.0).py(14.0).gap(12.0)
        .child(text(format!("Image settings → {} @ {}", s.current().name, s.current().ip)).w_full().wrap().font(type_::TITLE).color(t.text))
        .child(text("Each row sends real VISCA to the camera. Values follow the FoMaKo manual §5.2.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
                .child(section_title(t, "Exposure"))
                .child(exposure_row(s, t, dark, touch))
                .child(stepper(t, touch, "shutter", "Shutter", format!("0x{:02X}", s.shutter),
                    |s: &mut App| s.shutter = s.shutter.saturating_sub(1),
                    |s: &mut App| s.shutter = s.shutter.saturating_add(1).min(20)))
                .child(apply_row(t, dark, touch, "shutter-a", "Shutter direct", "VISCA 4A", actions::apply_shutter))
                .child(stepper(t, touch, "iris", "Iris", format!("0x{:02X}", s.iris),
                    |s: &mut App| s.iris = s.iris.saturating_sub(1),
                    |s: &mut App| s.iris = s.iris.saturating_add(1).min(20)))
                .child(apply_row(t, dark, touch, "iris-a", "Iris direct", "VISCA 4B", actions::apply_iris))
                .child(stepper(t, touch, "gain", "Gain limit", format!("{}", s.gain_limit),
                    |s: &mut App| s.gain_limit = s.gain_limit.saturating_sub(1),
                    |s: &mut App| s.gain_limit = s.gain_limit.saturating_add(1).min(15)))
                .child(apply_row(t, dark, touch, "gain-a", "Gain limit", "VISCA 2C", actions::apply_gain_limit))
                .child(stepper(t, touch, "brightlvl", "Bright level", format!("{}", s.bright_lvl),
                    |s: &mut App| s.bright_lvl = s.bright_lvl.saturating_sub(1),
                    |s: &mut App| s.bright_lvl = s.bright_lvl.saturating_add(1).min(23)))
                .child(apply_row(t, dark, touch, "brightlvl-a", "Bright", "VISCA 4D", actions::apply_bright_lvl))
                .child(section_title(t, "Backlight / WDR / NR"))
                .child(apply_row(t, dark, touch, "blc", "Backlight", if s.backlight { "ON" } else { "OFF" }, |s: &mut App| { s.backlight = !s.backlight; actions::apply_backlight(s); }))
                .child(stepper(t, touch, "wdr", "WDR level", format!("{}", s.wdr),
                    |s: &mut App| s.wdr = s.wdr.saturating_sub(1),
                    |s: &mut App| s.wdr = s.wdr.saturating_add(1).min(7)))
                .child(apply_row(t, dark, touch, "wdr-a", "WDR", "VISCA 51", actions::apply_wdr))
                .child(stepper(t, touch, "nr2d", "NR 2D (0-7)", format!("{}", s.nr2d),
                    |s: &mut App| s.nr2d = s.nr2d.saturating_sub(1),
                    |s: &mut App| s.nr2d = s.nr2d.saturating_add(1).min(7)))
                .child(stepper(t, touch, "nr3d", "NR 3D (0-8)", format!("{}", s.nr3d),
                    |s: &mut App| s.nr3d = s.nr3d.saturating_sub(1),
                    |s: &mut App| s.nr3d = s.nr3d.saturating_add(1).min(8)))
                .child(apply_row(t, dark, touch, "nr-a", "Noise reduction", "VISCA 53/54", actions::apply_nr))
                .child(section_title(t, "White balance / color"))
                .child(wb_row(s, t, dark, touch))
                .child(stepper(t, touch, "rgain", "R Gain", format!("{}", s.r_gain),
                    |s: &mut App| s.r_gain = s.r_gain.saturating_sub(1),
                    |s: &mut App| s.r_gain = s.r_gain.saturating_add(1)))
                .child(stepper(t, touch, "bgain", "B Gain", format!("{}", s.b_gain),
                    |s: &mut App| s.b_gain = s.b_gain.saturating_sub(1),
                    |s: &mut App| s.b_gain = s.b_gain.saturating_add(1)))
                .child(apply_row(t, dark, touch, "gains", "R/B Gain", "VISCA 43/44", actions::apply_gains))
                .child(section_title(t, "Picture (brightness / contrast / saturation / sharpness)"))
                .child(stepper(t, touch, "brightness", "Brightness", format!("{}", s.brightness),
                    |s: &mut App| s.brightness = s.brightness.saturating_sub(1),
                    |s: &mut App| s.brightness = (s.brightness + 1).min(14)))
                .child(apply_row(t, dark, touch, "brightness-a", "Brightness", "VISCA A1", actions::apply_brightness))
                .child(stepper(t, touch, "contrast", "Contrast", format!("{}", s.contrast),
                    |s: &mut App| s.contrast = s.contrast.saturating_sub(1),
                    |s: &mut App| s.contrast = (s.contrast + 1).min(14)))
                .child(apply_row(t, dark, touch, "contrast-a", "Contrast", "VISCA A2", actions::apply_contrast))
                .child(stepper(t, touch, "sat", "Saturation 0-14", format!("{} (60-200%)", s.saturation),
                    |s: &mut App| s.saturation = s.saturation.saturating_sub(1),
                    |s: &mut App| s.saturation = (s.saturation + 1).min(14)))
                .child(apply_row(t, dark, touch, "sat-a", "Saturation", "VISCA 49", actions::apply_saturation))
                .child(stepper(t, touch, "aperture", "Sharpness 0-15", format!("{}", s.aperture),
                    |s: &mut App| s.aperture = s.aperture.saturating_sub(1),
                    |s: &mut App| s.aperture = (s.aperture + 1).min(15)))
                .child(apply_row(t, dark, touch, "aperture-a", "Sharpness", "VISCA 42", actions::apply_aperture))
                .child(section_title(t, "Misc"))
                .child(stepper(t, touch, "gamma", "Gamma 0-4", format!("{}", s.gamma),
                    |s: &mut App| s.gamma = s.gamma.saturating_sub(1),
                    |s: &mut App| s.gamma = (s.gamma + 1).min(4)))
                .child(apply_row(t, dark, touch, "gamma-a", "Gamma", "VISCA 5B", actions::apply_gamma))
                .child(apply_row(t, dark, touch, "flicker", "Flicker", match s.flicker { 1 => "50Hz", 2 => "60Hz", _ => "OFF" }, |s: &mut App| { s.flicker = (s.flicker + 1) % 3; actions::apply_flicker(s); }))
                .child(apply_row(t, dark, touch, "lowlight", "Low-light", if s.lowlight { "ON" } else { "OFF" }, |s: &mut App| { s.lowlight = !s.lowlight; actions::apply_lowlight(s); }))
                .child(apply_row(t, dark, touch, "flip", "Flip", match s.flip { 1 => "H", 2 => "V", 3 => "HV", _ => "OFF" }, |s: &mut App| { s.flip = (s.flip + 1) % 4; actions::apply_flip(s); }))
                .child(apply_row(t, dark, touch, "bw", "B/W mode", if s.bw { "B&W" } else { "Color" }, |s: &mut App| { s.bw = !s.bw; actions::apply_bw(s); }))
}

fn exposure_row(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    let modes = ["Auto", "Manual", "Shutter", "Iris", "Bright"];
    let ch = if touch { 40.0 } else { 28.0 };
    let mut r = row().key("ae-row").w_full().gap(6.0).center_y().flex_wrap();
    r = r.child(text(format!("Mode: {}", s.ae_label())).font(type_::BODY).color(t.text).w(130.0));
    for (i, m) in modes.iter().enumerate() {
        let on = s.ae_mode as usize == i;
        let label = m.to_string();
        r = r.child(
            el().key(format!("ae-{}", i)).h(ch).px(12.0).center().radius(ch * 0.5)
                .fill(if on { tok::accent(0, dark) } else { t.card })
                .hover(t.hover).focusable(true)
                .role("radio").checked(on).label(format!("Exposure {}", label))
                .on_press(move |s: &mut App| { s.ae_mode = i as u8; actions::apply_ae(s); })
                .child(text(label).font(type_::LABEL).color(if on { rune::Color::WHITE } else { t.text }).a11y_hidden())
        );
    }
    r = r.child(accent_btn_labeled("Send", "Send exposure mode", "ae-send", dark, 64.0, ch, actions::apply_ae));
    r
}

fn wb_row(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    let modes = ["Auto", "3000K", "4000K", "1-Push", "Manual", "6500K"];
    let map = [0u8, 1, 2, 3, 5, 6];
    let ch = if touch { 40.0 } else { 26.0 };
    let mut r = row().key("wb-row").w_full().gap(6.0).center_y().flex_wrap();
    r = r.child(text(format!("WB: {}", s.wb_label())).font(type_::BODY).color(t.text).w(130.0));
    for (idx, m) in modes.iter().enumerate() {
        let v = map[idx];
        let on = s.wb_mode == v;
        let label = m.to_string();
        r = r.child(
            el().key(format!("wb-{}", v)).h(ch).px(10.0).center().radius(ch * 0.5)
                .fill(if on { tok::accent(0, dark) } else { t.card })
                .hover(t.hover).focusable(true)
                .role("radio").checked(on).label(format!("White balance {}", label))
                .on_press(move |s: &mut App| { s.wb_mode = v; actions::apply_wb(s); })
                .child(text(label).font(type_::LABEL).color(if on { rune::Color::WHITE } else { t.text }).a11y_hidden())
        );
    }
    r
}

// ---------------------------------------------------------------- picture edit

fn picture_page(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    let bh = if touch { 44.0 } else { 34.0 };
    let (b, c, sat, sharp) = actions::picture_edit_targets(s.pic_bright, s.pic_contrast, s.pic_sat, s.pic_sharp);
    col().key("picture").w_full().px(16.0).py(14.0).gap(12.0)
        .child(text("Picture Edit — preview, then Apply to Camera".to_owned()).w_full().wrap().font(type_::TITLE).color(t.text))
        .child(text("Tune the look like a photo editor. The preview below simulates the change live. Apply sends Brightness/Contrast/Saturation/Sharpness (VISCA A1/A2/49/42) to the selected camera.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        .child(preview_pane(s, t, touch))
        .child(col().w_full().gap(16.0)
            .child(col().w_full().gap(8.0)
                .child(section_title(t, "Adjust"))
                .child(pic_slider(t, touch, "pic-b", "Brightness", s.pic_bright))
                .child(pic_slider(t, touch, "pic-c", "Contrast", s.pic_contrast))
                .child(pic_slider(t, touch, "pic-s", "Saturation", s.pic_sat))
                .child(pic_slider(t, touch, "pic-sh", "Sharpness", s.pic_sharp))
                .child(row().gap(8.0).flex_wrap()
                    .child(accent_btn("Apply to Camera", "pic-apply", dark, 150.0, bh, actions::apply_picture_edit))
                    .child(btn_labeled("Reset", "Reset picture sliders", "pic-reset", t, 84.0, bh, |s: &mut App| {
                        s.pic_bright = 0; s.pic_contrast = 0; s.pic_sat = 0; s.pic_sharp = 0;
                        s.set_status("Picture sliders reset (camera unchanged)".into());
                    })))
                .child(hint_card(t, "Mapping: ±100 → Bright 0-14, Contrast 0-14, Sat 0-14 (60-200%), Sharp 0-15. Center = camera 7/7/4/8 (100% saturation)."))
            )
            .child(col().w_full().gap(8.0)
                .child(section_title(t, "Live simulated preview"))
                .child(preview_bars(s, t))
                .child(preview_gray(s, t))
                .child(text(format!("Will send: Bright {}  Contrast {}  Sat {}  Sharp {}", b, c, sat, sharp))
                    .w_full().wrap().font(type_::LABEL).color(t.text_secondary))
            )
        )
}

fn pic_slider(t: rune::tok::Theme, touch: bool, key: &'static str, label: &str, v: i32) -> El {
    let minus_key = format!("{}-minus", key);
    let plus_key = format!("{}-plus", key);
    let k = key.to_owned();
    let l = label.to_owned();
    let bw = if touch { 44.0 } else { 34.0 };
    let bh = if touch { 44.0 } else { 28.0 };
    let frac = ((v + 100) as f32 / 200.0).clamp(0.0, 1.0);
    row().key(k).w_full().min_h(bh + 6.0).py(2.0).center_y().gap(8.0).flex_wrap()
        .child(text(l).font(type_::BODY).color(t.text).min_w(88.0))
        .child(btn_labeled("−", &format!("Decrease {}", label), &minus_key, t, bw, bh, move |s: &mut App| {
            match key {
                "pic-b" => s.pic_bright = (s.pic_bright - 5).max(-100),
                "pic-c" => s.pic_contrast = (s.pic_contrast - 5).max(-100),
                "pic-s" => s.pic_sat = (s.pic_sat - 5).max(-100),
                _ => s.pic_sharp = (s.pic_sharp - 5).max(-100),
            }
        }))
        .child(text(format!("{:+}", v)).font(type_::MONO_CAPTION).color(t.text).min_w(44.0))
        .child(btn_labeled("+", &format!("Increase {}", label), &plus_key, t, bw, bh, move |s: &mut App| {
            match key {
                "pic-b" => s.pic_bright = (s.pic_bright + 5).min(100),
                "pic-c" => s.pic_contrast = (s.pic_contrast + 5).min(100),
                "pic-s" => s.pic_sat = (s.pic_sat + 5).min(100),
                _ => s.pic_sharp = (s.pic_sharp + 5).min(100),
            }
        }))
        .child(el().grow(1.0).min_w(80.0).max_w(220.0).h(8.0).radius(4.0).fill(t.control_track)
            .child(el().w_frac(frac).h(8.0).radius(4.0).fill(tok::accent(0, true))))
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

fn cameras_page(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    col().key("cameras").w_full().px(16.0).py(14.0).gap(12.0)
        .child(text("Cameras — switch, add, remove".to_owned()).w_full().wrap().font(type_::TITLE).color(t.text))
        .child(text("Stored in fomako-cameras.json next to the app (USB-stick portable). Default: admin/admin. FoMaKo LCD shows the IP.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        .child(camera_list(s, t, touch))
        .child(creds_section(s, t, touch))
        .child(scan_section(s, t, dark, touch))
        .child(relay_section(s, t, dark, touch))
        .child(add_form(s, t, dark, touch))
        .child(stream_help(s, t))
}

fn camera_list(s: &App, t: rune::tok::Theme, touch: bool) -> El {
    let bh = if touch { 42.0 } else { 28.0 };
    let mut c = col().key("cam-list").w_full().gap(6.0);
    c = c.child(section_title(t, &format!("Saved ({}). Tap Select to switch.", s.cameras.len())));
    for (i, cam) in s.cameras.iter().enumerate() {
        let on = i == s.selected;
        let name = cam.name.clone();
        let ip = cam.ip.clone();
        c = c.child(
            row().key(format!("camrow-{}", i)).w_full().min_h(44.0).py(6.0).px(12.0).gap(10.0).center_y().flex_wrap()
                .radius(8.0).fill(if on { t.control_track } else { t.card }).border(1.0, t.card_border)
                .child(text(format!("{} #{}  {}  {}:{}", if on { "●" } else { "○" }, cam.address, name, ip, cam.protocol.port()))
                    .font(type_::BODY).color(t.text).grow(1.0).min_w(160.0).wrap())
                .child(btn_labeled("Select", &format!("Select {}", name), &format!("sel-{}", i), t, 74.0, bh, move |s: &mut App| {
                    s.selected = i;
                    s.set_status(format!("Selected {}", s.current().name.clone()));
                }))
                .child(btn_labeled("Test", &format!("Test {}", name), &format!("test-{}", i), t, 64.0, bh, move |s: &mut App| {
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
            .child(btn("Remove selected", "cam-remove", t, 148.0, if touch { 42.0 } else { 30.0 }, |s: &mut App| {
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

fn creds_section(s: &App, t: rune::tok::Theme, touch: bool) -> El {
    // Edit box mirrors the selected camera; Save writes it back.
    let (name, user) = {
        let c = s.current();
        (c.name.clone(), c.username.clone())
    };
    let bh = if touch { 42.0 } else { 30.0 };
    col().key("creds").w_full().gap(8.0)
        .child(section_title(t, &format!("Login for {} (RTSP/web; VISCA needs none)", name)))
        .child(row().w_full().gap(8.0).center_y().flex_wrap()
            .child(text("User:".to_owned()).font(type_::BODY).color(t.text).w(48.0))
            .child(text_input(&s.user_edit, field_style()).single_line().key("cred-user").grow(1.0).min_w(100.0)
                .placeholder(user.clone(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.user_edit))
            .child(text("Pass:".to_owned()).font(type_::BODY).color(t.text).w(46.0))
            .child(text_input(&s.pass_edit, field_style()).single_line().secure().key("cred-pass").grow(1.0).min_w(100.0)
                .placeholder("••••••".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.pass_edit))
            .child(btn("Save login", "cred-save", t, 110.0, bh, move |s: &mut App| {
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

fn scan_section(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    let (running, done, total, msg) = match &s.scan {
        Some(sc) => (sc.running, sc.done, sc.total, sc.message.clone()),
        None => (false, 0, 254, String::new()),
    };
    let bh = if touch { 42.0 } else { 32.0 };
    let add_h = if touch { 42.0 } else { 28.0 };
    let mut c = col().key("scan").w_full().gap(8.0)
        .child(section_title(t, "Find cameras on my network (auto)"))
        .child(text("Phone/PC and camera must join the same WiFi/LAN. Desktop scans directly; the browser build scans through the relay below. Cameras on another network? Add the IP manually underneath.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        .child(row().w_full().gap(8.0).center_y().flex_wrap()
            .child(text("Subnet:".to_owned()).font(type_::BODY).color(t.text).w(60.0))
            .child(text_input(&s.scan_prefix, field_style()).single_line().key("subnet-field").grow(1.0).min_w(104.0).max_w(180.0)
                .placeholder("192.168.1".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.scan_prefix))
            .child(text(".1–254".to_owned()).font(type_::BODY).color(t.text_secondary))
            .child(accent_btn_tx(if running { "Stop" } else { "Scan network" }, "scan-btn", dark, 140.0, bh,
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
                row().key(format!("found-{}", i)).w_full().min_h(40.0).py(6.0).px(12.0).gap(10.0).center_y().flex_wrap()
                    .radius(8.0).fill(t.card).border(1.0, t.card_border)
                    .child(text(line).font(type_::BODY).color(t.text).grow(1.0).min_w(160.0).wrap())
                    .child(btn_labeled("Add", &format!("Add camera {}", ip), &format!("add-found-{}", i), t, 68.0, add_h, move |s: &mut App| {
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

fn relay_section(s: &App, t: rune::tok::Theme, _dark: bool, touch: bool) -> El {
    let bh = if touch { 42.0 } else { 30.0 };
    col().key("relay").w_full().gap(8.0)
        .child(section_title(t, "Phone / browser control (LAN relay)"))
        .child(text("Browsers can't open camera sockets, so the phone build talks to a tiny relay on your LAN PC (python3 scripts/visca-relay.py --https --serve-web dist/web), which forwards to the cameras. Turn it on, paste the relay URL, then Scan / control works from a phone browser.".to_owned()).w_full().wrap().font(type_::LABEL).color(t.text_secondary))
        .child(row().w_full().gap(8.0).center_y().flex_wrap()
            .child(btn(if s.use_relay { "Relay: ON" } else { "Relay: OFF" }, "relay-toggle", t, 110.0, bh, |s: &mut App| {
                s.use_relay = !s.use_relay;
                s.set_status(if s.use_relay { "Relay ON — commands go via relay URL".into() } else { "Relay OFF — direct sockets".into() });
            }))
            .child(text_input(&s.relay_url, field_style()).single_line().key("relay-field").grow(1.0).min_w(180.0)
                .placeholder("https://192.168.1.10:8443".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.relay_url)))
}

fn add_form(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    let bh = if touch { 42.0 } else { 32.0 };
    col().key("add").w_full().gap(8.0)
        .child(section_title(t, "Add camera"))
        .child(row().w_full().gap(8.0).center_y().flex_wrap()
            .child(text("Name:".to_owned()).font(type_::BODY).color(t.text).w(56.0))
            .child(text_input(&s.name_edit, field_style()).single_line().key("name-field").grow(1.0).min_w(140.0).max_w(280.0)
                .placeholder("Pulpit".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.name_edit)))
        .child(row().w_full().gap(8.0).center_y().flex_wrap()
            .child(text("IP:".to_owned()).font(type_::BODY).color(t.text).w(56.0))
            .child(text_input(&s.ip_edit, field_style()).single_line().key("ip-field").grow(1.0).min_w(140.0).max_w(280.0)
                .placeholder("192.168.5.163".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.ip_edit))
            .child(text("Protocol: tap to toggle (UDP 52381 recommended)".to_owned()).font(type_::LABEL).color(t.text_secondary).wrap()))
        .child(row().w_full().gap(8.0).center_y().flex_wrap()
            .child(text("Login:".to_owned()).font(type_::BODY).color(t.text).w(56.0))
            .child(text_input(&s.user_edit, field_style()).single_line().key("user-field").grow(1.0).min_w(96.0).max_w(160.0)
                .placeholder("admin".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.user_edit))
            .child(text("/".to_owned()).font(type_::BODY).color(t.text_secondary))
            .child(text_input(&s.pass_edit, field_style()).single_line().secure().key("pass-field").grow(1.0).min_w(96.0).max_w(160.0)
                .placeholder("admin".to_owned(), t.text_secondary)
                .on_edit(|s: &mut App| &mut s.pass_edit))
            .child(text("(RTSP/web login; VISCA needs none)".to_owned()).font(type_::LABEL).color(t.text_secondary).wrap()))
        .child(row().w_full().gap(8.0).flex_wrap()
            .child(accent_btn_flex("Add Sony UDP :52381", "Add camera with Sony UDP 52381", "add-udp", dark, 150.0, bh, |s: &mut App| {
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
            .child(btn_flex("Add TCP :5678", "Add camera with TCP 5678", "add-tcp", t, 130.0, bh, |s: &mut App| {
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
        .child(text(format!("Main:  rtsp://{}:554/1   (or /live/av0)", ip)).w_full().wrap().font(type_::MONO_CAPTION).color(t.text))
        .child(text(format!("Sub:   rtsp://{}:554/2   (or /live/av1)", ip)).w_full().wrap().font(type_::MONO_CAPTION).color(t.text))
        .child(text(format!("RTMP:  rtmp://{}:1935/live/av0", ip)).w_full().wrap().font(type_::MONO_CAPTION).color(t.text))
        .child(text(format!("Web UI: http://{}  ({} / {})", ip, s.current().username, "•••")).w_full().wrap().font(type_::MONO_CAPTION).color(t.text))
        .child(hint_card(t, "The in-app preview streams live (~30 fps) from RTSP. For full-motion video elsewhere, keep VLC/OBS open beside the controller or paste the RTSP URL into OBS as a Media Source."))
}

// ---------------------------------------------------------------- help

fn help_page(s: &App, t: rune::tok::Theme, _dark: bool, touch: bool) -> El {
    let ffmpeg_line = if s.ffmpeg_path.is_empty() {
        "ffmpeg: NOT FOUND — the preview falls back to HTTP stills (slow); install ffmpeg for the 30 fps RTSP stream (apt install ffmpeg / brew install ffmpeg / winget install ffmpeg).".to_string()
    } else {
        format!("ffmpeg: found at {} — live RTSP streaming is available.", s.ffmpeg_path)
    };
    col().key("help").w_full().px(16.0).py(14.0).gap(10.0)
        .child(text("Help — church setup in 5 minutes".to_owned()).font(type_::TITLE).color(t.text))
        .child(update_card(s, t, _dark, touch))
        .child(help_card(t, "0. Live picture", &format!("The Control page streams the camera's RTSP sub stream (`/2` or `/live/av1`) through ffmpeg at up to 30 fps. The browser build gets the same stream from the relay. Without ffmpeg it falls back to HTTP stills, which are much slower. {ffmpeg_line}")))
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

/// App updates: version, what the updater is doing, and the one button that
/// matters right now. Installs only on a click; a restart is a separate click
/// so nobody's service gets interrupted by a download finishing.
fn update_card(s: &App, t: rune::tok::Theme, dark: bool, touch: bool) -> El {
    let bh = if touch { 44.0 } else { 30.0 };
    let mut actions = row().w_full().gap(8.0).center_y().flex_wrap();
    match s.update_phase {
        UpdatePhase::Checking => {
            actions = actions.child(btn_labeled(
                "Checking…", "Checking GitHub for updates", "update-checking",
                t, 132.0, bh, |_s: &mut App| {},
            ));
        }
        UpdatePhase::Downloading => {
            actions = actions.child(btn_labeled(
                "Downloading…", "Update download in progress", "update-busy",
                t, 150.0, bh, |_s: &mut App| {},
            ));
        }
        UpdatePhase::Available => {
            // Browsers get the new build when the relay serves it: reload, not swap.
            #[cfg(not(target_arch = "wasm32"))]
            let primary = accent_btn_labeled(
                &format!("Install v{}", s.update_version),
                "Download, verify and install the new version",
                "update-install", dark, 168.0, bh,
                |s: &mut App| update::start_install(s),
            );
            #[cfg(target_arch = "wasm32")]
            let primary = accent_btn_labeled(
                &format!("Reload for v{}", s.update_version),
                "Reload this page to pick up the new build",
                "update-install", dark, 168.0, bh,
                |s: &mut App| update::restart(s),
            );
            actions = actions.child(primary).child(btn_labeled(
                "Check again", "Check GitHub for updates again", "update-recheck",
                t, 120.0, bh, |s: &mut App| update::start_check(s),
            ));
        }
        UpdatePhase::Ready => {
            actions = actions.child(accent_btn_labeled(
                "Restart now", "Restart the app to use the new version", "update-restart",
                dark, 140.0, bh, |s: &mut App| update::restart(s),
            ));
        }
        UpdatePhase::Idle | UpdatePhase::Failed => {
            actions = actions.child(btn_labeled(
                "Check for updates", "Check GitHub for a newer release", "update-check",
                t, 168.0, bh, |s: &mut App| update::start_check(s),
            ));
        }
    }
    actions = actions.child(link_btn(
        "Release page", "Open the release page in your browser",
        "update-releases", update::RELEASES_URL, t, bh,
    ));

    let platform_note = if update::asset_name().is_none() {
        format!(
            "This build ({} / {}) has no one-click download — take it from the release page.",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    } else {
        "Installs over this copy only: cameras and fomako-cameras.json stay put. The app checks at launch and every 6 hours; it never installs or restarts on its own."
            .to_string()
    };

    col().key("help-app-updates").w_full().padding(12.0).gap(6.0).radius(8.0)
        .fill(t.card).border(1.0, t.card_border)
        .child(row().w_full().gap(6.0).center_y().flex_wrap()
            .child(text("App updates".to_owned()).font(type_::BODY).color(t.text))
            .child(text(format!("v{}", update::current())).font(type_::MONO_CAPTION).color(t.text_secondary)))
        .child(text(s.update_msg.clone()).w_full().wrap().role("status")
            .live_region(rune::LiveRegion::Polite).font(type_::LABEL).color(t.text))
        .child(actions)
        .child(text(platform_note).w_full().wrap().font(type_::MONO_CAPTION).color(t.text_secondary))
}
