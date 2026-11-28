//! Live camera preview via HTTP JPEG snapshots (native) or the LAN relay (web).
//! RTSP/ffmpeg is a fallback for firmware without /action_snapshot.
//!
//! Rune's `Player` can't play RTSP (AVFoundation/MediaFoundation don't speak
//! it), so the desktop app grabs a JPEG every ~1.5s with ffmpeg and draws it
//! through `ImageCache`. The browser build can't run ffmpeg either, so it
//! pulls JPEGs from `GET /api/snapshot?ip=` on the relay instead.

#[cfg(target_arch = "wasm32")]
use std::sync::atomic::Ordering;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crate::model::App;

#[derive(Debug)]
pub enum PreviewUpdate {
    Frame { gen: u64, seq: u64, jpeg: Vec<u8> },
    Error { gen: u64, seq: u64, msg: String },
}

/// Everywhere we look for ffmpeg, in order. Church booth PCs are rarely on PATH,
/// so: $FFMPEG_BIN, PATH, next to the app binary, then the usual per-OS spots.
#[cfg(not(target_arch = "wasm32"))]
pub fn ffmpeg_candidates() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut out: Vec<PathBuf> = vec![];
    if let Some(v) = std::env::var_os("FFMPEG_BIN") {
        let p = PathBuf::from(v);
        if !p.as_os_str().is_empty() {
            out.push(p);
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if dir.as_os_str().is_empty() {
                continue;
            }
            #[cfg(target_os = "windows")]
            out.push(dir.join("ffmpeg.exe"));
            #[cfg(not(target_os = "windows"))]
            out.push(dir.join("ffmpeg"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(target_os = "windows")]
            out.push(dir.join("ffmpeg.exe"));
            #[cfg(not(target_os = "windows"))]
            out.push(dir.join("ffmpeg"));
        }
    }
    #[cfg(target_os = "windows")]
    for fixed in [
        "C:\\ffmpeg\\bin\\ffmpeg.exe",
        "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
    ] {
        out.push(PathBuf::from(fixed));
    }
    #[cfg(target_os = "windows")]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let mut p = PathBuf::from(local);
        p.push("ffmpeg\\bin\\ffmpeg.exe");
        out.push(p);
    }
    #[cfg(target_os = "macos")]
    for fixed in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
        out.push(PathBuf::from(fixed));
    }
    #[cfg(target_os = "linux")]
    for fixed in [
        "/usr/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/snap/bin/ffmpeg",
    ] {
        out.push(PathBuf::from(fixed));
    }
    out
}

/// First ffmpeg candidate that exists and runs, if any.
#[cfg(not(target_arch = "wasm32"))]
fn ffmpeg_validated() -> Option<String> {
    for cand in ffmpeg_candidates() {
        let s = cand.to_string_lossy().into_owned();
        if cand.is_file()
            && std::process::Command::new(&cand)
                .arg("-version")
                .output()
                .is_ok()
        {
            return Some(s);
        }
    }
    None
}

/// Best ffmpeg binary path, or the plain name (so error messages still tell
/// the user what to install). Re-resolved on every grab: installing ffmpeg
/// mid-session just starts working.
#[cfg(not(target_arch = "wasm32"))]
pub fn ffmpeg_bin() -> String {
    if let Some(p) = ffmpeg_validated() {
        return p;
    }
    // Last resort: hope PATH resolves at spawn time (covers wrappers/shims).
    if std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok()
    {
        return "ffmpeg".to_string();
    }
    "ffmpeg".to_string()
}

