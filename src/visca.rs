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
        Ok((n, _)) => Ok(format!("sent {}B seq={} ack {}B", packet.len(), seq, n)),
        Err(_) => Ok(format!("sent {}B seq={} (no ack, ok on LAN)", packet.len(), seq)),
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
}
