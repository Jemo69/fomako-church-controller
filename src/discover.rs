//! Find FoMaKo cameras on the local network (std only, native).
//!
//! Probes per host: TCP 5678 (IP-VISCA), UDP 52381 (Sony VISCA inquiry),
//! HTTP :80 title sniff. Plus ONVIF WS-Discovery multicast.
//!
//! Browsers (wasm) cannot open raw sockets, so on wasm the scan runs
//! through the LAN relay (`scripts/visca-relay.py`, same machine that
//! serves the web UI): `GET /api/scan?subnet=192.168.1`.

#[cfg(not(target_arch = "wasm32"))]
use std::net::{TcpStream, UdpSocket};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::Ordering;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct FoundCamera {
    pub ip: String,
    pub tcp5678: bool,
    pub udp52381: bool,
    pub http_hint: String,
    pub onvif: bool,
}

impl FoundCamera {
    pub fn detail(&self) -> String {
        let mut bits: Vec<String> = vec![];
        if self.tcp5678 {
            bits.push("VISCA :5678".to_string());
        }
        if self.udp52381 {
            bits.push("Sony :52381".to_string());
        }
        if self.onvif {
            bits.push("ONVIF".to_string());
        }
        if !self.http_hint.is_empty() {
            bits.push(format!("web “{}”", self.http_hint));
        } else {
            bits.push("web :80".to_string());
        }
        bits.join(" · ")
    }
}

/// Events a scan worker pushes; the UI thread drains them in `poll`.
#[derive(Clone, Debug)]
pub enum ScanEvent {
    Found(FoundCamera),
}

#[derive(Debug, Default)]
pub struct ScanHandle {
    pub done: AtomicBool,
    pub abort: AtomicBool,
    pub checked: AtomicUsize,
    pub total: AtomicUsize,
}

/// This machine's LAN address via the connect-without-traffic trick.
/// Browsers have no sockets: returns None there (subnet comes from the URL hash).
#[cfg(not(target_arch = "wasm32"))]
pub fn local_ipv4() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    // No packet is sent; connect() only picks the route's source address.
    sock.connect("8.8.8.8:80").ok()?;
    let ip = sock.local_addr().ok()?.ip().to_string();
    if ip.starts_with("127.") {
        return None;
    }
    Some(ip)
}

#[cfg(target_arch = "wasm32")]
pub fn local_ipv4() -> Option<String> {
    None
}

/// "192.168.1.5" -> "192.168.1". Falls back to the FoMaKo default subnet.
pub fn prefix_of(ip: &str) -> Option<String> {
    let p: Vec<&str> = ip.split('.').collect();
    if p.len() == 4 && p.iter().all(|o| o.parse::<u8>().is_ok()) {
        Some(format!("{}.{}.{}", p[0], p[1], p[2]))
    } else {
        None
    }
}

pub fn detect_prefix() -> String {
    local_ipv4()
        .and_then(|ip| prefix_of(&ip))
        .unwrap_or_else(|| "192.168.5".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn tcp_open(ip: &str, port: u16, ms: u64) -> bool {
    use std::net::ToSocketAddrs;
    let target = format!("{}:{}", ip, port);
    match target
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next())
    {
        Some(a) => TcpStream::connect_timeout(&a, Duration::from_millis(ms)).is_ok(),
        None => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn http_hint(ip: &str) -> String {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    let target = format!("{}:80", ip);
    let addr = match target
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next())
    {
        Some(a) => a,
        None => return String::new(),
    };
    let mut s = match TcpStream::connect_timeout(&addr, Duration::from_millis(400)) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let _ = s.set_read_timeout(Some(Duration::from_millis(600)));
    let _ = s.set_write_timeout(Some(Duration::from_millis(400)));
    let req = format!(
        "GET / HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        ip
    );
    if s.write_all(req.as_bytes()).is_err() {
        return String::new();
    }
    let mut buf = [0u8; 4096];
    let mut data = Vec::new();
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if data.len() > 16384 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&data).to_lowercase();
    let mut hint = String::new();
    if let Some(i) = text.find("<title>") {
        let rest = &text[i + 7..];
        if let Some(j) = rest.find("</title>") {
            hint = rest[..j.min(64)].trim().to_string();
        }
    }
    for key in ["fomako", "ptz", "ip camera", "network camera"] {
        if text.contains(key) {
            if hint.is_empty() {
                hint = key.to_string();
            } else if !hint.contains(key) {
                hint.push_str(&format!(" +{}", key));
            }
            break;
        }
    }
    hint
}

#[cfg(not(target_arch = "wasm32"))]
fn visca_udp_ok(ip: &str) -> bool {
    // Sony VISCA-over-IP power inquiry; any reply counts.
    let payload = [0x81u8, 0x09, 0x04, 0x00, 0xFF];
    let pkt = crate::visca::sony_encapsulate(&payload, 1);
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(450)));
    let _ = sock.set_write_timeout(Some(Duration::from_millis(400)));
    if sock.send_to(&pkt, format!("{}:52381", ip)).is_err() {
        return false;
    }
    let mut buf = [0u8; 128];
    matches!(sock.recv_from(&mut buf), Ok((n, _)) if n >= 8)
}