/// Validated ffmpeg path for display (Help page), or empty when absent.
/// The fetcher itself uses [`ffmpeg_bin`], so it never goes stale.
#[cfg(not(target_arch = "wasm32"))]
pub fn ffmpeg_path_or_empty() -> String {
    ffmpeg_validated().unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
pub fn ffmpeg_path_or_empty() -> String {
    // Browsers can't run ffmpeg; the relay does (see its /api/snapshot).
    String::new()
}

/// RTSP port (554), overridable for remapped/NVR setups: `FOMAKO_RTSP_PORT=8554`.
#[cfg(not(target_arch = "wasm32"))]
pub fn rtsp_port() -> u16 {
    std::env::var("FOMAKO_RTSP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(554)
}

/// Percent-encode a userinfo part (usernames/passwords with @ : / ? # break URLs).
/// Keeps unreserved + sub-delims (except % stays encoded), encodes the rest as UTF-8.
pub fn userinfo_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'0'..=b'9'
            | b'A'..=b'Z'
            | b'a'..=b'z'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'=' => out.push(*b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// rtsp://[user:pass@]ip:port/1 — the FoMaKo firmware's primary stream path.
pub fn rtsp_url(ip: &str, port: u16, user: &str, pass: &str) -> String {
    rtsp_url_for_path(ip, port, user, pass, "1")
}

fn rtsp_url_for_path(ip: &str, port: u16, user: &str, pass: &str, path: &str) -> String {
    if user.is_empty() {
        format!("rtsp://{}:{}/{}", ip, port, path)
    } else {
        format!(
            "rtsp://{}:{}@{}:{}/{}",
            userinfo_encode(user),
            userinfo_encode(pass),
            ip,
            port,
            path
        )
    }
}

/// FoMaKo exposes a lightweight JPEG still at /action_snapshot. It works on
/// phones and avoids RTSP codec/authentication differences between firmware.
#[cfg(not(target_arch = "wasm32"))]
fn grab_http_snapshot_at(ip: &str, port: u16) -> Option<Vec<u8>> {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};

    let addr = (ip, port).to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(1200)).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(2500))).ok()?;
    stream.set_write_timeout(Some(Duration::from_millis(800))).ok()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let request = format!(
        "GET /action_snapshot?t={} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\nCache-Control: no-cache\r\n\r\n",
        stamp, ip, port
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        if data.len() > 8 * 1024 * 1024 {
            return None;
        }
    }
    let header_end = data.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let head = String::from_utf8_lossy(&data[..header_end]);
    if !head.lines().next()?.contains(" 200 ") {
        return None;
    }
    let body = &data[header_end..];
    if body.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(body.to_vec())
    } else {
        None
    }
}

/// One JPEG frame from the camera. Prefer the firmware's HTTP still endpoint;
/// older firmware falls back to RTSP paths /1 and /live/av0.
#[cfg(not(target_arch = "wasm32"))]
pub fn grab_frame(ip: &str, user: &str, pass: &str) -> Result<Vec<u8>, String> {
    if let Some(jpeg) = grab_http_snapshot_at(ip, 80) {
        return Ok(jpeg);
    }
    let mut last_error = String::new();
    for path in ["1", "live/av0"] {
        match grab_ffmpeg_frame(ip, user, pass, path) {
            Ok(jpeg) => return Ok(jpeg),
            Err(error) => last_error = error,
        }
    }
    Err(if last_error.is_empty() {
        "preview unavailable".to_string()
    } else {
        last_error
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn grab_ffmpeg_frame(ip: &str, user: &str, pass: &str, path: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new(ffmpeg_bin())
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-rtsp_transport",
            "tcp",
            "-timeout",
            "4000000",
            "-i",
            &rtsp_url_for_path(ip, rtsp_port(), user, pass, path),
            // Keyframes only: a 1-frame grab starting mid-GOP decodes to gray garbage.
            "-skip_frame",
            "nokey",
            "-frames:v",
            "1",
            "-q:v",
            "4",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "-",
        ])
        .output();
    match out {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
            "ffmpeg not found — install it (apt install ffmpeg / brew install ffmpeg / winget install ffmpeg), or drop ffmpeg(.exe) next to the app".to_string(),
        ),
        Err(e) => Err(format!("ffmpeg: {}", e)),
        Ok(o) if o.status.success() && !o.stdout.is_empty() => Ok(o.stdout),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let brief: String = err.lines().last().unwrap_or("ffmpeg failed").to_string();
            let mut msg: String = brief.chars().take(220).collect();
            if err.contains("401") {
                msg.push_str(" — check camera login (FoMaKo default admin/admin)");
            }
            Err(msg)
        }
    }
}

