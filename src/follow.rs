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
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::model::CameraConfig;
#[cfg(not(target_arch = "wasm32"))]
use crate::visca::{self, Protocol};

/// Largest magnitude accepted for a stored or nudged offset.
pub const OFFSET_MAX: i32 = 0x7FFF;
/// ~0.1° at the documented ~0.5° per 100 counts; the camera's own aim accuracy.
pub const DEADBAND: i32 = 20;
#[cfg(not(target_arch = "wasm32"))]
const RESEND: Duration = Duration::from_secs(2);
#[cfg(not(target_arch = "wasm32"))]
const TICK: Duration = Duration::from_millis(120);

#[derive(Debug)]
pub enum FollowEvent {
    Status { gen: u64, msg: String },
    Error { gen: u64, msg: String },
    Calibrated { gen: u64, pan: i32, tilt: i32 },
    CalibrationError { gen: u64, msg: String },
}

pub fn following_note(leader: &str, follower: &str) -> String {
    format!("Following — {} drives {}.", leader, follower)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FollowSaved {
    pub leader_ip: String,
    pub follower_ip: String,
    pub pan_offset: i32,
    pub tilt_offset: i32,
}

fn paths() -> Vec<std::path::PathBuf> {
    let cwd = std::path::PathBuf::from("fomako-follow.json");
    let cwd_abs = std::env::current_dir()
        .ok()
        .and_then(|d| d.canonicalize().ok())
        .map(|d| d.join("fomako-follow.json"));
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("fomako-follow.json")));
    match exe {
        Some(exe) if cwd_abs.as_ref() != Some(&exe) => vec![cwd, exe],
        _ => vec![cwd],
    }
}

fn load_from(path: &std::path::Path) -> Option<FollowSaved> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn load_newest(paths: Vec<std::path::PathBuf>) -> Option<FollowSaved> {
    // Prefer the newest copy, so a stale working-directory file cannot
    // shadow an offset written beside the app.
    paths
        .into_iter()
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
            load_from(&path).map(|saved| (modified, saved))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, saved)| saved)
}

pub fn load_saved() -> Option<FollowSaved> {
    load_newest(paths())
}

fn write_one(path: &std::path::Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_extension("json.tmp");
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{}: {error}", tmp.display()));
    }
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{}: could not replace", path.display()));
    }
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// Write every portable copy. A read-only location does not discard a writable one.
pub fn save(saved: &FollowSaved) -> Result<(), String> {
    save_to(&paths(), saved)
}

