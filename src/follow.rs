//! Cross-camera follow: slave one camera's pan/tilt to another's.
//!
//! FoMaKo Gen-3 cameras can auto-track a person on their own, but they expose
//! no VISCA subject coordinates or tracking status (manual V3.0 §5.2/§5.3).
//! What they do expose is absolute pan/tilt and a pan/tilt position inquiry.
//! So the follower mirrors the leader's *aim*: point both cameras at the same
//! landmark once, calibrate the offset, and the follower repeats every move
//! the leader's AI tracking makes.
//!
//! Limits (documented in the UI/README): both cameras must see the subject,
//! the follower's own AI tracking must be off, and the offset is a fixed aim
//! mapping, not a depth-corrected 3D solve. It is the closest behaviour the
//! hardware supports without a vision tracker in this app.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::model::CameraConfig;
#[cfg(not(target_arch = "wasm32"))]
use crate::visca::{self, Protocol};

/// Position values are 16-bit linear counts (`0Y 0Y 0Y 0Y`).
pub const POS_MAX: i32 = 0xFFFF;
const DEADBAND: i32 = 3;
#[cfg(not(target_arch = "wasm32"))]
const TICK: Duration = Duration::from_millis(120);

#[derive(Clone, Debug)]
pub enum FollowEvent {
    Status(String),
    Error(String),
    Calibrated { gen: u64, pan: i32, tilt: i32 },
    CalibrationError { gen: u64, msg: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FollowSaved {
    pub leader_ip: String,
    pub follower_ip: String,
    pub pan_offset: i32,
    pub tilt_offset: i32,
}

fn saved_paths() -> Vec<std::path::PathBuf> {
    let cwd = std::path::PathBuf::from("fomako-follow.json");
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("fomako-follow.json")));
    match exe {
        Some(exe) if exe != cwd => vec![cwd, exe],
        _ => vec![cwd],
    }
}

pub fn load_saved() -> Option<FollowSaved> {
    for path in saved_paths() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(saved) = serde_json::from_str::<FollowSaved>(&text) {
                return Some(saved);
            }
        }
    }
    None
}