/// Background loop: grab → slot → wake, until `gen` goes stale.
#[cfg(not(target_arch = "wasm32"))]
pub fn grab_loop(
    ip: String,
    user: String,
    pass: String,
    gen: u64,
    cur: Arc<AtomicU64>,
    slot: Arc<Mutex<Option<PreviewUpdate>>>,
    wake: rune::Waker,
    mut seq: u64,
) {
    loop {
        if cur.load(Ordering::SeqCst) != gen {
            break;
        }
        let grabbed = grab_frame(&ip, &user, &pass);
        // The camera may have changed while ffmpeg was blocked on the old stream.
        if cur.load(Ordering::SeqCst) != gen {
            break;
        }
        let failed = match grabbed {
            Ok(jpg) => {
                seq += 1;
                if let Ok(mut s) = slot.lock() {
                    *s = Some(PreviewUpdate::Frame { gen, seq, jpeg: jpg });
                }
                false
            }
            Err(msg) => {
                seq += 1;
                if let Ok(mut s) = slot.lock() {
                    *s = Some(PreviewUpdate::Error { gen, seq, msg });
                }
                true
            }
        };
        wake();
        // Sleep in slices so stop/select reacts fast.
        let wait = if failed { 40 } else { 15 };
        for _ in 0..wait {
            if cur.load(Ordering::SeqCst) != gen {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Make sure exactly one fetcher targets the selected camera (native: ffmpeg thread).
/// Restarts when the camera, its credentials, or the on/off switch changes.
/// Call from poll each frame; cheap when nothing changed.
#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_native(app: &mut App) {
    let cam = app.current().clone();
    let want: Option<(String, String, String)> = if app.preview_on {
        Some((cam.ip, cam.username, cam.password))
    } else {
        None
    };
    let cur = if app.preview_ip.is_empty() {
        None
    } else {
        Some((
            app.preview_ip.clone(),
            app.preview_user.clone(),
            app.preview_pass.clone(),
        ))
    };
    if cur == want {
        return;
    }
    // Stop whatever runs and discard a frame that belonged to the old target.
    let gen = app.preview_gen.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut slot) = app.preview_slot.lock() {
        *slot = None;
    }
    match want {
        None => {
            app.preview_ip.clear();
            app.preview_user.clear();
            app.preview_pass.clear();
            app.preview_image = None;
            app.preview_ready = false;
            app.preview_error.clear();
            app.preview_status = "Preview paused".to_string();
        }
        Some((ip, user, pass)) => {
            app.preview_ip = ip.clone();
            app.preview_user = user.clone();
            app.preview_pass = pass.clone();
            app.preview_status = format!("Connecting to {} …", ip);
            if let Some(wake) = app.preview_waker.clone() {
                let cur = app.preview_gen.clone();
                let slot = app.preview_slot.clone();
                let seq = app.preview_frames;
                std::thread::spawn(move || grab_loop(ip, user, pass, gen, cur, slot, wake, seq));
            }
        }
    }
}

// ---------------------------------------------------------------- wasm relay fetch

/// One JPEG from the relay: GET {relay}/api/snapshot?ip=&user=&pass= (same origin).
#[cfg(target_arch = "wasm32")]
async fn fetch_snapshot(
    relay_url: &str,
    ip: &str,
    user: &str,
    pass: &str,
    seq: u64,
) -> Result<Vec<u8>, String> {
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::{AbortController, Request, RequestCache, RequestInit};

    let base = relay_url.trim_end_matches('/');
    let url = format!(
        "{}/api/snapshot?ip={}&user={}&pass={}&seq={}",
        base,
        userinfo_encode(ip),
        userinfo_encode(user),
        userinfo_encode(pass),
        seq
    );
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let controller = AbortController::new().map_err(|_| "could not create snapshot abort controller".to_string())?;
    let signal = controller.signal();
    let opts = RequestInit::new();
    opts.set_cache(RequestCache::NoStore);
    opts.set_signal(Some(&signal));
    let req = Request::new_with_str_and_init(&url, &opts)
        .map_err(|_| "bad snapshot request".to_string())?;

    let timeout = Closure::once(move || controller.abort());
    let timeout_fn: js_sys::Function = timeout.as_ref().clone().into();
    let timer = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(&timeout_fn, 18_000)
        .map_err(|_| "could not start snapshot timeout".to_string())?;
    let fetched = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req)).await;
    window.clear_timeout_with_handle(timer);
    drop(timeout);

    let resp: web_sys::Response = match fetched {
        Ok(value) => value
            .dyn_into()
            .map_err(|_| "relay returned an invalid response".to_string())?,
        Err(_) if signal.aborted() => {
            return Err("relay snapshot timed out — check camera/relay WiFi".to_string());
        }
        Err(_) => return Err("relay fetch failed — is the relay URL right?".to_string()),
    };
    if resp.status() != 200 {
        let text = wasm_bindgen_futures::JsFuture::from(
            resp.text().map_err(|_| "no error body".to_string())?,
        )
        .await
        .map_err(|_| "read error failed".to_string())?;
        let text = String::from(js_sys::JsString::from(text));
        let detail = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
            .unwrap_or(text);
        return Err(format!("relay snapshot: {}", detail.chars().take(180).collect::<String>()));
    }
    let buf = wasm_bindgen_futures::JsFuture::from(
        resp.array_buffer()
            .map_err(|_| "no body".to_string())?,
    )
    .await
    .map_err(|_| "read failed".to_string())?;
    let bytes = js_sys::Uint8Array::new(&buf).to_vec();
    if bytes.is_empty() {
        return Err("relay returned empty image".to_string());
    }
    Ok(bytes)
}

