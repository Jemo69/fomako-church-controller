//! App model: cameras, pages, image settings, persistence.

use std::sync::{Arc, Mutex, atomic::AtomicU64};

use rune::TextEdit;
use serde::{Deserialize, Serialize};

use crate::discover::{FoundCamera, ScanEvent, ScanHandle};
use crate::update::{UpdateEvent, UpdatePhase};
use crate::visca::Protocol;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Control,
    Image,
    Picture,
    Cameras,
    Help,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Control => "Control",
            Page::Image => "Image",
            Page::Picture => "Picture Edit",
            Page::Cameras => "Cameras",
            Page::Help => "Help",
        }
    }
    pub fn all() -> [Page; 5] {
        [Page::Control, Page::Image, Page::Picture, Page::Cameras, Page::Help]
    }
}

fn default_user() -> String {
    "admin".to_string()
}
fn default_address() -> u8 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraConfig {
    pub name: String,
    pub ip: String,
    #[serde(default)]
    pub protocol: ProtoSer,
    #[serde(default = "default_address")]
    pub address: u8,
    /// Web/RTSP login (FoMaKo default admin/admin). VISCA itself needs no password.
    #[serde(default = "default_user")]
    pub username: String,
    #[serde(default = "default_user")]
    pub password: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ProtoSer {
    #[default]
    SonyUdp,
    Tcp,
}

impl ProtoSer {
    pub fn to_protocol(self) -> Protocol {
        match self {
            ProtoSer::SonyUdp => Protocol::SonyUdp,
            ProtoSer::Tcp => Protocol::IpViscaTcp,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ProtoSer::SonyUdp => "Sony VISCA UDP :52381",
            ProtoSer::Tcp => "IP-VISCA TCP :5678",
        }
    }
    pub fn port(self) -> u16 {
        match self {
            ProtoSer::SonyUdp => 52381,
            ProtoSer::Tcp => 5678,
        }
    }
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig {
            name: "Pulpit".into(),
            ip: "192.168.5.163".into(),
            protocol: ProtoSer::SonyUdp,
            address: 1,
            username: default_user(),
            password: default_user(),
        }
    }
}

pub struct App {
    pub cameras: Vec<CameraConfig>,
    pub selected: usize,
    pub page: Page,
    pub dark: bool,

    pub pan_speed: u8,
    pub tilt_speed: u8,
    pub zoom_speed: u8,

    pub status: String,
    pub log: Vec<String>,

    // camera form edits
    pub name_edit: TextEdit,
    pub ip_edit: TextEdit,
    pub user_edit: TextEdit,
    pub pass_edit: TextEdit,

    // image settings (UI state, applied to camera on demand)
    pub ae_mode: u8, // 0 auto,1 manual,2 shutter,3 iris,4 bright
    pub shutter: u8,
    pub iris: u8,
    pub gain_limit: u8,
    pub bright_lvl: u8,
    pub expcomp_on: bool,
    pub expcomp_lvl: u8,
    pub backlight: bool,
    pub wdr: u8,
    pub nr2d: u8,
    pub nr3d: u8,
    pub flicker: u8, // 0 off 1 50Hz 2 60Hz
    pub gamma: u8,
    pub lowlight: bool,
    pub aperture: u8,   // sharpness 0..15
    pub brightness: u8,  // 0..14 (maps to 0x00..0x0E scaled)
    pub contrast: u8,   // 0..14
    pub saturation: u8,  // 0..14 (60%..200%)
    pub wb_mode: u8,     // 0 auto .. 11
    pub awb_sens: u8,    // 0 low 1 normal 2 high
    pub r_gain: u8,
    pub b_gain: u8,
    pub flip: u8, // 0 off 1 H 2 V 3 HV
    pub bw: bool,

    // picture-edit preview modifiers (0..100)
    pub pic_bright: i32,
    pub pic_contrast: i32,
    pub pic_sat: i32,
    pub pic_sharp: i32,
    pub preset_sel: u8,

    // Camera-side AI tracking (FoMaKo AI-tracking models, manual §5.2)..
    pub track_on: bool,
    /// false = real-time tracking, true = area tracking.
    pub track_area: bool,
    pub track_status: String,

