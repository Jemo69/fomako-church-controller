#![allow(dead_code)]
//! VISCA protocol for FoMaKo PTZ cameras (from User Manual V3.0).
//! Supports:
//! - Sony VISCA-over-IP (UDP port 52381): 8-byte header + VISCA payload
//! - FoMaKo IP-VISCA (TCP port 5678): raw VISCA payload
//!
//! Manual refs: IP Visca port 5678, Sony Visca port 52381, Onvif 2000,
//! user admin / admin, default IP 192.168.5.163.

use std::net::{TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    SonyUdp,
    IpViscaTcp,
}

impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Protocol::SonyUdp => "VISCA-over-IP (UDP 52381)",
            Protocol::IpViscaTcp => "IP-VISCA (TCP 5678)",
        }
    }
    pub fn default_port(self) -> u16 {
        match self {
            Protocol::SonyUdp => 52381,
            Protocol::IpViscaTcp => 5678,
        }
    }
}

/// Clamp camera VISCA address 1..=7 to header nibble.
pub fn addr_byte(address: u8) -> u8 {
    let a = address.clamp(1, 7);
    0x80 | a
}

pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------- builders

pub fn power(address: u8, on: bool) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x00, if on { 0x02 } else { 0x03 }, 0xFF]
}

#[derive(Clone, Copy)]
pub enum PtDir {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Stop,
    Home,
}

pub fn pan_tilt_drive(address: u8, pan_speed: u8, tilt_speed: u8, dir: PtDir) -> Vec<u8> {
    let a = addr_byte(address);
    let vs = pan_speed.clamp(0x01, 0x18);
    let ws = tilt_speed.clamp(0x01, 0x14);
    match dir {
        PtDir::Home => vec![a, 0x01, 0x06, 0x04, 0xFF],
        PtDir::Stop => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x03, 0x03, 0xFF],
        PtDir::Up => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x03, 0x01, 0xFF],
        PtDir::Down => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x03, 0x02, 0xFF],
        PtDir::Left => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x01, 0x03, 0xFF],
        PtDir::Right => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x02, 0x03, 0xFF],
        PtDir::UpLeft => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x01, 0x01, 0xFF],
        PtDir::UpRight => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x02, 0x01, 0xFF],
        PtDir::DownLeft => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x01, 0x02, 0xFF],
        PtDir::DownRight => vec![a, 0x01, 0x06, 0x01, vs, ws, 0x02, 0x02, 0xFF],
    }
}

pub fn pan_tilt_reset(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x06, 0x05, 0xFF]
}

pub fn zoom_stop(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x07, 0x00, 0xFF]
}
pub fn zoom_tele_std(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x07, 0x02, 0xFF]
}
pub fn zoom_wide_std(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x07, 0x03, 0xFF]
}
pub fn zoom_tele_var(address: u8, speed: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x07, 0x20 | (speed & 0x07), 0xFF]
}
pub fn zoom_wide_var(address: u8, speed: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x07, 0x30 | (speed & 0x07), 0xFF]
}

pub fn focus_stop(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x08, 0x00, 0xFF]
}
pub fn focus_far(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x08, 0x02, 0xFF]
}
pub fn focus_near(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x08, 0x03, 0xFF]
}
pub fn focus_auto(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x38, 0x02, 0xFF]
}
pub fn focus_manual(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x38, 0x03, 0xFF]
}
pub fn focus_one_push(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x38, 0x04, 0xFF]
}

// Presets 0..=254 (remote uses 0..=9)
pub fn preset_set(address: u8, n: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x3F, 0x01, n, 0xFF]
}
pub fn preset_recall(address: u8, n: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x3F, 0x02, n, 0xFF]
}
pub fn preset_clear(address: u8, n: u8) -> Vec<u8> {
    // Manual lists Reset 00 pq; hardware "Clear" maps here.
    vec![addr_byte(address), 0x01, 0x04, 0x3F, 0x00, n, 0xFF]
}

