//! Live camera preview: a continuous RTSP → MJPEG stream at the camera's rate
//! (capped at 30 fps), never a fresh snapshot request per frame.
//!
//! Rune's `Player` can't play RTSP (AVFoundation/MediaFoundation don't speak
//! it), so the desktop app runs one long-lived ffmpeg that decodes RTSP and
//! writes MJPEG frames to stdout; this module splits them and draws through
//! `ImageCache`. The browser build can't run ffmpeg, so the relay does the
//! same and exposes `GET /api/stream?ip=` as multipart JPEG; the web client
//! decodes those with zune-jpeg and uploads RGBA through `Pixels::set` (no
//! browser image decode, no mip chain). If ffmpeg is missing, the HTTP still
//! endpoint is polled flat out as a fallback.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crate::model::App;

/// One frame, one error, or the end of a relay stream (web restarts it).
#[derive(Debug)]
pub enum PreviewUpdate {
    Frame { gen: u64, seq: u64, jpeg: Vec<u8> },
    Error { gen: u64, seq: u64, msg: String },
}

/// Largest JPEG we will hold while looking for its end marker.
const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Reassembles JPEGs from a byte stream (ffmpeg stdout, or a relay's
/// multipart body). Boundaries and headers between frames are ignored.
pub struct JpegSplitter {
    buf: Vec<u8>,
}

impl JpegSplitter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Feed one chunk; calls `on_frame` for every complete JPEG it completes.
    pub fn push(&mut self, data: &[u8], mut on_frame: impl FnMut(Vec<u8>)) {
        self.buf.extend_from_slice(data);
        loop {
            let Some(start) = soi(&self.buf) else {
                // A marker can straddle the chunk edge; keep the last two bytes.
                if self.buf.len() > 2 {
                    let keep = self.buf.len() - 2;
                    self.buf.drain(..keep);
                }
                break;
            };
            if start > 0 {
                self.buf.drain(..start);
            }
            match eoi(&self.buf) {
                Some(end) => {
                    let frame: Vec<u8> = self.buf.drain(..end).collect();
                    on_frame(frame);
                }
                None => {
                    if self.buf.len() > MAX_FRAME {
                        self.buf.clear();
                    }
                    break;
                }
            }
        }
    }
}

impl Default for JpegSplitter {
    fn default() -> Self {
        Self::new()
    }
}

fn soi(data: &[u8]) -> Option<usize> {
    data.windows(3).position(|w| w == [0xFF, 0xD8, 0xFF])
}