    // "Match this picture" (src/look.rs): a reference image drives the camera's
    // picture settings. Independent of the camera's AI tracking above.
    pub look_target: Option<crate::look::Stats>,
    pub look_live: Option<crate::look::Stats>,
    pub look_label: String,
    pub look_status: String,
    pub look_image: Option<rune::Image>,
    pub look_bytes: Option<std::sync::Arc<[u8]>>,
    pub look_path: TextEdit,
    /// Pictures found in ./looks, shown as one-tap buttons.
    pub look_files: Vec<String>,
    /// Continuously nudge the camera toward the reference look.
    pub look_auto: bool,
    /// Also match colour with R/B gain (switches WB to Manual).
    pub look_color: bool,
    pub look_next_at: f64,

    // network auto-discovery
    pub scan_prefix: TextEdit,
    pub scan: Option<ScanState>,
    // LAN relay for phone/browser builds (see scripts/visca-relay.py)
    pub relay_url: TextEdit,
    pub use_relay: bool,
    /// One-shot: start a scan on first poll (lets a QR link open straight into a scan).
    /// Read on web; kept on desktop for shape parity.
    #[allow(dead_code)]
    pub autostart_scan: bool,
    // Live preview (HTTP JPEG natively, relay snapshots on web).
    pub preview_on: bool,
    pub preview_ip: String,
    pub preview_user: String,
    pub preview_pass: String,
    // Bumped to stop the native ffmpeg thread; web drives fetches from poll instead.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub preview_gen: Arc<AtomicU64>,
    pub preview_slot: Arc<Mutex<Option<crate::preview::PreviewUpdate>>>,
    pub preview_cache: Option<rune::ImageCache>,
    pub preview_waker: Option<rune::Waker>,
    pub preview_image: Option<rune::Image>,
    /// The frame currently decoding; promoted to `preview_image` when ready.
    pub preview_pending: Option<rune::Image>,
    pub preview_status: String,
    pub preview_error: String,
    pub preview_frames: u64,
    pub preview_ready: bool,
    /// Decoded-frame counter and clock for the live fps readout.
    pub preview_fps: f32,
    pub preview_fps_frames: u64,
    pub preview_fps_since: f64,
    /// Resolved once at startup for the Help page; the fetcher re-resolves
    /// every grab so a later install is picked up without a restart.
    pub ffmpeg_path: String,
    /// Where the preview box was drawn last (web: positions the MJPEG overlay).
    #[cfg(target_arch = "wasm32")]
    pub preview_rect: Option<[f32; 4]>,
    /// Stream URL currently loaded into the web overlay; empty when hidden.
    #[cfg(target_arch = "wasm32")]
    pub preview_overlay_url: String,
    #[cfg(target_arch = "wasm32")]
    pub action_slot: Arc<Mutex<Option<ActionReply>>>,
    #[cfg(target_arch = "wasm32")]
    pub action_waker: Option<rune::Waker>,

    // Auto-updater (see src/update.rs): check GitHub releases, install on click.
    pub update_phase: UpdatePhase,
    pub update_msg: String,
    /// Version offered by the latest release (empty until a check finds one).
    pub update_version: String,
    pub update_url: String,
    pub update_sums_url: String,
    pub update_release_url: String,
    pub update_asset: String,
    /// Bytes downloaded so far (shown during download).
    pub update_done: u64,
    /// Path of the installed binary, once the swap happened.
    pub update_ready_path: String,
    /// Seconds on the poll clock; 0 disables the scheduled check.
    pub update_next_check: f64,
    pub update_slot: Arc<Mutex<Vec<UpdateEvent>>>,
    pub update_waker: Option<rune::Waker>,

    // Cross-camera follow: leader aim → follower pan/tilt (src/follow.rs).
    pub follow_on: bool,
    pub follow_leader: usize,
    pub follow_follower: usize,
    pub follow_pan_offset: i32,
    pub follow_tilt_offset: i32,
    pub follow_status: String,
    pub follow_ready: bool,
    pub follow_gen: Arc<AtomicU64>,
    pub follow_slot: Arc<Mutex<Vec<crate::follow::FollowEvent>>>,
    pub follow_waker: Option<rune::Waker>,
}