// Exposure
pub fn ae_mode(address: u8, mode: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x39, mode, 0xFF]
}
// mode bytes: 0x00 full auto, 0x03 manual, 0x0A shutter, 0x0B iris, 0x0D bright
pub fn shutter_direct(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x4A, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn iris_direct(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x4B, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn gain_limit(address: u8, p: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x2C, p & 0x0F, 0xFF]
}
pub fn bright_direct(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x4D, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn expcomp_onoff(address: u8, on: bool) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x3E, if on { 0x02 } else { 0x03 }, 0xFF]
}
pub fn expcomp_direct(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x4E, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn backlight(address: u8, on: bool) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x33, if on { 0x02 } else { 0x03 }, 0xFF]
}
pub fn wdr(address: u8, level: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x51, 0x00, 0x00, 0x00, level & 0x0F, 0xFF]
}
pub fn nr2d(address: u8, level: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x53, level & 0x07, 0xFF]
}
pub fn nr3d(address: u8, level: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x54, level & 0x0F, 0xFF]
}
pub fn flicker(address: u8, mode: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x23, mode & 0x0F, 0xFF]
}
pub fn gamma(address: u8, p: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x5B, p & 0x0F, 0xFF]
}
pub fn lowlight(address: u8, on: bool) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x2D, if on { 0x01 } else { 0x00 }, 0xFF]
}
pub fn aperture(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x42, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn brightness_direct(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0xA1, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn contrast_direct(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0xA2, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
/// p = 0..=14 (60%..200%)
pub fn saturation(address: u8, p: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, p.min(14) & 0x0F, 0xFF]
}
pub fn wb_mode(address: u8, pq: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x35, pq, 0xFF]
}
pub fn wb_one_push_trigger(address: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x10, 0x05, 0xFF]
}
pub fn awb_sensitivity(address: u8, level: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0xA9, level.min(2) & 0x0F, 0xFF]
}
pub fn rgain(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x43, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn bgain(address: u8, v: u8) -> Vec<u8> {
    let a = addr_byte(address);
    vec![a, 0x01, 0x04, 0x44, 0x00, 0x00, (v >> 4) & 0x0F, v & 0x0F, 0xFF]
}
pub fn flip(address: u8, mode: u8) -> Vec<u8> {
    // 0 off, 1 H, 2 V, 3 HV
    vec![addr_byte(address), 0x01, 0x04, 0xA4, mode & 0x03, 0xFF]
}
pub fn lr_reverse(address: u8, on: bool) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x61, if on { 0x02 } else { 0x03 }, 0xFF]
}
pub fn picture_flip(address: u8, on: bool) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x66, if on { 0x02 } else { 0x03 }, 0xFF]
}
pub fn picture_bw(address: u8, on: bool) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x63, if on { 0x04 } else { 0x00 }, 0xFF]
}
pub fn video_system(address: u8, p: u8) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x06, 0x35, 0x00, p & 0x0F, 0xFF]
}
pub fn menu_onoff(address: u8, on: bool) -> Vec<u8> {
    vec![addr_byte(address), 0x01, 0x04, 0x06, 0x06, if on { 0x02 } else { 0x03 }, 0xFF]
}

pub fn named_command(op: &str, addr: u8, a: i32, b: i32, c: i32) -> Option<Vec<u8>> {
    let a_u8 = a.clamp(0, 255) as u8;
    let b_u8 = b.clamp(0, 255) as u8;
    Some(match op {
        "power" => power(addr, a != 0),
        "pt_drive" => pan_tilt_drive(addr, a_u8, b_u8, dir_from_code(c)),
        "zoom_tele" => zoom_tele_var(addr, a_u8),
        "zoom_wide" => zoom_wide_var(addr, a_u8),
        "zoom_stop" => zoom_stop(addr),
        "focus_far" => focus_far(addr),
        "focus_near" => focus_near(addr),
        "focus_stop" => focus_stop(addr),
        "focus_auto" => focus_auto(addr),
        "focus_manual" => focus_manual(addr),
        "focus_one_push" => focus_one_push(addr),
        "preset_recall" => preset_recall(addr, a_u8),
        "preset_set" => preset_set(addr, a_u8),
        "preset_clear" => preset_clear(addr, a_u8),
        "ae_mode" => ae_mode(addr, a_u8),
        "shutter" => shutter_direct(addr, a_u8),
        "iris" => iris_direct(addr, a_u8),
        "gain_limit" => gain_limit(addr, a_u8),
        "bright" => bright_direct(addr, a_u8),
        "expcomp_onoff" => expcomp_onoff(addr, a != 0),
        "expcomp" => expcomp_direct(addr, a_u8),
        "backlight" => backlight(addr, a != 0),
        "wdr" => wdr(addr, a_u8),
        "nr2d" => nr2d(addr, a_u8),
        "nr3d" => nr3d(addr, a_u8),
        "flicker" => flicker(addr, a_u8),
        "gamma" => gamma(addr, a_u8),
        "lowlight" => lowlight(addr, a != 0),
        "aperture" => aperture(addr, a_u8),
        "brightness" => brightness_direct(addr, a_u8),
        "contrast" => contrast_direct(addr, a_u8),
        "saturation" => saturation(addr, a_u8),
        "wb_mode" => wb_mode(addr, a_u8),
        "wb_one_push" => wb_one_push_trigger(addr),
        "awb_sens" => awb_sensitivity(addr, a_u8),
        "rgain" => rgain(addr, a_u8),
        "bgain" => bgain(addr, a_u8),
        "flip" => flip(addr, a_u8),
        "bw" => picture_bw(addr, a != 0),
        _ => return None,
    })
}