fn save_to(paths: &[std::path::PathBuf], saved: &FollowSaved) -> Result<(), String> {
    let text = serde_json::to_string_pretty(saved).map_err(|e| e.to_string())?;
    let mut first_error = None;
    let mut wrote = false;
    for path in paths {
        match write_one(path, &text) {
            Ok(()) => wrote = true,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }
    if wrote {
        match first_error {
            Some(error) => Err(format!("saved one copy but not all: {}", error)),
            None => Ok(()),
        }
    } else {
        Err(first_error.unwrap_or_else(|| "no writable config location".to_string()))
    }
}

/// Persist the calibrated pair; rejects the same camera twice.
pub fn save_pair(
    cameras: &[CameraConfig],
    leader: usize,
    follower: usize,
    pan_offset: i32,
    tilt_offset: i32,
) -> Result<(), String> {
    if leader >= cameras.len() || follower >= cameras.len() || leader == follower {
        return Err("pick two different cameras".to_string());
    }
    if cameras[leader].ip == cameras[follower].ip {
        return Err("pick two different cameras (same IP)".to_string());
    }
    save(&FollowSaved {
        leader_ip: cameras[leader].ip.clone(),
        follower_ip: cameras[follower].ip.clone(),
        pan_offset,
        tilt_offset,
    })
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

/// Shortest signed delta on the 16-bit circle. FoMaKo/Sony positions are
/// signed two's complement around home, so `0x0010 → 0xFFF0` is -32, not
/// +65504.
pub fn shortest_delta(leader: u16, follower: u16) -> i32 {
    let delta = (follower as i32 - leader as i32).rem_euclid(0x1_0000);
    if delta > 0x7FFF {
        delta - 0x1_0000
    } else {
        delta
    }
}

/// Offset that maps leader aim to follower aim at one shared point.
pub fn offset_from(leader: (u16, u16), follower: (u16, u16)) -> (i32, i32) {
    (
        shortest_delta(leader.0, follower.0),
        shortest_delta(leader.1, follower.1),
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn calibrate(leader: &CameraConfig, follower: &CameraConfig) -> Result<(i32, i32), String> {
    if leader.ip == follower.ip {
        return Err("pick two different cameras".to_string());
    }
    let first = query_pos(leader)?;
    let follower_pos = query_pos(follower)?;
    let second = query_pos(leader)?;
    let follower_second = query_pos(follower)?;
    if shortest_delta(first.0, second.0).abs() > DEADBAND
        || shortest_delta(first.1, second.1).abs() > DEADBAND
        || shortest_delta(follower_pos.0, follower_second.0).abs() > DEADBAND
        || shortest_delta(follower_pos.1, follower_second.1).abs() > DEADBAND
    {
        return Err("a camera moved during calibration — hold both still and retry".to_string());
    }
    let (pan, tilt) = offset_from(first, follower_pos);
    Ok((
        pan.clamp(i16::MIN as i32, OFFSET_MAX),
        tilt.clamp(i16::MIN as i32, OFFSET_MAX),
    ))
}

/// Map a leader position through the calibrated offset. The result wraps on
/// the same 16-bit circle the camera uses, so home is crossed, not clamped.
pub fn mapped_target(leader_pan: u16, leader_tilt: u16, pan_offset: i32, tilt_offset: i32) -> (u16, u16) {
    let pan = (leader_pan as i32 + pan_offset).rem_euclid(0x1_0000);
    let tilt = (leader_tilt as i32 + tilt_offset).rem_euclid(0x1_0000);
    (pan as u16, tilt as u16)
}

pub fn needs_move(last: Option<(u16, u16)>, target: (u16, u16)) -> bool {
    match last {
        None => true,
        Some((p, t)) => {
            (shortest_delta(p, target.0)).abs() > DEADBAND
                || (shortest_delta(t, target.1)).abs() > DEADBAND
        }
    }
}

#[derive(Debug)]
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
    slot: Arc<Mutex<Vec<FollowEvent>>>,
    wake: Option<rune::Waker>,
) {
    std::thread::spawn(move || {
        // A follower left in camera-side tracking ignores pan/tilt.
        let (follower_protocol, follower_port) = proto(&spec.follower);
        let mut tracking_off_error = None;
        let mut last_tracking_off = std::time::Instant::now();
        if let Err(error) = visca::send_ack(
            &spec.follower.ip,
            follower_port,
            follower_protocol,
            &visca::tracking(spec.follower.address, false, false),
        ) {
            let msg = format!("{} tracking off: {}", spec.follower.name, error);
            tracking_off_error = Some(msg.clone());
            emit(&slot, &wake, FollowEvent::Error { gen, msg });
        }
        let mut last_sent: Option<(u16, u16)> = None;
        let mut last_sent_at: Option<std::time::Instant> = None;
        let mut last_note = String::new();
        let mut last_leader_error = String::new();
        let mut last_follower_error = String::new();
        while current.load(Ordering::SeqCst) == gen {
            if tracking_off_error.is_some() && last_tracking_off.elapsed() >= RESEND {
                last_tracking_off = std::time::Instant::now();
                let (protocol, port) = proto(&spec.follower);
                match visca::send_ack(
                    &spec.follower.ip,
                    port,
                    protocol,
                    &visca::tracking(spec.follower.address, false, false),
                ) {
                    Ok(()) => tracking_off_error = None,
                    Err(error) => {
                        let msg = format!("{} tracking off: {}", spec.follower.name, error);
                        if tracking_off_error.as_ref() != Some(&msg) {
                            tracking_off_error = Some(msg.clone());
                            emit(&slot, &wake, FollowEvent::Error { gen, msg });
                        }
                    }
                }
            }
            match query_pos(&spec.leader) {
                Ok((lp, lt)) => {
                    if current.load(Ordering::SeqCst) != gen {
                        break;
                    }
                    if !last_leader_error.is_empty() {
                        last_leader_error.clear();
                        last_note.clear();
                    }
                    let target =
                        mapped_target(lp, lt, spec.pan_offset, spec.tilt_offset);
                    let mut tick_error = false;
                    let stale = last_sent_at
                        .map(|at| at.elapsed() >= RESEND)
                        .unwrap_or(false);
                    let follower_off = stale
                        && query_pos(&spec.follower)
                            .map(|(fp, ft)| {
                                shortest_delta(fp, target.0).abs() > DEADBAND
                                    || shortest_delta(ft, target.1).abs() > DEADBAND
                            })
                            .unwrap_or(true);
                    if stale && !follower_off {
                        last_sent_at = Some(std::time::Instant::now());
                    }
                    if needs_move(last_sent, target) || follower_off {
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
                        match visca::send_only(&spec.follower.ip, port, protocol, &payload) {
                            Ok(()) => {
                                last_sent = Some(target);
                                last_sent_at = Some(std::time::Instant::now());
                                last_follower_error.clear();
                            }
                            Err(error) => {
                                let note = format!("{}: {}", spec.follower.name, error);
                                tick_error = true;
                                if note != last_follower_error {
                                    last_follower_error = note.clone();
                                    last_note.clear();
                                    if current.load(Ordering::SeqCst) == gen {
                                        emit(&slot, &wake, FollowEvent::Error { gen, msg: note });
                                    }
                                }
                            }
                        }
                    }
                    if current.load(Ordering::SeqCst) != gen {
                        break;
                    }
                    if tick_error {
                        sleep_while(gen, &current, TICK);
                        continue;
                    }
                    let note = following_note(&spec.leader.name, &spec.follower.name);
                    if note != last_note {
                        last_note = note.clone();
                        emit(&slot, &wake, FollowEvent::Status { gen, msg: note });
                    }
                }
                Err(error) => {
                    if current.load(Ordering::SeqCst) != gen {
                        break;
                    }
                    let note = format!("{}: {}", spec.leader.name, error);
                    if note != last_leader_error {
                        last_leader_error = note.clone();
                        last_note.clear();
                        emit(&slot, &wake, FollowEvent::Error { gen, msg: note });
                    }
                }
            }
            sleep_while(gen, &current, TICK);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn emit(
    slot: &Arc<Mutex<Vec<FollowEvent>>>,
    wake: &Option<rune::Waker>,
    event: FollowEvent,
) {
    if let Ok(mut s) = slot.lock() {
        s.push(event);
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
    fn offset_maps_and_wraps_at_home() {
        assert_eq!(mapped_target(1000, 2000, 100, -500), (1100, 1500));
        // crossing the zero/home point wraps on the 16-bit circle
        assert_eq!(mapped_target(10, 10, -100, -100), (0xFFA6, 0xFFA6));
        assert_eq!(mapped_target(0xFFF0, 0xFFF0, 100, 100), (0x0054, 0x0054));
    }

    #[test]
    fn calibration_offset_is_shortest_signed_delta() {
        assert_eq!(offset_from((1000, 2000), (900, 2100)), (-100, 100));
        assert_eq!(offset_from((0x0010, 0x0010), (0xFFF0, 0xFFF0)), (-32, -32));
    }

    #[test]
    fn small_aim_changes_are_ignored() {
        assert!(needs_move(None, (100, 100)));
        assert!(!needs_move(Some((100, 100)), (120, 80)));
        assert!(needs_move(Some((100, 100)), (121, 100)));
        assert!(needs_move(Some((0x0000, 100)), (0xFFE0, 100)));
    }

    #[test]
    fn mapped_target_inverts_the_calibrated_offset() {
        for leader in [(1000u16, 2000u16), (0x0005, 0x8000), (0xFFF0, 0x0010)] {
            for follower in [(900u16, 2100u16), (0x0010, 0x7FF0), (0x1234, 0xABCD)] {
                let offset = offset_from(leader, follower);
                assert_eq!(mapped_target(leader.0, leader.1, offset.0, offset.1), follower);
            }
        }
    }

    #[test]
    fn save_pair_rejects_the_same_camera() {
        let cam = |ip: &str| CameraConfig {
            name: ip.into(),
            ip: ip.into(),
            address: 1,
            protocol: crate::model::ProtoSer::SonyUdp,
            username: "admin".into(),
            password: "admin".into(),
        };
        let cameras = vec![cam("192.168.5.163"), cam("192.168.5.163"), cam("192.168.5.164")];
        assert!(save_pair(&cameras, 0, 1, 0, 0).is_err());
        assert!(save_pair(&cameras, 0, 0, 0, 0).is_err());
    }

    #[test]
    fn newest_copy_wins() {
        let dir = std::env::temp_dir().join(format!(
            "fomako-follow-newest-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("old.json");
        let new = dir.join("new.json");
        let saved = |pan| FollowSaved {
            leader_ip: "a".into(),
            follower_ip: "b".into(),
            pan_offset: pan,
            tilt_offset: 0,
        };
        std::fs::write(&old, serde_json::to_string(&saved(1)).unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&new, serde_json::to_string(&saved(2)).unwrap()).unwrap();
        assert_eq!(load_newest(vec![old.clone(), new.clone()]).unwrap().pan_offset, 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn partial_save_reports_an_error_but_keeps_the_writable_copy() {
        let dir = std::env::temp_dir().join(format!(
            "fomako-follow-partial-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.json");
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let bad = blocker.join("bad.json");
        let saved = FollowSaved {
            leader_ip: "a".into(),
            follower_ip: "b".into(),
            pan_offset: 7,
            tilt_offset: 8,
        };
        let result = save_to(&[good.clone(), bad], &saved);
        assert!(result.is_err(), "partial write must be reported");
        assert_eq!(load_from(&good).unwrap().pan_offset, 7);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saved_offsets_round_trip_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "fomako-follow-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let path = nested.join("fomako-follow.json");
        let saved = FollowSaved {
            leader_ip: "192.168.5.163".into(),
            follower_ip: "192.168.5.164".into(),
            pan_offset: -32,
            tilt_offset: 100,
        };
        let text = serde_json::to_string_pretty(&saved).unwrap();
        write_one(&path, &text).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.pan_offset, -32);
        assert_eq!(loaded.tilt_offset, 100);
        assert!(write_one(&nested, &text).is_err(), "directory is not a file target");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