#[cfg(not(target_arch = "wasm32"))]
fn probe_host(ip: String) -> Option<FoundCamera> {
    let tcp5678 = tcp_open(&ip, 5678, 350);
    let hint = http_hint(&ip);
    let web = !hint.is_empty() || tcp_open(&ip, 80, 250);
    let udp52381 = if tcp5678 || web {
        visca_udp_ok(&ip)
    } else {
        false
    };
    if tcp5678 || udp52381 || !hint.is_empty() {
        Some(FoundCamera {
            ip,
            tcp5678,
            udp52381,
            http_hint: hint,
            onvif: false,
        })
    } else {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
const ONVIF_MCAST: &str = "239.255.255.250:3702";

#[cfg(not(target_arch = "wasm32"))]
fn onvif_probe_xml() -> String {
    // MessageID only needs to be unique-ish; responders don't validate it.
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(1);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><e:Envelope xmlns:e="http://www.w3.org/2003/05/soap-envelope" xmlns:w="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dn="http://www.onvif.org/ver10/network/wsdl"><e:Header><w:MessageID>uuid:fomako-{id}</w:MessageID><w:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</w:To><w:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</w:Action></e:Header><e:Body><d:Probe><d:Types>dn:NetworkVideoTransmitter</d:Types></d:Probe></e:Body></e:Envelope>"#
    )
}

/// Pull `http(s)://A.B.C.D...` hosts out of WS-Discovery responses.
#[cfg(not(target_arch = "wasm32"))]
fn extract_ips(text: &str) -> Vec<String> {
    let mut out = vec![];
    let lower = text.to_lowercase();
    for marker in ["http://", "https://"] {
        let mut rest = lower.as_str();
        while let Some(i) = rest.find(marker) {
            let host = &rest[i + marker.len()..];
            let end = host
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != ':')
                .unwrap_or(host.len());
            let host = host[..end].to_string();
            let ip = host.split(':').next().unwrap_or("").to_string();
            if prefix_of(&ip).is_some() && !out.contains(&ip) {
                out.push(ip);
            }
            rest = &rest[i + marker.len()..];
        }
    }
    out
}

/// Multicast WS-Discovery probe; pushes Found events for responders.
#[cfg(not(target_arch = "wasm32"))]
pub fn onvif_discover(
    queue: &Arc<Mutex<Vec<ScanEvent>>>,
    handle: &Arc<ScanHandle>,
    listen_ms: u64,
) {
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = sock.set_broadcast(true);
    if sock
        .send_to(onvif_probe_xml().as_bytes(), ONVIF_MCAST)
        .is_err()
    {
        return;
    }
    let deadline = std::time::Instant::now() + Duration::from_millis(listen_ms);
    let mut buf = vec![0u8; 65535];
    let mut seen: Vec<String> = vec![];
    while std::time::Instant::now() < deadline && !handle.abort.load(Ordering::Relaxed) {
        match sock.recv_from(&mut buf) {
            Ok((n, _)) => {
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                for ip in extract_ips(&text) {
                    if seen.contains(&ip) {
                        continue;
                    }
                    seen.push(ip.clone());
                    let found = FoundCamera {
                        ip,
                        tcp5678: false,
                        udp52381: false,
                        http_hint: String::new(),
                        onvif: true,
                    };
                    if let Ok(mut q) = queue.lock() {
                        q.push(ScanEvent::Found(found));
                    }
                }
            }
            Err(_) => continue,
        }
    }
}