/// Direction codes used by the Android JNI bridge.
pub fn dir_from_code(code: i32) -> PtDir {
    match code {
        0 => PtDir::Up,
        1 => PtDir::Down,
        2 => PtDir::Left,
        3 => PtDir::Right,
        4 => PtDir::UpLeft,
        5 => PtDir::UpRight,
        6 => PtDir::DownLeft,
        7 => PtDir::DownRight,
        8 => PtDir::Home,
        _ => PtDir::Stop,
    }
}

// ---------------------------------------------------------------- transport

/// Sony VISCA-over-IP header: 01 00 LL LL SS SS SS SS + payload
pub fn sony_encapsulate(payload: &[u8], seq: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.push(0x01);
    out.push(0x00);
    let len = payload.len() as u16;
    out.push((len >> 8) as u8);
    out.push((len & 0xFF) as u8);
    out.push(((seq >> 24) & 0xFF) as u8);
    out.push(((seq >> 16) & 0xFF) as u8);
    out.push(((seq >> 8) & 0xFF) as u8);
    out.push((seq & 0xFF) as u8);
    out.extend_from_slice(payload);
    out
}

fn send_sony_udp(ip: &str, port: u16, payload: &[u8]) -> Result<String, String> {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let packet = sony_encapsulate(payload, seq);
    let local = if ip.contains(':') { "[::]:0" } else { "0.0.0.0:0" };
    let sock = UdpSocket::bind(local).map_err(|e| format!("udp bind: {e}"))?;
    sock.set_read_timeout(Some(Duration::from_millis(400)))
        .map_err(|e| format!("timeout: {e}"))?;
    sock.set_write_timeout(Some(Duration::from_millis(800)))
        .map_err(|e| format!("timeout: {e}"))?;
    let target = format!("{}:{}", ip, port);
    sock.send_to(&packet, &target)
        .map_err(|e| format!("send {}: {e}", target))?;
    // Best-effort read of ACK/completion; ignore timeout (camera still moves).
    let mut buf = [0u8; 64];
    match sock.recv_from(&mut buf) {
        Ok((n, _)) => {
            let reply = if n >= 8 && buf[..2] == [0x01, 0x00] {
                &buf[8..n]
            } else {
                &buf[..n]
            };
            if reply.len() >= 3 && reply[0] == 0x90 && reply[1] == 0x41 {
                return Err(format!("camera rejected command ({})", hex(reply)));
            }
            Ok(format!("sent {}B seq={} ack {}B", packet.len(), seq, n))
        }
        Err(_) => Ok(format!("sent {}B seq={} (no ACK)", packet.len(), seq)),
    }
}

fn send_tcp(ip: &str, port: u16, payload: &[u8]) -> Result<String, String> {
    let target = format!("{}:{}", ip, port);
    let sock_addr: std::net::SocketAddr = {
        use std::net::ToSocketAddrs;
        target.to_socket_addrs().map_err(|e| format!("bad address {}: {}", target, e))?.next().ok_or_else(|| format!("bad address {}", target))?
    };
    let mut stream = TcpStream::connect_timeout(&sock_addr, Duration::from_millis(1200))
        .map_err(|e| format!("tcp {}: {e}", target))?;
    stream
        .set_write_timeout(Some(Duration::from_millis(800)))
        .map_err(|e| format!("timeout: {e}"))?;
    use std::io::Write;
    stream
        .write_all(payload)
        .map_err(|e| format!("write: {e}"))?;
    Ok(format!("sent {}B tcp {}", payload.len(), target))
}