#[derive(Clone, Debug)]
pub struct ActionReply {
    pub what: String,
    pub result: Result<String, String>,
}

#[derive(Clone, Debug)]
pub struct ScanResult {
    pub ip: String,
    pub detail: String,
    pub tcp5678: bool,
    pub udp52381: bool,
}

pub struct ScanState {
    pub running: bool,
    pub via_relay: bool,
    pub prefix: String,
    pub done: usize,
    pub total: usize,
    pub results: Vec<ScanResult>,
    pub message: String,
    pub queue: Arc<Mutex<Vec<ScanEvent>>>,
    pub handle: Arc<ScanHandle>,
}

impl App {
    pub fn new() -> Self {
        let cameras = load_cameras();
        let saved = crate::follow::load_saved();
        let (follow_leader, follow_follower, follow_pan_offset, follow_tilt_offset, follow_ready) = match &saved {
            Some(sv) if cameras.len() > 1 => {
                let leader = cameras.iter().position(|c| c.ip == sv.leader_ip);
                let follower = cameras.iter().position(|c| c.ip == sv.follower_ip);
                match (leader, follower) {
                    (Some(l), Some(f)) if l != f => (
                        l,
                        f,
                        sv.pan_offset.clamp(-crate::follow::OFFSET_MAX, crate::follow::OFFSET_MAX),
                        sv.tilt_offset.clamp(-crate::follow::OFFSET_MAX, crate::follow::OFFSET_MAX),
                        true,
                    ),
                    _ => (0, 1, 0, 0, false),
                }
            }
            _ => (0, if cameras.len() > 1 { 1 } else { 0 }, 0, 0, false),
        };
        let mut s = App {
            cameras,
            selected: 0,
            page: Page::Control,
            dark: true,
            pan_speed: 14,
            tilt_speed: 12,
            zoom_speed: 4,
            status: "Ready. Pick a camera, then drive it.".into(),
            log: vec![],
            name_edit: TextEdit::new("New Camera"),
            ip_edit: TextEdit::new("192.168.5.163"),
            user_edit: TextEdit::new(""),
            pass_edit: TextEdit::new(""),
            ae_mode: 0,
            shutter: 10,
            iris: 8,
            gain_limit: 8,
            bright_lvl: 8,
            expcomp_on: false,
            expcomp_lvl: 7,
            backlight: false,
            wdr: 0,
            nr2d: 3,
            nr3d: 5,
            flicker: 0,
            gamma: 2,
            lowlight: false,
            aperture: 8,
            brightness: 7,
            contrast: 7,
            saturation: 4,
            wb_mode: 0,
            awb_sens: 1,
            r_gain: 32,
            b_gain: 32,
            flip: 0,
            bw: false,
            pic_bright: 0,
            pic_contrast: 0,
            pic_sat: 0,
            pic_sharp: 0,
            preset_sel: 1,
            track_on: false,
            track_area: false,
            track_status: "Off — camera aims manually".to_string(),
            look_target: None,
            look_live: None,
            look_label: String::new(),
            look_status: "Load a picture and the app tunes the camera to match it".to_string(),
            look_image: None,
            look_bytes: None,
            look_path: TextEdit::new(""),
            look_files: Vec::new(),
            look_auto: false,
            look_color: false,
            look_next_at: 0.0,
            scan_prefix: TextEdit::new(crate::discover::detect_prefix()),
            scan: None,
            relay_url: TextEdit::new(""),
            use_relay: false,
            autostart_scan: false,
            preview_on: true,
            preview_ip: String::new(),
            preview_user: String::new(),
            preview_pass: String::new(),
            preview_gen: Arc::new(AtomicU64::new(0)),
            preview_slot: Arc::new(Mutex::new(None)),
            preview_cache: None,
            preview_waker: None,
            preview_image: None,
            preview_pending: None,
            preview_status: "Preview starting…".to_string(),
            preview_error: String::new(),
            preview_frames: 0,
            preview_ready: false,
            preview_fps: 0.0,
            preview_fps_frames: 0,
            preview_fps_since: 0.0,
            ffmpeg_path: crate::preview::ffmpeg_path_or_empty(),
            #[cfg(target_arch = "wasm32")]
            preview_rect: None,
            #[cfg(target_arch = "wasm32")]
            preview_overlay_url: String::new(),
            #[cfg(target_arch = "wasm32")]
            action_slot: Arc::new(Mutex::new(None)),
            #[cfg(target_arch = "wasm32")]
            action_waker: None,
            update_phase: UpdatePhase::Idle,
            update_msg: "Not checked yet — the app looks for a newer release a few seconds after launch."
                .to_string(),
            update_version: String::new(),
            update_url: String::new(),
            update_sums_url: String::new(),
            update_release_url: crate::update::RELEASES_URL.to_string(),
            update_asset: String::new(),
            update_done: 0,
            update_ready_path: String::new(),
            update_next_check: 4.0,
            update_slot: Arc::new(Mutex::new(Vec::new())),
            update_waker: None,
            follow_on: false,
            follow_leader,
            follow_follower,
            follow_pan_offset,
            follow_tilt_offset,
            follow_status: "Idle — calibrate both cameras on one landmark, then Start.".to_string(),
            follow_ready,
            follow_gen: Arc::new(AtomicU64::new(0)),
            follow_slot: Arc::new(Mutex::new(Vec::new())),
            follow_waker: None,
        };
        if s.cameras.is_empty() {
            s.cameras.push(CameraConfig::default());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            s.look_files = crate::look::list_pictures("looks");
        }
        if s.follow_ready {
            s.follow_status = format!(
                "Ready — saved offset pan {:+}, tilt {:+} steps. Start when ready.",
                s.follow_pan_offset, s.follow_tilt_offset
            );
        }
        s.push_log("FoMaKo controller started (VISCA 52381 / 5678, admin/admin)".into());
        s
    }