fn eoi(data: &[u8]) -> Option<usize> {
    data.windows(2)
        .skip(2)
        .position(|w| w == [0xFF, 0xD9])
        .map(|p| p + 2 + 2)
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
    // Browsers can't run ffmpeg; the relay does (see its /api/stream).
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

/// RTSP paths to try, sub stream first: 720p/360p is plenty for aiming and
/// keeps the H.264 → MJPEG transcode cheap. FoMaKo firmware exposes the sub
/// stream as `/2` or `/live/av1`, the main as `/1` or `/live/av0`.
#[cfg(not(target_arch = "wasm32"))]
const RTSP_PATHS: [&str; 4] = ["2", "live/av1", "1", "live/av0"];

#[cfg(not(target_arch = "wasm32"))]
const FFMPEG_MISSING: &str = "ffmpeg not found — install it (apt install ffmpeg / brew install ffmpeg / winget install ffmpeg), or drop ffmpeg(.exe) next to the app";

/// Publishes the newest frame into the one slot, replacing an unread frame.
/// A UI that can't keep up drops frames instead of accumulating latency.
#[cfg(not(target_arch = "wasm32"))]
struct Emitter {
    gen: u64,
    cur: Arc<AtomicU64>,
    slot: Arc<Mutex<Option<PreviewUpdate>>>,
    wake: rune::Waker,
    seq: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl Emitter {
    fn stale(&self) -> bool {
        self.cur.load(Ordering::SeqCst) != self.gen
    }

    fn publish(&mut self, update: PreviewUpdate) {
        if let Ok(mut slot) = self.slot.lock() {
            *slot = Some(update);
        }
        (self.wake)();
    }

    fn frame(&mut self, jpeg: Vec<u8>) {
        self.seq += 1;
        self.publish(PreviewUpdate::Frame {
            gen: self.gen,
            seq: self.seq,
            jpeg,
        });
    }

    fn error(&mut self, msg: String) {
        self.seq += 1;
        self.publish(PreviewUpdate::Error {
            gen: self.gen,
            seq: self.seq,
            msg,
        });
    }
}

/// Keep only the tail of a child's stderr: enough for the last error line.
#[cfg(not(target_arch = "wasm32"))]
fn read_tail(mut reader: impl std::io::Read) -> String {
    let mut tail = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok(n) = reader.read(&mut buf) {
        if n == 0 {
            break;
        }
        tail.extend_from_slice(&buf[..n]);
        if tail.len() > 8192 {
            let cut = tail.len() - 8192;
            tail.drain(..cut);
        }
    }
    String::from_utf8_lossy(&tail).into_owned()
}

#[cfg(not(target_arch = "wasm32"))]
fn ffmpeg_error(stderr: &str) -> String {
    let line = stderr
        .trim()
        .lines()
        .last()
        .unwrap_or("ffmpeg could not open the camera stream");
    let mut msg: String = line.chars().take(220).collect();
    if stderr.contains("401") || stderr.to_ascii_lowercase().contains("unauthorized") {
        msg.push_str(" — check camera login (FoMaKo default admin/admin)");
    }
    msg
}

/// One long-lived ffmpeg decode of `url`, publishing every JPEG it emits.
/// Returns whether it saw any frame; an immediate failure is an error so the
/// caller can try the next RTSP path or fall back to HTTP stills.
#[cfg(not(target_arch = "wasm32"))]
fn run_stream(em: &mut Emitter, url: &str) -> Result<bool, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new(ffmpeg_bin())
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-rtsp_transport",
            "tcp",
            "-timeout",
            "5000000",
            "-i",
            url,
            "-an",
            "-sn",
            "-dn",
            // Cap at 30 fps; the sub stream is ~720p, and 576 wide is plenty for
            // the preview box while keeping JPEG decode fast enough for 30 fps.
            "-vf",
            "fps=30,scale=576:-2",
            "-f",
            "mjpeg",
            "-q:v",
            "5",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FFMPEG_MISSING.to_string()
            } else {
                format!("ffmpeg: {}", e)
            }
        })?;
    let stdout = child.stdout.take().ok_or("ffmpeg gave no stdout")?;
    let stderr = child.stderr.take().ok_or("ffmpeg gave no stderr")?;
    let stderr_thread = std::thread::spawn(move || read_tail(stderr));

    let mut reader = std::io::BufReader::with_capacity(64 * 1024, stdout);
    let mut splitter = JpegSplitter::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let mut saw = false;
    loop {
        if em.stale() {
            break;
        }
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                splitter.push(&chunk[..n], |jpeg| {
                    saw = true;
                    em.frame(jpeg);
                });
            }
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let stderr = stderr_thread.join().unwrap_or_default();
    if !saw && !em.stale() {
        return Err(ffmpeg_error(&stderr));
    }
    Ok(saw)
}

/// Sleep in slices so a camera switch or Pause is noticed immediately.
#[cfg(not(target_arch = "wasm32"))]
fn sleep_sliced(em: &Emitter, ms: u64) {
    let until = std::time::Instant::now() + Duration::from_millis(ms);
    while std::time::Instant::now() < until {
        if em.stale() {
            return;
        }
        let left = until.saturating_duration_since(std::time::Instant::now());
        std::thread::sleep(left.min(Duration::from_millis(25)));
    }
}