/// Scan 1..=254 of `prefix` ("192.168.1") + ONVIF multicast.
/// Runs on the calling thread; call it from a background thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn scan_subnet(
    prefix: &str,
    queue: Arc<Mutex<Vec<ScanEvent>>>,
    handle: Arc<ScanHandle>,
) {
    onvif_discover(&queue, &handle, 2500);
    if handle.abort.load(Ordering::Relaxed) {
        handle.done.store(true, Ordering::SeqCst);
        return;
    }
    let hosts: Vec<String> = (1u16..=254).map(|i| format!("{}.{}", prefix, i)).collect();
    handle.total.store(hosts.len(), Ordering::SeqCst);
    std::thread::scope(|sc| {
        // ~32 workers; each takes a chunk.
        for chunk in hosts.chunks(8) {
            let queue = queue.clone();
            let handle = handle.clone();
            let chunk: Vec<String> = chunk.to_vec();
            sc.spawn(move || {
                for ip in chunk {
                    if handle.abort.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(found) = probe_host(ip) {
                        if let Ok(mut q) = queue.lock() {
                            q.push(ScanEvent::Found(found));
                        }
                    }
                    handle.checked.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
    });
    handle.done.store(true, Ordering::SeqCst);
}

// ---------------------------------------------------------------- relay client
// The LAN relay (`scripts/visca-relay.py`) forwards VISCA and scans on
// behalf of browsers, which cannot open raw TCP/UDP sockets.

fn relay_base(relay_url: &str) -> String {
    relay_url.trim().trim_end_matches('/').to_string()
}

/// POST /visca {ip,port,proto,payload_hex} via plain HTTP (native).
#[cfg(not(target_arch = "wasm32"))]
pub fn relay_post_visca(
    relay_url: &str,
    ip: &str,
    port: u16,
    proto: &str,
    payload_hex: &str,
) -> Result<String, String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    let base = relay_base(relay_url);
    let hostport = base
        .strip_prefix("http://")
        .or_else(|| base.strip_prefix("HTTP://"))
        .ok_or_else(|| "relay URL must start with http://".to_string())?;
    let hostport = hostport.split('/').next().unwrap_or(hostport);
    let addr = hostport
        .to_socket_addrs()
        .map_err(|e| format!("relay {}: {}", hostport, e))?
        .next()
        .ok_or_else(|| format!("relay: cannot resolve {}", hostport))?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(1500))
        .map_err(|e| format!("relay {}: {}", hostport, e))?;
    let _ = s.set_read_timeout(Some(Duration::from_millis(2500)));
    let _ = s.set_write_timeout(Some(Duration::from_millis(1000)));
    let body = format!(
        "{{\"ip\":\"{}\",\"port\":{},\"proto\":\"{}\",\"payload\":\"{}\"}}",
        ip, port, proto, payload_hex
    );
    let req = format!(
        "POST /visca HTTP/1.0\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        hostport,
        body.len(),
        body
    );
    s.write_all(req.as_bytes())
        .map_err(|e| format!("relay write: {}", e))?;
    let mut buf = [0u8; 4096];
    let mut data = Vec::new();
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if data.len() > 16384 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&data).to_string();
    let body = text.split("\r\n\r\n").last().unwrap_or("").to_string();
    let reply: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| format!("relay: bad reply: {}", body.chars().take(160).collect::<String>()))?;
    if reply.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        Ok(reply.get("msg").and_then(serde_json::Value::as_str).unwrap_or("sent").to_string())
    } else {
        Err(reply
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("relay rejected command")
            .to_string())
    }
}