    pub fn current(&self) -> &CameraConfig {
        &self.cameras[self.selected.min(self.cameras.len().saturating_sub(1))]
    }

    /// Switch cameras. AI tracking lives in the camera, so selecting another one
    /// always leaves tracking off until it is switched on again.
    pub fn select_camera(&mut self, i: usize) {
        self.selected = i.min(self.cameras.len().saturating_sub(1));
        self.track_on = false;
        self.track_status = "Off — camera aims manually".to_string();
    }

    pub fn push_log(&mut self, msg: String) {
        self.log.push(msg);
        if self.log.len() > 40 {
            let excess = self.log.len() - 40;
            self.log.drain(0..excess);
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status = msg.clone();
        self.push_log(msg);
    }

    pub fn save(&self) {
        save_cameras(&self.cameras);
    }

    pub fn ae_byte(&self) -> u8 {
        match self.ae_mode {
            1 => 0x03,
            2 => 0x0A,
            3 => 0x0B,
            4 => 0x0D,
            _ => 0x00,
        }
    }
    pub fn ae_label(&self) -> &'static str {
        match self.ae_mode {
            1 => "Manual",
            2 => "Shutter Pri",
            3 => "Iris Pri",
            4 => "Bright",
            _ => "Full Auto",
        }
    }
    pub fn wb_label(&self) -> &'static str {
        match self.wb_mode {
            0 => "Auto",
            1 => "3000K",
            2 => "4000K",
            3 => "OnePush",
            4 => "OnePushTrig",
            5 => "Manual",
            6 => "6500K",
            7 => "3500K",
            8 => "4500K",
            9 => "5500K",
            10 => "6000K",
            11 => "7000K",
            _ => "?",
        }
    }
    pub fn wb_byte(&self) -> u8 {
        // Manual table mapping (same order as inquiry list):
        // 00 Auto, 01 3000K? Actually manual CAM_WB Mode pq=00..0B:
        // From inquiry: 00 Auto, 01 3000K? Let's use direct index.
        self.wb_mode.min(11)
    }

    /// Drain background scan results; call from AppSpec::poll. True when changed.
    pub fn scan_poll(&mut self, tx: &mut rune::Tx) -> bool {
        use std::sync::atomic::Ordering;
        use std::time::Duration;
        // Snapshot under the lock, then release before mutating self.
        let (events, done, total, finished) = match self.scan.as_ref() {
            Some(scan) if scan.running => {
                let events = scan
                    .queue
                    .lock()
                    .map(|mut q| std::mem::take(&mut *q))
                    .unwrap_or_default();
                let done = scan.handle.checked.load(Ordering::SeqCst);
                let total = scan.handle.total.load(Ordering::SeqCst);
                let finished = scan.handle.done.load(Ordering::SeqCst);
                (events, done, total, finished)
            }
            _ => return false,
        };
        let mut changed = false;
        for e in events {
            match e {
                ScanEvent::Found(f) => {
                    self.apply_scan_found(&f);
                    changed = true;
                }
            }
        }
        if let Some(scan) = self.scan.as_mut() {
            if done != scan.done || total != scan.total {
                scan.done = done;
                scan.total = total;
                changed = true;
            }
            if finished {
                scan.running = false;
                let n = scan.results.len();
                let prefix = scan.prefix.clone();
                let via = if scan.via_relay { " via relay" } else { "" };
                scan.message = if n == 0 {
                    format!(
                        "No cameras found on {}.x{} — same WiFi as the camera? FoMaKo default lives on 192.168.5.x; type that prefix above, or add the IP manually below.",
                        prefix, via
                    )
                } else {
                    format!(
                        "Found {} camera{} on {}.x{} — tap Add.",
                        n,
                        if n == 1 { "" } else { "s" },
                        prefix, via
                    )
                };
                self.push_log(format!("Scan finished: {} found", n));
                changed = true;
            } else {
                tx.wake_after(Duration::from_millis(150));
            }
        }
        changed
    }

    /// Drain follow-loop updates; call from AppSpec::poll. True when changed.
    pub fn follow_poll(&mut self) -> bool {
        let events = self
            .follow_slot
            .lock()
            .map(|mut s| std::mem::take(&mut *s))
            .unwrap_or_default();
        if events.is_empty() {
            return false;
        }
        let gen = self.follow_gen.load(std::sync::atomic::Ordering::SeqCst);
        let mut changed = false;
        for event in events {
            match event {
                crate::follow::FollowEvent::Status { gen: g, msg } => {
                    if g != gen || !self.follow_on || self.follow_status == msg {
                        continue;
                    }
                    self.follow_status = msg;
                    changed = true;
                }
                crate::follow::FollowEvent::Error { gen: g, msg } => {
                    if g != gen || !self.follow_on {
                        continue;
                    }
                    let text = format!("follow error — {}", msg);
                    if self.follow_status != text {
                        self.follow_status = text.clone();
                        self.push_log(text);
                        changed = true;
                    }
                }
                crate::follow::FollowEvent::Calibrated { gen: g, pan, tilt } => {
                    if g != gen {
                        continue;
                    }
                    self.follow_pan_offset = pan;
                    self.follow_tilt_offset = tilt;
                    self.follow_ready = true;
                    let saved = self.save_follow_offsets();
                    self.follow_status = match saved {
                        Ok(()) => format!(
                            "Calibrated — offset pan {:+}, tilt {:+} steps. Start when ready.",
                            pan, tilt
                        ),
                        Err(error) => format!(
                            "Calibrated, but could not save the offset: {}",
                            error
                        ),
                    };
                    self.push_log(format!("Follow calibrated: pan {:+} tilt {:+}", pan, tilt));
                    changed = true;
                }
                crate::follow::FollowEvent::CalibrationError { gen: g, msg } => {
                    if g != gen {
                        continue;
                    }
                    self.follow_ready = false;
                    self.follow_status = format!("Calibration failed — {}", msg);
                    self.push_log(format!("Follow calibration failed: {}", msg));
                    changed = true;
                }
            }
        }
        changed
    }

    fn save_follow_offsets(&self) -> Result<(), String> {
        if self.cameras.len() < 2 {
            return Err("add a second camera first".to_string());
        }
        crate::follow::save_pair(
            &self.cameras,
            self.follow_leader.min(self.cameras.len() - 1),
            self.follow_follower.min(self.cameras.len() - 1),
            self.follow_pan_offset,
            self.follow_tilt_offset,
        )
    }

    #[cfg(test)]
    pub fn with_test_runtime() -> rune::Runtime<App> {
        Self::with_test_runtime_sized((1120.0, 720.0))
    }

    #[cfg(test)]
    pub fn with_test_runtime_sized(size: (f32, f32)) -> rune::Runtime<App> {
        let env = rune::Env {
            system_dark: true,
            scale: 1.0,
        };
        rune::Runtime::new(App::new(), crate::view::view, size, env)
    }

    /// Merge a discovery hit into the scan results (dedupe by IP).
    pub fn apply_scan_found(&mut self, f: &FoundCamera) {
        let Some(scan) = self.scan.as_mut() else {
            return;
        };
        if let Some(r) = scan.results.iter_mut().find(|r| r.ip == f.ip) {
            r.tcp5678 |= f.tcp5678;
            r.udp52381 |= f.udp52381;
            if r.detail.is_empty() || f.onvif {
                r.detail = f.detail();
            }
        } else {
            scan.results.push(ScanResult {
                ip: f.ip.clone(),
                detail: f.detail(),
                tcp5678: f.tcp5678,
                udp52381: f.udp52381,
            });
        }
        scan.results.sort_by(|a, b| a.ip.cmp(&b.ip));
    }
}