/// Fallback when ffmpeg is absent or RTSP fails: poll the HTTP still endpoint
/// flat out. Slower than a real stream, but far faster than the old 1.5 s tick.
#[cfg(not(target_arch = "wasm32"))]
fn snapshot_loop(em: &mut Emitter, ip: &str) -> bool {
    let mut failures = 0u32;
    let mut saw = false;
    loop {
        if em.stale() {
            return saw;
        }
        match grab_http_snapshot_at(ip, 80) {
            Some(jpeg) => {
                failures = 0;
                saw = true;
                em.frame(jpeg);
            }
            None => {
                failures += 1;
                if failures >= 3 {
                    em.error("HTTP snapshot failed — check camera IP/login, or install ffmpeg for RTSP".to_string());
                    return saw;
                }
                sleep_sliced(em, 800);
            }
        }
    }
}

/// Background loop: one stream, then restart/fall back until `gen` goes stale.
#[cfg(not(target_arch = "wasm32"))]
pub fn grab_loop(
    ip: String,
    user: String,
    pass: String,
    gen: u64,
    cur: Arc<AtomicU64>,
    slot: Arc<Mutex<Option<PreviewUpdate>>>,
    wake: rune::Waker,
    seq: u64,
) {
    let mut em = Emitter {
        gen,
        cur,
        slot,
        wake,
        seq,
    };
    loop {
        if em.stale() {
            break;
        }
        let mut live = false;
        if !ffmpeg_path_or_empty().is_empty() {
            for path in RTSP_PATHS {
                if em.stale() {
                    return;
                }
                match run_stream(&mut em, &rtsp_url_for_path(&ip, rtsp_port(), &user, &pass, path))
                {
                    Ok(true) => {
                        live = true;
                        break;
                    }
                    Ok(false) => {}
                    Err(msg) => {
                        em.error(msg);
                        break;
                    }
                }
            }
        }
        if em.stale() {
            break;
        }
        if !live {
            live = snapshot_loop(&mut em, &ip);
        }
        if em.stale() {
            break;
        }
        sleep_sliced(&em, if live { 250 } else { 3000 });
    }
}

