//! FoMaKo Church PTZ Controller — Rune app.
//! Pan/tilt/zoom, presets, full image settings, picture-edit preview, multi-camera.

mod actions;
mod discover;
mod follow;
mod look;
mod model;
mod preview;
mod update;
mod visca;
mod view;

use model::App;
use rune::{AppSpec, Tx};

/// Called once with a waker other threads use when new data arrives.
fn setup(s: &mut App, waker: rune::Waker) {
    s.preview_cache = Some(rune::ImageCache::new(waker.clone()));
    s.preview_waker = Some(waker.clone());
    s.follow_waker = Some(waker.clone());
    s.update_waker = Some(waker);
}

/// Drain background network-scan results + preview frames; re-arm while busy.
fn poll(s: &mut App, clock: f64, tx: &mut Tx) -> bool {
    let mut changed = s.scan_poll(tx);
    changed |= s.follow_poll();
    changed |= crate::preview::poll(s, clock, tx);
    changed |= crate::actions::poll_look(s, clock);
    changed |= crate::update::poll(s, clock, tx);
    changed
}

fn main() {
    // Clear staging/`.old` leftovers from an earlier update before drawing anything.
    update::cleanup_stale();
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
        setup: Some(setup),
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