/// Fire-and-forget from UI thread.
pub fn send_async(ip: String, port: u16, protocol: Protocol, payload: Vec<u8>) {
    std::thread::spawn(move || {
        let r = match protocol {
            Protocol::SonyUdp => send_sony_udp(&ip, port, &payload),
            Protocol::IpViscaTcp => send_tcp(&ip, port, &payload),
        };
        if let Err(e) = r {
            eprintln!("[visca] {} -> {}:{} failed: {}", hex(&payload), ip, port, e);
        }
    });
}

pub fn send_blocking(ip: &str, port: u16, protocol: Protocol, payload: &[u8]) -> Result<String, String> {
    match protocol {
        Protocol::SonyUdp => send_sony_udp(ip, port, payload),
        Protocol::IpViscaTcp => send_tcp(ip, port, payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headers_look_right() {
        let p = pan_tilt_drive(1, 12, 12, PtDir::Up);
        assert_eq!(p, vec![0x81, 0x01, 0x06, 0x01, 12, 12, 0x03, 0x01, 0xFF]);
        let enc = sony_encapsulate(&p, 1);
        assert_eq!(&enc[0..4], &[0x01, 0x00, 0x00, 0x09]);
        assert_eq!(&enc[8..], &p[..]);
    }
    #[test]
    fn presets() {
        assert_eq!(preset_set(1, 5), vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x05, 0xFF]);
        assert_eq!(preset_recall(1, 5), vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x05, 0xFF]);
    }

    #[test]
    fn camera_error_ack_is_not_reported_as_success() {
        let camera = UdpSocket::bind("127.0.0.1:0").expect("bind fake camera");
        let port = camera.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let mut packet = [0u8; 128];
            let (_, source) = camera.recv_from(&mut packet).expect("receive command");
            camera
                .send_to(&[0x01, 0x00, 0x00, 0x03, 0, 0, 0, 1, 0x90, 0x41, 0xFF], source)
                .unwrap();
        });
        let result = send_blocking("127.0.0.1", port, Protocol::SonyUdp, &power(1, true));
        assert!(result.unwrap_err().contains("camera rejected command"));
    }

    #[test]
    fn android_image_and_picture_commands_match_the_manual() {
        let cases = [
            ("ae_mode", 0x00, vec![0x81, 0x01, 0x04, 0x39, 0x00, 0xFF]),
            ("shutter", 0x10, vec![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, 0x01, 0x00, 0xFF]),
            ("iris", 0x08, vec![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x08, 0xFF]),
            ("gain_limit", 0x08, vec![0x81, 0x01, 0x04, 0x2C, 0x08, 0xFF]),
            ("bright", 0x08, vec![0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x00, 0x08, 0xFF]),
            ("backlight", 0x01, vec![0x81, 0x01, 0x04, 0x33, 0x02, 0xFF]),
            ("wdr", 0x03, vec![0x81, 0x01, 0x04, 0x51, 0x00, 0x00, 0x00, 0x03, 0xFF]),
            ("nr2d", 0x03, vec![0x81, 0x01, 0x04, 0x53, 0x03, 0xFF]),
            ("nr3d", 0x05, vec![0x81, 0x01, 0x04, 0x54, 0x05, 0xFF]),
            ("aperture", 0x08, vec![0x81, 0x01, 0x04, 0x42, 0x00, 0x00, 0x00, 0x08, 0xFF]),
            ("brightness", 0x07, vec![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00, 0x07, 0xFF]),
            ("contrast", 0x07, vec![0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00, 0x07, 0xFF]),
            ("saturation", 0x04, vec![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00, 0x04, 0xFF]),
            ("flip", 0x03, vec![0x81, 0x01, 0x04, 0xA4, 0x03, 0xFF]),
            ("bw", 0x01, vec![0x81, 0x01, 0x04, 0x63, 0x04, 0xFF]),
        ];
        for (op, value, expected) in cases {
            assert_eq!(named_command(op, 1, value, 0, 0), Some(expected), "{op}");
        }
        assert_eq!(named_command("not-a-command", 1, 0, 0, 0), None);
    }
}
