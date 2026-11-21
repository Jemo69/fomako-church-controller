//! FoMaKo Church PTZ Controller — Rune app.
//! Pan/tilt/zoom, presets, full image settings, picture-edit preview, multi-camera.

mod actions;
mod discover;
mod model;
mod visca;
mod view;

use model::App;
use rune::{AppSpec, Tx};

/// Drain background network-scan results into state; re-arm while scanning.
fn poll(s: &mut App, _clock: f64, tx: &mut Tx) -> bool {
    s.scan_poll(tx)
}

fn main() {
    let spec = AppSpec {
        scenes: &[
            ("control", |s: &mut App| s.page = model::Page::Control),
            ("image", |s: &mut App| s.page = model::Page::Image),
            ("picture", |s: &mut App| s.page = model::Page::Picture),
            ("cameras", |s: &mut App| s.page = model::Page::Cameras),
            ("help", |s: &mut App| s.page = model::Page::Help),
            ("light", |s: &mut App| { s.page = model::Page::Control; s.dark = false; }),
            ("scan", |s: &mut App| s.page = model::Page::Cameras),
        ],
        poll: Some(poll),
        ..AppSpec::new(
            "FoMaKo Church Controller",
            (1120.0, 720.0),
            App::new,
            view::view,
        )
    };
    rune::run(spec);
}