/// GET /api/scan?subnet=192.168.1 (native, blocking).
#[cfg(not(target_arch = "wasm32"))]
pub fn relay_get_scan(relay_url: &str, prefix: &str) -> Result<Vec<FoundCamera>, String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    let base = relay_base(relay_url);
    let hostport = base
        .strip_prefix("http://")
        .ok_or_else(|| "relay URL must start with http://".to_string())?;
    let hostport = hostport.split('/').next().unwrap_or(hostport);
    let addr = hostport
        .to_socket_addrs()
        .map_err(|e| format!("relay {}: {}", hostport, e))?
        .next()
        .ok_or_else(|| format!("relay: cannot resolve {}", hostport))?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map_err(|e| format!("relay {}: {}", hostport, e))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = s.set_write_timeout(Some(Duration::from_millis(1000)));
    let req = format!(
        "GET /api/scan?subnet={} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        prefix, hostport
    );
    s.write_all(req.as_bytes())
        .map_err(|e| format!("relay write: {}", e))?;
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if data.len() > 262144 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&data).to_string();
    let body = text.split("\r\n\r\n").last().unwrap_or("").to_string();
    parse_scan_json(&body)
}

pub fn parse_scan_json(body: &str) -> Result<Vec<FoundCamera>, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("relay scan: {}", e))?;
    let arr = v
        .get("found")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "relay: bad scan reply".to_string())?;
    Ok(arr
        .iter()
        .filter_map(|o| {
            Some(FoundCamera {
                ip: o.get("ip")?.as_str()?.to_string(),
                tcp5678: o.get("tcp5678").and_then(|b| b.as_bool()).unwrap_or(false),
                udp52381: o
                    .get("udp52381")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
                http_hint: o
                    .get("http_hint")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
                onvif: o.get("onvif").and_then(|b| b.as_bool()).unwrap_or(false),
            })
        })
        .collect())
}

// ---------------------------------------------------------------- wasm fetch
// Same relay calls from the browser build (same-origin, no CORS issue).