pub fn save(saved: &FollowSaved) {
    if let Ok(text) = serde_json::to_string_pretty(saved) {
        for path in saved_paths() {
            let _ = std::fs::write(path, &text);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn proto(cam: &CameraConfig) -> (Protocol, u16) {
    (cam.protocol.to_protocol(), cam.protocol.port())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn query_pos(cam: &CameraConfig) -> Result<(u16, u16), String> {
    let (protocol, port) = proto(cam);
    let reply = visca::query_blocking(
        &cam.ip,
        port,
        protocol,
        &visca::pan_tilt_pos_inq(cam.address),
    )?;
    visca::parse_pan_tilt_pos(&reply).ok_or_else(|| {
        format!(
            "{} sent an unexpected reply: {}",
            cam.name,
            visca::hex(&reply)
        )
    })
}

/// Offset that maps leader aim to follower aim at one shared point.
pub fn offset_from(
    leader: (u16, u16),
    follower: (u16, u16),
) -> (i32, i32) {
    (
        follower.0 as i32 - leader.0 as i32,
        follower.1 as i32 - leader.1 as i32,
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn calibrate(leader: &CameraConfig, follower: &CameraConfig) -> Result<(i32, i32), String> {
    if leader.ip == follower.ip {
        return Err("pick two different cameras".to_string());
    }
    let leader_pos = query_pos(leader)?;
    let follower_pos = query_pos(follower)?;
    Ok(offset_from(leader_pos, follower_pos))
}

/// Map a leader position through the calibrated offset, clamped to the range.
pub fn mapped_target(leader_pan: u16, leader_tilt: u16, pan_offset: i32, tilt_offset: i32) -> (u16, u16) {
    let pan = (leader_pan as i32 + pan_offset).clamp(0, POS_MAX) as u16;
    let tilt = (leader_tilt as i32 + tilt_offset).clamp(0, POS_MAX) as u16;
    (pan, tilt)
}

pub fn needs_move(last: Option<(u16, u16)>, target: (u16, u16)) -> bool {
    match last {
        None => true,
        Some((p, t)) => {
            (p as i32 - target.0 as i32).abs() > DEADBAND
                || (t as i32 - target.1 as i32).abs() > DEADBAND
        }
    }
}

#[derive(Clone, Debug)]
pub struct FollowSpec {
    pub leader: CameraConfig,
    pub follower: CameraConfig,
    pub pan_offset: i32,
    pub tilt_offset: i32,
    pub pan_speed: u8,
    pub tilt_speed: u8,
}

/// Start the follow loop. Bumping `gen`, or process exit, stops it.
#[cfg(not(target_arch = "wasm32"))]
pub fn start(
    spec: FollowSpec,
    gen: u64,
    current: Arc<AtomicU64>,
    slot: Arc<Mutex<Option<FollowEvent>>>,
    wake: Option<rune::Waker>,
) {
    std::thread::spawn(move || {
        let mut last_sent: Option<(u16, u16)> = None;
        let mut last_note = String::new();
        let mut last_leader_error = String::new();
        let mut last_follower_error = String::new();
        while current.load(Ordering::SeqCst) == gen {
            match query_pos(&spec.leader) {
                Ok((lp, lt)) => {
                    if current.load(Ordering::SeqCst) != gen {
                        break;
                    }
                    last_leader_error.clear();
                    let target =
                        mapped_target(lp, lt, spec.pan_offset, spec.tilt_offset);
                    if needs_move(last_sent, target) {
                        let (protocol, port) = proto(&spec.follower);
                        let payload = visca::pan_tilt_absolute(
                            spec.follower.address,
                            spec.pan_speed,
                            spec.tilt_speed,
                            target.0,
                            target.1,
                        );
                        if current.load(Ordering::SeqCst) != gen {
                            break;
                        }
                        if let Err(error) = visca::send_only(&spec.follower.ip, port, protocol, &payload)
                        {
                            let note = format!("{}: {}", spec.follower.name, error);
                            if note != last_follower_error {
                                last_follower_error = note.clone();
                                emit(&slot, &wake, FollowEvent::Error(note));
                            }
                        } else {
                            last_sent = Some(target);
                            last_follower_error.clear();
                        }
                    }
                    let note = format!("Following — {} drives {}", spec.leader.name, spec.follower.name);
                    if note != last_note {
                        last_note = note.clone();
                        emit(&slot, &wake, FollowEvent::Status(note));
                    }
                }
                Err(error) => {
                    if current.load(Ordering::SeqCst) != gen {
                        break;
                    }
                    let note = format!("{}: {}", spec.leader.name, error);
                    if note != last_leader_error {
                        last_leader_error = note.clone();
                        emit(&slot, &wake, FollowEvent::Error(note));
                    }
                }
            }
            sleep_while(gen, &current, TICK);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn emit(
    slot: &Arc<Mutex<Option<FollowEvent>>>,
    wake: &Option<rune::Waker>,
    event: FollowEvent,
) {
    if let Ok(mut s) = slot.lock() {
        *s = Some(event);
    }
    if let Some(w) = wake {
        w();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sleep_while(gen: u64, current: &Arc<AtomicU64>, total: Duration) {
    let step = Duration::from_millis(20);
    let started = std::time::Instant::now();
    while started.elapsed() < total {
        if current.load(Ordering::SeqCst) != gen {
            break;
        }
        std::thread::sleep(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_maps_and_clamps() {
        assert_eq!(mapped_target(1000, 2000, 100, -500), (1100, 1500));
        assert_eq!(mapped_target(10, 10, -100, -100), (0, 0));
        assert_eq!(mapped_target(0xFFF0, 0xFFF0, 100, 100), (0xFFFF, 0xFFFF));
    }

    #[test]
    fn calibration_offset_is_follower_minus_leader() {
        assert_eq!(offset_from((1000, 2000), (900, 2100)), (-100, 100));
    }

    #[test]
    fn small_aim_changes_are_ignored() {
        assert!(needs_move(None, (100, 100)));
        assert!(!needs_move(Some((100, 100)), (102, 98)));
        assert!(needs_move(Some((100, 100)), (104, 100)));
    }
}