fn config_path() -> std::path::PathBuf {
    // Keep next to binary / cwd so church PC is portable (USB stick friendly).
    std::path::PathBuf::from("fomako-cameras.json")
}

fn load_cameras() -> Vec<CameraConfig> {
    // 1) cwd file, 2) exe dir file, else defaults (2 cams: pulpit + wide)
    for p in [config_path(), exe_dir_file()] {
        if let Ok(data) = std::fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str::<Vec<CameraConfig>>(&data) {
                if !v.is_empty() {
                    return v;
                }
            }
        }
    }
    vec![
        CameraConfig {
            name: "Pulpit".into(),
            ip: "192.168.5.163".into(),
            protocol: ProtoSer::SonyUdp,
            address: 1,
            username: default_user(),
            password: default_user(),
        },
        CameraConfig {
            name: "Wide / Choir".into(),
            ip: "192.168.5.164".into(),
            protocol: ProtoSer::SonyUdp,
            address: 2,
            username: default_user(),
            password: default_user(),
        },
    ]
}

fn exe_dir_file() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("fomako-cameras.json")))
        .unwrap_or_else(config_path)
}

fn save_cameras(cams: &[CameraConfig]) {
    if let Ok(j) = serde_json::to_string_pretty(cams) {
        let _ = std::fs::write(config_path(), j);
        // also try exe dir (may fail without perms; ignore)
        let _ = std::fs::write(exe_dir_file(), serde_json::to_string_pretty(cams).unwrap_or_default());
    }
}

#[cfg(test)]
mod follow_tests {
    use super::*;

    #[test]
    fn stale_calibration_is_ignored() {
        let mut app = App::new();
        app.follow_ready = false;
        app.follow_on = true;
        let gen = app.follow_gen.load(std::sync::atomic::Ordering::SeqCst);
        app.follow_slot
            .lock()
            .unwrap()
            .push(crate::follow::FollowEvent::Calibrated {
                gen: gen.wrapping_sub(1),
                pan: 111,
                tilt: 222,
            });
        assert!(!app.follow_poll());
        assert!(!app.follow_ready);
        assert_ne!(app.follow_pan_offset, 111);
    }

    #[test]
    fn follow_status_event_from_an_old_thread_is_ignored() {
        let mut app = App::new();
        app.follow_on = true;
        app.follow_status = "Stopped".into();
        let gen = app.follow_gen.load(std::sync::atomic::Ordering::SeqCst);
        app.follow_slot
            .lock()
            .unwrap()
            .push(crate::follow::FollowEvent::Status {
                gen: gen.wrapping_sub(1),
                msg: "Following — old drives stale".into(),
            });
        assert!(!app.follow_poll());
        assert_eq!(app.follow_status, "Stopped");
    }
}