#[cfg(target_arch = "wasm32")]
mod wasm_net {
    use super::FoundCamera;
    use serde_json::Value;
    use wasm_bindgen::prelude::*;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    async fn fetch_text(url: &str, opts: Option<RequestInit>) -> Result<String, String> {
        let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
        let req = match opts {
            Some(o) => Request::new_with_str_and_init(url, &o)
                .map_err(|_| "bad request".to_string())?,
            None => Request::new_with_str(url).map_err(|_| "bad request".to_string())?,
        };
        let resp: Response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req))
            .await
            .map_err(|_| "fetch failed (is the relay serving this page?)".to_string())?
            .into();
        let status = resp.status();
        let text = wasm_bindgen_futures::JsFuture::from(
            resp.text().map_err(|_| "no body".to_string())?,
        )
        .await
        .map_err(|_| "read failed".to_string())?;
        let text = js_sys::JsString::from(text).into();
        if !(200..300).contains(&status) {
            return Err(format!("relay HTTP {status}: {text}"));
        }
        Ok(text)
    }

    pub async fn post_visca(
        relay_url: &str,
        ip: &str,
        port: u16,
        proto: &str,
        payload_hex: &str,
    ) -> Result<String, String> {
        let opts = RequestInit::new();
        opts.set_method("POST");
        opts.set_mode(RequestMode::SameOrigin);
        let headers = web_sys::Headers::new().map_err(|_| "headers failed".to_string())?;
        headers
            .set("Content-Type", "application/json")
            .map_err(|_| "header failed".to_string())?;
        opts.set_headers_headers(&headers);
        let body = format!(
            "{{\"ip\":\"{}\",\"port\":{},\"proto\":\"{}\",\"payload\":\"{}\"}}",
            ip, port, proto, payload_hex
        );
        opts.set_body(&JsValue::from_str(&body));
        let url = format!("{}/visca", super::relay_base(relay_url));
        let text = fetch_text(&url, Some(opts)).await?;
        let reply: Value = serde_json::from_str(&text)
            .map_err(|_| "relay returned an invalid reply".to_string())?;
        if reply.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(reply
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("sent")
                .to_string())
        } else {
            Err(reply
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("relay rejected command")
                .to_string())
        }
    }

    pub async fn get_scan(relay_url: &str, prefix: &str) -> Result<Vec<FoundCamera>, String> {
        let url = format!("{}/api/scan?subnet={}", super::relay_base(relay_url), prefix);
        let body = fetch_text(&url, None).await?;
        super::parse_scan_json(&body)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_net::{get_scan as relay_get_scan_async, post_visca as relay_post_visca_async};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix_parsing() {
        assert_eq!(prefix_of("192.168.5.163"), Some("192.168.5".to_string()));
        assert_eq!(prefix_of("nope"), None);
    }
    #[test]
    fn onvif_ip_extraction() {
        let s = "<d:XAddrs>http://192.168.1.64:2000/onvif/device_service http://[fe80::1]/x</d:XAddrs>";
        let ips = extract_ips(s);
        assert!(ips.contains(&"192.168.1.64".to_string()));
    }
    #[test]
    fn scan_finds_fake_loopback_camera() {
        // Fake IP-VISCA TCP :5678 (accept + close is enough).
        let tcp = std::net::TcpListener::bind("127.0.0.1:5678").expect("bind 5678");
        std::thread::spawn(move || {
            for conn in tcp.incoming().take(300) {
                if let Ok(s) = conn {
                    drop(s);
                }
            }
        });
        // Fake Sony VISCA UDP :52381: any 8+ byte reply counts.
        let udp = std::net::UdpSocket::bind("127.0.0.1:52381").expect("bind 52381");
        std::thread::spawn(move || {
            let mut b = [0u8; 128];
            for _ in 0..300 {
                match udp.recv_from(&mut b) {
                    Ok((_, src)) => {
                        let _ = udp.send_to(
                            &[0x02, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01, 0x90, 0x41, 0xFF],
                            src,
                        );
                    }
                    Err(_) => break,
                }
            }
        });
        // Direct probe first (fast, no multicast).
        let f = probe_host("127.0.0.1".to_string()).expect("probe finds fake cam");
        assert!(f.tcp5678, "TCP 5678 should be open");
        assert!(f.udp52381, "UDP 52381 should answer");
        // Full subnet path: queue + handle, as the UI uses it.
        let queue = Arc::new(Mutex::new(Vec::new()));
        let handle = Arc::new(ScanHandle::default());
        scan_subnet("127.0.0", queue.clone(), handle.clone());
        assert!(handle.done.load(Ordering::SeqCst));
        let q = queue.lock().unwrap();
        let hit = q.iter().find_map(|e| match e {
            ScanEvent::Found(f) if f.ip == "127.0.0.1" => Some(f.clone()),
            _ => None,
        });
        assert!(hit.is_some(), "scan_subnet should report 127.0.0.1, got {:?}", *q);
    }
    /// Manual on-site check: `cargo test scan_live -- --ignored --nocapture`.
    /// Scans this machine's subnet and prints what answered (nothing here
    /// means no cameras on this network, not a broken scanner).
    #[test]
    #[ignore]
    fn scan_live() {
        let prefix = detect_prefix();
        println!("live scan on {}.0/24 ...", prefix);
        let queue = Arc::new(Mutex::new(Vec::new()));
        let handle = Arc::new(ScanHandle::default());
        scan_subnet(&prefix, queue.clone(), handle.clone());
        let q = queue.lock().unwrap();
        println!(
            "checked {} hosts, {} responders:",
            handle.checked.load(Ordering::SeqCst),
            q.len()
        );
        for e in q.iter() {
            if let ScanEvent::Found(f) = e {
                println!("  {} -- {}", f.ip, f.detail());
            }
        }
    }
    #[test]
    fn scan_button_wires_handler() {
        // Native headless click on "Scan network" must create a ScanState.
        // Tall viewport so the whole Cameras page fits without scrolling.
        let mut rt = crate::model::App::with_test_runtime_sized((1120.0, 4000.0));
        rt.frame(0.0);
        // Go to the Cameras page first (default is Control).
        rt.update(|s, _| s.page = crate::model::Page::Cameras);
        // Let the Control page's exit transition finish so it can't cover the click.
        for _ in 0..120 {
            rt.frame(1.0 / 60.0);
        }
        let (x, y) = rt.find("scan-btn").expect("scan button present");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        let scan = rt.state.scan.as_ref().expect("scan started");
        assert!(scan.running);
        assert!(scan.message.starts_with("Scanning"), "{}", scan.message);
        // Stop it so no background thread outlives the test.
        scan
            .handle
            .abort
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    #[test]
    fn image_page_pairs_each_apply_with_its_value() {
        // The Image page rule: one row per setting, ±/value/Apply together, so
        // the value you dial is the one Apply sends.
        let mut rt = crate::model::App::with_test_runtime_sized((1120.0, 4000.0));
        rt.frame(0.0);
        rt.update(|s, _| s.page = crate::model::Page::Image);
        for _ in 0..120 {
            rt.frame(1.0 / 60.0);
        }
        let start = rt.state.shutter;
        let (px, py) = rt.find("shutter-plus").expect("shutter + present");
        rt.pointer_down(px, py);
        rt.pointer_up(px, py);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.shutter, start + 1, "+ should raise the shutter value");
        let (ax, ay) = rt.find("shutter-apply").expect("shutter Apply present");
        rt.pointer_down(ax, ay);
        rt.pointer_up(ax, ay);
        rt.frame(1.0 / 60.0);
        let sent = format!("00 {:02X} FF", start + 1);
        assert!(
            rt.state.status.contains("Shutter") && rt.state.status.contains(&sent),
            "Apply should send the dialed shutter value ({sent}), status: {}",
            rt.state.status
        );
        // Every setting keeps its own Apply; the grouped rows are gone.
        for key in ["iris-apply", "gain-apply", "brightlvl-apply", "nr2d-apply", "nr3d-apply", "rgain-apply", "bgain-apply"] {
            assert!(rt.find(key).is_some(), "{key} missing on the Image page");
        }
        assert!(rt.find("nr-a").is_none(), "combined noise-reduction row should be gone");
    }

    #[test]
    fn image_page_rows_fit_narrow_windows() {
        // The Image page keeps −, the value, + and Apply in one group, so a
        // narrow window wraps the group instead of pushing Apply off-canvas.
        for w in [320.0f32, 390.0] {
            let mut rt = crate::model::App::with_test_runtime_sized((w, 3000.0));
            rt.frame(0.0);
            rt.update(|s, _| s.page = crate::model::Page::Image);
            for _ in 0..120 {
                rt.frame(1.0 / 60.0);
            }
            for key in [
                "shutter-apply",
                "iris-apply",
                "nr3d-apply",
                "rgain-apply",
                "bgain-apply",
                "gamma-apply",
            ] {
                let (rect, _) = rt
                    .presented(key)
                    .unwrap_or_else(|| panic!("{key} missing at {w}"));
                assert!(rect[0] >= -0.5, "{key} starts off-canvas at {w}: {rect:?}");
                assert!(
                    rect[0] + rect[2] <= w + 0.5,
                    "{key} overflows a {w} wide window: {rect:?}"
                );
            }
        }
    }

    #[test]
    fn tab_buttons_wire_handlers() {
        let mut rt = crate::model::App::with_test_runtime_sized((1120.0, 4000.0));
        rt.frame(0.0);
        assert_eq!(rt.state.page, crate::model::Page::Control);
        let (x, y) = rt.find("tab-Image").expect("image tab present");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.page, crate::model::Page::Image);
    }
    #[test]
    fn narrow_and_short_viewports_keep_controls_within_bounds() {
        for (w, h) in [(320.0, 540.0), (640.0, 340.0)] {
            let mut rt = crate::model::App::with_test_runtime_sized((w, h));
            for page in crate::model::Page::all() {
                rt.update(|s, _| s.page = page);
                rt.frame(0.0);
                let (tx, ty) = rt.find("theme").expect("theme button present");
                assert!(tx <= w, "theme button overflowed {w}x{h}: x={tx}");
                assert!(ty <= h, "theme button outside height {w}x{h}: y={ty}");
                if page == crate::model::Page::Control {
                    let (px, _) = rt.find("pt-ne").expect("pt-ne present");
                    let (sx, _) = rt.find("pt-stop").expect("pt-stop present");
                    assert!(px <= w && sx <= w, "PTZ overflowed {w}: px={px}, sx={sx}");
                }
            }
        }
    }
    #[test]
    fn ffmpeg_search_covers_env_path_and_exe_dir() {
        // $FFMPEG_BIN always wins, even for a path that doesn't exist.
        let old_bin = std::env::var_os("FFMPEG_BIN");
        std::env::set_var("FFMPEG_BIN", "/tmp/does-not-exist-ffmpeg");
        let cands = crate::preview::ffmpeg_candidates();
        assert!(!cands.is_empty());
        assert_eq!(
            cands[0].to_string_lossy().as_ref(),
            "/tmp/does-not-exist-ffmpeg"
        );
        match old_bin {
            Some(v) => std::env::set_var("FFMPEG_BIN", v),
            None => std::env::remove_var("FFMPEG_BIN"),
        }
        // The app's own folder is always searched (bundled ffmpeg next to the exe).
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let cands = crate::preview::ffmpeg_candidates();
        if let Some(dir) = exe_dir {
            #[cfg(target_os = "windows")]
            let want = dir.join("ffmpeg.exe");
            #[cfg(not(target_os = "windows"))]
            let want = dir.join("ffmpeg");
            assert!(
                cands.contains(&want),
                "exe dir must be searched: {:?}",
                want
            );
        }
    }
    #[test]
    fn rtsp_urls() {
        assert_eq!(
            crate::preview::rtsp_url("192.168.5.163", 554, "", ""),
            "rtsp://192.168.5.163:554/1"
        );
        assert_eq!(
            crate::preview::rtsp_url("192.168.5.163", 554, "admin", "admin"),
            "rtsp://admin:admin@192.168.5.163:554/1"
        );
        // '@ : / ? #' must never break the URL structure.
        assert_eq!(
            crate::preview::rtsp_url("10.0.0.2", 8554, "ad@min", "p:a/s?s#"),
            "rtsp://ad%40min:p%3Aa%2Fs%3Fs%23@10.0.0.2:8554/1"
        );
        assert_eq!(crate::preview::rtsp_url("10.0.0.2", 554, "admin", ""), "rtsp://admin:@10.0.0.2:554/1");
    }
    #[test]
    fn old_configs_without_login_still_load() {
        // Configs saved before usernames existed must keep working (defaults admin/admin).
        let old = r#"[{"name":"Pulpit","ip":"192.168.5.163","protocol":"SonyUdp","address":1}]"#;
        let cams: Vec<crate::model::CameraConfig> = serde_json::from_str(old).unwrap();
        assert_eq!(cams[0].username, "admin");
        assert_eq!(cams[0].password, "admin");
    }
    /// 2x2 BMP, 70 bytes, hand-built: exercises cache.load -> size/error/pixels
    /// without needing ffmpeg or network.
    fn tiny_bmp() -> Vec<u8> {
        let mut b = vec![];
        b.extend_from_slice(b"BM");
        b.extend_from_slice(&70u32.to_le_bytes()); // file size
        b.extend_from_slice(&[0u8; 4]);
        b.extend_from_slice(&54u32.to_le_bytes()); // pixel offset
        b.extend_from_slice(&40u32.to_le_bytes()); // DIB size
        b.extend_from_slice(&2i32.to_le_bytes()); // w
        b.extend_from_slice(&2i32.to_le_bytes()); // h
        b.extend_from_slice(&1u16.to_le_bytes()); // planes
        b.extend_from_slice(&24u16.to_le_bytes()); // bpp
        b.extend_from_slice(&0u32.to_le_bytes()); // no compression
        b.extend_from_slice(&16u32.to_le_bytes()); // image size
        b.extend_from_slice(&[0u8; 16]); // ppm + colours
        // bottom-up BGR, rows padded to 4 bytes: red, green / blue, white
        b.extend_from_slice(&[0, 0, 255, 0, 0, 255, 0, 0]);
        b.extend_from_slice(&[255, 0, 0, 0, 255, 255, 255, 0]);
        b
    }
    #[test]
    fn preview_image_decodes_and_reports_size() {
        use std::sync::{Arc, Mutex};
        let wake: rune::Waker = Arc::new(|| {});
        let cache = rune::ImageCache::new(wake);
        let img = cache.load(tiny_bmp());
        // Decode happens on worker threads; poll for it briefly.
        let mut size = None;
        for _ in 0..100 {
            if let Some(e) = img.error() {
                panic!("decode error: {}", e.0);
            }
            size = img.size();
            if size.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_eq!(size, Some([2, 2]));
        // pixels() must hand back a drawable handle once decoded.
        let _px = img.pixels();
    }
    #[test]
    fn scan_json_parsing() {
        let b = r#"{"found":[{"ip":"192.168.5.163","tcp5678":true,"udp52381":true,"http_hint":"fomako","onvif":false}]}"#;
        let v = parse_scan_json(b).unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].udp52381);
    }
}