/// Kick off one relay snapshot fetch (wasm); result lands in the slot + wake.
#[cfg(target_arch = "wasm32")]
pub fn kick_relay_fetch(app: &mut App, gen: u64) {
    if app.preview_fetching {
        return;
    }
    let relay = app.relay_url.text.trim().to_string();
    let cam = app.current().clone();
    let (ip, user, pass) = (cam.ip, cam.username, cam.password);
    if relay.is_empty() {
        app.preview_status =
            "Phone preview needs the relay URL below (LAN relay serves + forwards)".to_string();
        return;
    }
    app.preview_fetching = true;
    let slot = app.preview_slot.clone();
    let cur = app.preview_gen.clone();
    let seq = app.preview_frames + 1;
    let wake = app.preview_waker.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let upd = match fetch_snapshot(&relay, &ip, &user, &pass, seq).await {
            Ok(jpeg) => PreviewUpdate::Frame { gen, seq, jpeg },
            Err(msg) => PreviewUpdate::Error { gen, seq, msg },
        };
        if cur.load(Ordering::SeqCst) != gen {
            return;
        }
        if let Ok(mut s) = slot.lock() {
            *s = Some(upd);
        }
        if let Some(w) = wake {
            w();
        }
    });
}

// ---------------------------------------------------------------- shared poll