/// Make sure exactly one fetcher targets the selected camera (native: ffmpeg thread).
/// Restarts when the camera, its credentials, or the on/off switch changes.
/// Call from poll each frame; cheap when nothing changed.
#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_native(app: &mut App, clock: f64) {
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
    app.preview_fps = 0.0;
    app.preview_fps_frames = 0;
    app.preview_fps_since = clock;
    app.preview_error.clear();
    app.preview_image = None;
    app.preview_pending = None;
    match want {
        None => {
            app.preview_ip.clear();
            app.preview_user.clear();
            app.preview_pass.clear();
            app.preview_ready = false;
            app.preview_status = "Preview paused".to_string();
        }
        Some((ip, user, pass)) => {
            app.preview_ip = ip.clone();
            app.preview_user = user.clone();
            app.preview_pass = pass.clone();
            app.preview_ready = false;
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

// ---------------------------------------------------------------- wasm MJPEG overlay

/// Web preview: a browser-native JPEG canvas over the Rune view. The browser
/// decodes and composites frames off the UI thread, so video stays at the
/// relay's ~30 fps even when Rune's software canvas repaints slowly.
#[cfg(target_arch = "wasm32")]
const OVERLAY_JS: &str = r#"
(function () {
  if (window.__fomakoPreviewSet) return;
  const cv = document.createElement('canvas');
  cv.id = 'fomako-preview';
  cv.style.cssText = 'position:fixed;display:none;pointer-events:none;z-index:2147483646;background:#000;border-radius:8px';
  document.body.appendChild(cv);
  const ctx = cv.getContext('2d');
  const wake = () => { if (window.__fomakoWake) window.__fomakoWake(); };
  let aborter = null, drawn = 0, started = performance.now();
  window.__fomakoPreviewFps = 0;
  window.__fomakoPreviewError = '';
  window.__fomakoPreviewPlace = function (x, y, w, h) {
    cv.style.left = x + 'px';
    cv.style.top = y + 'px';
    cv.style.width = w + 'px';
    cv.style.height = h + 'px';
  };
  async function run(url) {
    if (aborter) aborter.abort();
    aborter = new AbortController();
    const signal = aborter.signal;
    window.__fomakoPreviewFps = 0;
    window.__fomakoPreviewError = '';
    cv.style.display = 'block';
    drawn = 0;
    started = performance.now();
    let buf = new Uint8Array(0), latest = null, decoding = false;
    const cat = (a, b) => { const c = new Uint8Array(a.length + b.length); c.set(a); c.set(b, a.length); return c; };
    async function drain() {
      if (decoding) return;
      decoding = true;
      while (latest) {
        const jpeg = latest;
        latest = null;
        try {
          const bmp = await createImageBitmap(new Blob([jpeg], {type: 'image/jpeg'}));
          if (cv.width !== bmp.width || cv.height !== bmp.height) {
            cv.width = bmp.width;
            cv.height = bmp.height;
          }
          ctx.drawImage(bmp, 0, 0);
          bmp.close();
          drawn++;
          const now = performance.now();
          if (now - started >= 1000) {
            window.__fomakoPreviewFps = drawn * 1000 / (now - started);
            drawn = 0;
            started = now;
            wake();
          }
        } catch (e) {}
      }
      decoding = false;
    }
    try {
      const resp = await fetch(url, {signal: signal, cache: 'no-store'});
      if (!resp.ok) { window.__fomakoPreviewError = 'relay HTTP ' + resp.status; wake(); return; }
      const reader = resp.body.getReader();
      for (;;) {
        const r = await reader.read();
        if (r.done) break;
        buf = cat(buf, r.value);
        for (;;) {
          let s = -1;
          for (let i = 0; i + 2 < buf.length; i++) {
            if (buf[i] === 255 && buf[i + 1] === 216 && buf[i + 2] === 255) { s = i; break; }
          }
          if (s < 0) { buf = buf.slice(Math.max(0, buf.length - 2)); break; }
          if (s > 0) buf = buf.slice(s);
          let e = -1;
          for (let i = 2; i + 1 < buf.length; i++) {
            if (buf[i] === 255 && buf[i + 1] === 217) { e = i + 2; break; }
          }
          if (e < 0) break;
          latest = buf.slice(0, e);
          buf = buf.slice(e);
          drain();
        }
      }
      if (!signal.aborted) { window.__fomakoPreviewError = 'stream ended'; wake(); }
    } catch (e) {
      if (!signal.aborted) { window.__fomakoPreviewError = String((e && e.message) || e); wake(); }
    }
  }
  window.__fomakoPreviewSet = function (url) {
    if (!url) {
      if (aborter) aborter.abort();
      aborter = null;
      cv.style.display = 'none';
      window.__fomakoPreviewFps = 0;
      window.__fomakoPreviewError = '';
      return;
    }
    run(url);
  };
})();
"#;

#[cfg(target_arch = "wasm32")]
fn overlay_ready() {
    use std::cell::Cell;
    thread_local! {
        static READY: Cell<bool> = const { Cell::new(false) };
    }
    READY.with(|ready| {
        if !ready.get() {
            ensure_overlay_fn("__fomakoPreviewSet").or_else(|| {
                let _ = js_sys::eval(OVERLAY_JS);
                ensure_overlay_fn("__fomakoPreviewSet")
            });
            ready.set(true);
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn ensure_overlay_fn(name: &str) -> Option<()> {
    let window = web_sys::window()?;
    js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str(name))
        .ok()
        .filter(|value| value.is_function())
        .map(|_| ())
}

#[cfg(target_arch = "wasm32")]
fn overlay_call(name: &str, args: &[wasm_bindgen::JsValue]) {
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else { return };
    let Ok(value) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str(name)) else {
        return;
    };
    if let Ok(function) = value.dyn_into::<js_sys::Function>() {
        let list = js_sys::Array::new();
        for arg in args {
            list.push(arg);
        }
        let _ = function.apply(&wasm_bindgen::JsValue::NULL, &list);
    }
}

#[cfg(target_arch = "wasm32")]
fn overlay_number(name: &str) -> f64 {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str(name)).ok()
        })
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0)
}