/// Drain new frames/errors into the image cache; (re)start fetchers as needed.
/// Call from AppSpec::poll every frame.
pub fn poll(app: &mut App, _clock: f64, tx: &mut rune::Tx) -> bool {
    let mut changed = false;
    #[cfg(not(target_arch = "wasm32"))]
    ensure_native(app);
    #[cfg(target_arch = "wasm32")]
    {
        // wasm: fetch through the relay, paced by the clock.
        let cam = app.current().clone();
        let relay_ok = app.use_relay && !app.relay_url.text.trim().is_empty();
        let want: Option<(String, String, String)> = if app.preview_on && relay_ok {
            Some((cam.ip, cam.username, cam.password))
        } else {
            None
        };
        let cur: Option<(String, String, String)> = if app.preview_ip.is_empty() {
            None
        } else {
            Some((
                app.preview_ip.clone(),
                app.preview_user.clone(),
                app.preview_pass.clone(),
            ))
        };
        if want != cur {
            app.preview_gen.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut slot) = app.preview_slot.lock() {
                *slot = None;
            }
            app.preview_user = want.as_ref().map(|w| w.1.clone()).unwrap_or_default();
            app.preview_pass = want.as_ref().map(|w| w.2.clone()).unwrap_or_default();
            app.preview_ip = want.as_ref().map(|w| w.0.clone()).unwrap_or_default();
            app.preview_image = None;
            app.preview_ready = false;
            app.preview_error.clear();
            app.preview_fetching = false;
            app.preview_next_at = _clock;
            app.preview_status = match &want {
                Some((ip, _, _)) => format!("Connecting to {} …", ip),
                None => {
                    if app.preview_on && !relay_ok {
                        "Phone preview needs the relay URL below".to_string()
                    } else {
                        "Preview paused".to_string()
                    }
                }
            };
            changed = true;
        }
        if app.preview_on && !relay_ok {
            let message = "Phone preview needs the relay URL below (LAN relay serves + forwards)";
            if app.preview_status != message {
                app.preview_status = message.to_string();
                changed = true;
            }
        }
        if want.is_some() && !app.preview_fetching && _clock >= app.preview_next_at {
            app.preview_next_at = _clock + 1.5;
            let gen = app.preview_gen.load(Ordering::SeqCst);
            kick_relay_fetch(app, gen);
        }
        let _ = tx;
    }
    // Drain whatever the fetcher delivered.
    let update = app
        .preview_slot
        .lock()
        .map(|mut s| s.take())
        .unwrap_or(None);
    if let Some(u) = update {
        let gen = match &u {
            PreviewUpdate::Frame { gen, .. } | PreviewUpdate::Error { gen, .. } => *gen,
        };
        if gen != app.preview_gen.load(Ordering::SeqCst) {
            return changed;
        }
        match u {
            PreviewUpdate::Frame { seq, jpeg, .. } => {
                app.preview_frames = seq;
                app.preview_error.clear();
                app.preview_ready = false;
                if let Some(cache) = app.preview_cache.as_ref() {
                    app.preview_image = Some(cache.load(jpeg));
                    app.preview_status = format!("decoding frame {} …", seq);
                } else {
                    app.preview_status = "image cache not ready".to_string();
                }
                #[cfg(target_arch = "wasm32")]
                {
                    app.preview_fetching = false;
                }
                changed = true;
            }
            PreviewUpdate::Error { seq, msg, .. } => {
                app.preview_frames = seq;
                app.preview_error = msg.clone();
                app.preview_status = format!("retrying… {}", msg.chars().take(120).collect::<String>());
                #[cfg(target_arch = "wasm32")]
                {
                    // Back off before the next try.
                    app.preview_fetching = false;
                    app.preview_next_at = _clock + 4.0;
                }
                changed = true;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = tx;
        }
    }
    // Surface decode errors (bad bytes, unsupported codec).
    // A newly decoded image also needs one more commit so the still window repaints.
    if let Some(img) = app.preview_image.as_ref() {
        if img.size().is_some() && !app.preview_ready {
            app.preview_ready = true;
            app.preview_status = format!("live · frame {}", app.preview_frames);
            changed = true;
        }
        if img.size().is_none() && app.preview_error.is_empty() {
            let msg = format!("decoding frame {} …", app.preview_frames);
            if app.preview_status != msg {
                app.preview_status = msg;
                changed = true;
            }
        }
        if let Some(e) = img.error() {
            let msg = format!("decode: {}", e.0.chars().take(120).collect::<String>());
            if app.preview_error != msg {
                app.preview_error = msg.clone();
                app.preview_status = msg;
                changed = true;
            }
        }
    }
    changed
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::grab_http_snapshot_at;
    use std::io::{Read, Write};

    #[test]
    fn http_snapshot_is_preferred_and_decoded_as_jpeg() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
            }
            assert!(String::from_utf8_lossy(&request).contains("/action_snapshot"));
            let body = [0xFF, 0xD8, 0xFF, 0xD9];
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });
        assert_eq!(grab_http_snapshot_at("127.0.0.1", port), Some(vec![0xFF, 0xD8, 0xFF, 0xD9]));
    }
}