#[cfg(target_arch = "wasm32")]
fn overlay_string(name: &str) -> String {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str(name)).ok()
        })
        .and_then(|value| value.as_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------- shared poll

/// Native: drain frames into the image cache. Web: drive the JPEG canvas overlay.
/// Call from AppSpec::poll every frame.
pub fn poll(app: &mut App, clock: f64, _tx: &mut rune::Tx) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = clock;
        return poll_overlay(app);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        poll_native(app, clock)
    }
}

#[cfg(target_arch = "wasm32")]
fn poll_overlay(app: &mut App) -> bool {
    let mut changed = false;
    let relay = app.relay_url.text.trim().trim_end_matches('/').to_string();
    let relay_ok = app.use_relay && !relay.is_empty();
    let url = if app.preview_on && relay_ok && app.page == crate::model::Page::Control {
        let cam = app.current();
        format!(
            "{}/api/stream?ip={}&user={}&pass={}",
            relay,
            userinfo_encode(&cam.ip),
            userinfo_encode(&cam.username),
            userinfo_encode(&cam.password)
        )
    } else {
        String::new()
    };
    overlay_ready();
    // Bridge the overlay's once-a-second fps/error updates into a Rune wake so
    // the status line follows the stream without animating the whole UI.
    if ensure_overlay_fn("__fomakoWake").is_none() {
        if let Some(waker) = app.preview_waker.clone() {
            let callback = wasm_bindgen::prelude::Closure::<dyn Fn()>::new(move || waker());
            if let Some(window) = web_sys::window() {
                let _ = js_sys::Reflect::set(
                    &window,
                    &wasm_bindgen::JsValue::from_str("__fomakoWake"),
                    callback.as_ref(),
                );
            }
            callback.forget();
        }
    }
    if url != app.preview_overlay_url {
        overlay_call(
            "__fomakoPreviewSet",
            &[wasm_bindgen::JsValue::from_str(&url)],
        );
        app.preview_overlay_url = url;
        app.preview_fps = 0.0;
        app.preview_ready = !app.preview_overlay_url.is_empty();
        app.preview_status = if app.preview_overlay_url.is_empty() {
            if app.preview_on && !relay_ok {
                "Phone preview needs the relay URL below".to_string()
            } else {
                "Preview paused".to_string()
            }
        } else {
            "Connecting…".to_string()
        };
        changed = true;
    }
    if let Some([x, y, w, h]) = app.preview_rect {
        overlay_call(
            "__fomakoPreviewPlace",
            &[
                x.into(),
                y.into(),
                w.into(),
                h.into(),
            ],
        );
    }
    let fps = overlay_number("__fomakoPreviewFps");
    if fps >= 0.5 && (fps as f32 - app.preview_fps).abs() > 0.01 {
        app.preview_fps = fps as f32;
        // Wake Rune once a second so the status line follows the overlay.
        changed = true;
    }
    let error = overlay_string("__fomakoPreviewError");
    let status = if !error.is_empty() {
        format!("retrying… {}", error.chars().take(120).collect::<String>())
    } else if fps >= 0.5 {
        format!("live · {:.0} fps", fps)
    } else if !app.preview_overlay_url.is_empty() {
        "live".to_string()
    } else {
        app.preview_status.clone()
    };
    if app.preview_status != status {
        app.preview_status = status;
        changed = true;
    }
    changed
}

#[cfg(not(target_arch = "wasm32"))]
fn poll_native(app: &mut App, clock: f64) -> bool {
    let mut changed = false;
    ensure_native(app, clock);
    // Promote a decoded frame only when it is ready, so the window always
    // draws a complete picture (never a pending/blank one).
    if let Some(img) = app.preview_pending.as_ref() {
        if img.size().is_some() {
            app.preview_image = app.preview_pending.take();
            app.preview_ready = true;
            app.preview_fps_frames += 1;
            changed = true;
        } else if let Some(e) = img.error() {
            let msg = format!("decode: {}", e.0.chars().take(120).collect::<String>());
            app.preview_error = msg.clone();
            app.preview_status = msg;
            app.preview_pending = None;
            changed = true;
        }
    }
    // One decode in flight at a time; the slot keeps only the newest frame.
    let pending = if app.preview_pending.is_none() {
        app.preview_slot
            .lock()
            .map(|mut slot| slot.take())
            .unwrap_or(None)
    } else {
        None
    };
    if let Some(update) = pending {
        let gen = match &update {
            PreviewUpdate::Frame { gen, .. } | PreviewUpdate::Error { gen, .. } => *gen,
        };
        if gen == app.preview_gen.load(Ordering::SeqCst) {
            match update {
                PreviewUpdate::Frame { seq, jpeg, .. } => {
                    let first = app.preview_image.is_none() && app.preview_pending.is_none();
                    app.preview_frames = seq;
                    app.preview_error.clear();
                    if let Some(cache) = app.preview_cache.as_ref() {
                        app.preview_pending = Some(cache.load(jpeg));
                        if first {
                            app.preview_status = format!("decoding frame {} …", seq);
                        }
                    } else {
                        app.preview_status = "image cache not ready".to_string();
                    }
                    changed = true;
                }
                PreviewUpdate::Error { seq, msg, .. } => {
                    app.preview_frames = seq;
                    app.preview_error = msg.clone();
                    app.preview_status =
                        format!("retrying… {}", msg.chars().take(120).collect::<String>());
                    changed = true;
                }
            }
        }
    }
    // Frames decoded per second, shown in the status line so 30 fps is visible.
    if app.preview_fps_since == 0.0 {
        app.preview_fps_since = clock;
    }
    let span = clock - app.preview_fps_since;
    if span >= 1.0 {
        app.preview_fps = app.preview_fps_frames as f32 / span as f32;
        app.preview_fps_frames = 0;
        app.preview_fps_since = clock;
        changed = true;
    }
    if app.preview_ready {
        let status = if app.preview_fps >= 0.5 {
            format!("live · {:.0} fps", app.preview_fps)
        } else {
            "live".to_string()
        };
        if app.preview_status != status {
            app.preview_status = status;
            changed = true;
        }
    }
    changed
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{JpegSplitter, grab_http_snapshot_at};
    use std::io::{Read, Write};

    fn split(chunks: &[&[u8]]) -> Vec<Vec<u8>> {
        let mut splitter = JpegSplitter::new();
        let mut frames = Vec::new();
        for chunk in chunks {
            splitter.push(chunk, |jpeg| frames.push(jpeg));
        }
        frames
    }

    #[test]
    fn splitter_finds_frames_across_chunk_edges() {
        // Two tiny JPEGs, one split mid-marker, with multipart headers between.
        let a = [0xFF, 0xD8, 0xFF, 0xE0, 0x01, 0x02, 0xFF, 0xD9];
        let b = [0xFF, 0xD8, 0xFF, 0xDB, 0x03, 0xFF, 0xD9];
        let mut stream = Vec::new();
        stream.extend_from_slice(b"--frame\r\nContent-Type: image/jpeg\r\n\r\n");
        stream.extend_from_slice(&a);
        stream.extend_from_slice(b"\r\n--frame\r\n\r\n");
        stream.extend_from_slice(&b);
        stream.extend_from_slice(b"\r\n--frame--\r\n");
        for cut in 1..stream.len() {
            let frames = split(&[&stream[..cut], &stream[cut..]]);
            assert_eq!(frames, vec![a.to_vec(), b.to_vec()], "cut at {cut}");
        }
    }

    #[test]
    fn splitter_keeps_only_the_latest_complete_frames() {
        let frame = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0xFF, 0xD9];
        let mut bytes = Vec::new();
        for _ in 0..3 {
            bytes.extend_from_slice(&frame);
        }
        let frames = split(&[&bytes]);
        assert_eq!(frames.len(), 3);
        assert!(frames.iter().all(|f| f == &frame));
    }

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
