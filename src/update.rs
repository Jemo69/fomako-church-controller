//! Auto-updater: ask GitHub for the latest release, download this platform's
//! binary, check it against the release's `SHA256SUMS`, swap it beside the
//! running app, then wait for the user to restart.
//!
//! Checking runs by itself a few seconds after launch and every six hours, but
//! nothing is ever installed without a click and the app never restarts on its
//! own — a booth PC must not drop mid-service.
//!
//! HTTP uses no new crates: `curl`, else `wget`, else a clear error. Release
//! assets are plain binaries, so installing is a rename beside the running
//! executable: no installer, no elevation, no background service.
//!
//! `FOMAKO_UPDATE_API` points the check at another URL (mirror, test server).
//! `FOMAKO_NO_UPDATE_CHECK=1` turns the automatic check off.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::model::App;

const REPO: &str = "Jemo69/fomako-church-controller";
/// Public release page, for "show me the notes" and manual installs.
pub const RELEASES_URL: &str = "https://github.com/Jemo69/fomako-church-controller/releases/latest";
const CHECK_EVERY: f64 = 6.0 * 60.0 * 60.0;
const RETRY_AFTER: f64 = 60.0 * 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UpdatePhase {
    /// Nothing running; `update_msg` says what happened last.
    #[default]
    Idle,
    Checking,
    /// A newer release exists; the download button is live.
    Available,
    Downloading,
    /// New binary is in place; restart picks it up.
    Ready,
    Failed,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Debug)]
pub enum UpdateEvent {
    UpToDate,
    Available {
        version: String,
        url: String,
        sums_url: String,
        page_url: String,
        asset: String,
    },
    Failed(String),
    Progress { done: u64 },
    Installed { path: String },
}

/// The release fields the updater needs, parsed out of the GitHub API JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub asset: String,
    pub url: String,
    pub sums_url: String,
    pub page_url: String,
}

/// This build's version, straight from Cargo.toml.
pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The release file that matches this OS/architecture, if we ship one.
pub fn asset_name() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("fomako-church-controller-linux-x64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("fomako-church-controller-windows-x64.exe")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("fomako-church-controller-mac-arm64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("fomako-church-controller-mac-x64")
    } else {
        None
    }
}

fn api_url() -> String {
    std::env::var("FOMAKO_UPDATE_API")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| format!("https://api.github.com/repos/{REPO}/releases/latest"))
}

fn auto_check_disabled() -> bool {
    std::env::var_os("FOMAKO_NO_UPDATE_CHECK").is_some()
}

// ---------------------------------------------------------------- versions

fn version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split('-') // drop any pre-release suffix
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Is `remote` a newer release than `local`? Plain dotted numbers, so 0.10 > 0.9.
pub fn is_newer(remote: &str, local: &str) -> bool {
    let (a, b) = (version_parts(remote), version_parts(local));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// Parse the `/releases/latest` body. `want` is the asset for this machine;
/// pass "" to only care about version and links (web build).
pub fn parse_release(json: &str, want: &str) -> Result<Release, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("GitHub sent something odd: {}", e))?;
    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
        return Err(format!("GitHub says: {}", msg));
    }
    let tag = v
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("release has no tag")?;
    let page_url = v
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or(RELEASES_URL)
        .to_string();
    let mut asset = String::new();
    let mut url = String::new();
    let mut sums_url = String::new();
    for a in v.get("assets").and_then(|a| a.as_array()).into_iter().flatten() {
        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let link = a
            .get("browser_download_url")
            .and_then(|u| u.as_str())
            .unwrap_or("");
        if name == "SHA256SUMS" {
            sums_url = link.to_string();
        } else if !want.is_empty() && name == want {
            asset = name.to_string();
            url = link.to_string();
        }
    }
    if !want.is_empty() && url.is_empty() {
        return Err(format!(
            "the release has no {} build — open the release page to install by hand",
            want
        ));
    }
    Ok(Release {
        version: tag.trim_start_matches('v').to_string(),
        asset,
        url,
        sums_url,
        page_url,
    })
}

// ---------------------------------------------------------------- actions

/// Ask GitHub what the latest release is. Non-blocking on desktop.
pub fn start_check(app: &mut App) {
    if matches!(app.update_phase, UpdatePhase::Checking | UpdatePhase::Downloading) {
        return;
    }
    app.update_phase = UpdatePhase::Checking;
    app.update_msg = "Checking GitHub for a newer release…".to_string();
    app.update_next_check = 0.0;
    check_worker(app.update_slot.clone(), app.update_waker.clone());
}

/// Download, verify and swap in the newer binary. Desktop only: browsers and
/// the APK update by replacing what's served/installed.
#[cfg(not(target_arch = "wasm32"))]
pub fn start_install(app: &mut App) {
    if app.update_phase != UpdatePhase::Available {
        return;
    }
    if app.update_sums_url.is_empty() {
        app.update_phase = UpdatePhase::Failed;
        app.update_msg =
            "That release has no SHA256SUMS, so it can't be verified — install it by hand from the release page."
                .to_string();
        return;
    }
    app.update_phase = UpdatePhase::Downloading;
    app.update_done = 0;
    app.update_msg = format!("Downloading v{} …", app.update_version);
    install_worker(
        app.update_url.clone(),
        app.update_sums_url.clone(),
        app.update_asset.clone(),
        app.update_slot.clone(),
        app.update_waker.clone(),
    );
}

/// Launch the freshly installed binary and quit this one.
#[cfg(not(target_arch = "wasm32"))]
pub fn restart(app: &mut App) {
    let Ok(exe) = std::env::current_exe() else {
        app.update_msg = "Could not find this app's path — close and reopen it.".to_string();
        return;
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The new process inherits the console/window; this one leaves the stage.
    match std::process::Command::new(&exe).args(&args).spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            app.update_msg = format!(
                "Could not restart automatically ({}). Close this window and open the app again.",
                e
            );
        }
    }
}

/// Web has nothing to swap: the relay serves the files. Reload to pick up a
/// redeployed build.
#[cfg(target_arch = "wasm32")]
pub fn restart(_app: &mut App) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

// ---------------------------------------------------------------- worker: check

fn check_worker(slot: Arc<Mutex<Vec<UpdateEvent>>>, wake: Option<rune::Waker>) {
    let done = move |ev: UpdateEvent| deliver(&slot, &wake, ev);
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let ev = match fetch_text(&api_url()).await.and_then(|body| {
                let want = asset_name().unwrap_or("");
                parse_release(&body, want).map(|rel| (rel, body))
            }) {
                Ok((rel, _)) => {
                    if is_newer(&rel.version, current()) {
                        UpdateEvent::Available {
                            version: rel.version,
                            url: rel.url,
                            sums_url: rel.sums_url,
                            page_url: rel.page_url,
                            asset: rel.asset,
                        }
                    } else {
                        UpdateEvent::UpToDate
                    }
                }
                Err(e) => UpdateEvent::Failed(e),
            };
            done(ev);
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            let ev = match fetch_text(&api_url()).and_then(|body| {
                let want = asset_name().unwrap_or("");
                parse_release(&body, want)
            }) {
                Ok(rel) => {
                    if is_newer(&rel.version, current()) {
                        UpdateEvent::Available {
                            version: rel.version,
                            url: rel.url,
                            sums_url: rel.sums_url,
                            page_url: rel.page_url,
                            asset: rel.asset,
                        }
                    } else {
                        UpdateEvent::UpToDate
                    }
                }
                Err(e) => UpdateEvent::Failed(e),
            };
            done(ev);
        });
    }
}

// ---------------------------------------------------------------- worker: install

#[cfg(not(target_arch = "wasm32"))]
fn install_worker(
    url: String,
    sums_url: String,
    asset: String,
    slot: Arc<Mutex<Vec<UpdateEvent>>>,
    wake: Option<rune::Waker>,
) {
    std::thread::spawn(move || {
        let note = |ev: UpdateEvent| deliver(&slot, &wake, ev);
        let result = download_and_install(&url, &sums_url, &asset, |done| {
            note(UpdateEvent::Progress { done })
        });
        note(match result {
            Ok(path) => UpdateEvent::Installed { path },
            Err(e) => UpdateEvent::Failed(e),
        });
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn download_and_install(
    url: &str,
    sums_url: &str,
    asset: &str,
    mut progress: impl FnMut(u64),
) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot find this app's path: {e}"))?;
    let dir = exe
        .parent()
        .ok_or("Cannot find this app's folder")?
        .to_path_buf();
    let staging = dir.join(".fomako-update");
    std::fs::create_dir_all(&staging).map_err(|e| {
        format!(
            "Cannot write next to the app ({}): {} — move the app somewhere writable or install by hand.",
            staging.display(),
            e
        )
    })?;
    let tmp = download_verified(url, sums_url, asset, &staging, &mut progress)?;
    install_binary(&tmp, &exe).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e
    })?;
    Ok(exe.display().to_string())
}

/// Download into `staging`, check the hash against `SHA256SUMS`, and leave the
/// verified file on disk for the caller to swap in (or discard).
#[cfg(not(target_arch = "wasm32"))]
fn download_verified(
    url: &str,
    sums_url: &str,
    asset: &str,
    staging: &std::path::Path,
    mut progress: impl FnMut(u64),
) -> Result<std::path::PathBuf, String> {
    use std::io::Write;

    if sums_url.trim().is_empty() {
        return Err(
            "This release has no SHA256SUMS, so the download can't be verified — install it by hand from the release page."
                .to_string(),
        );
    }

    let tmp = staging.join(asset);
    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| format!("Cannot create {}: {}", tmp.display(), e))?;

    let mut hasher = Sha256::new();
    let mut done: u64 = 0;
    let mut last_note = std::time::Instant::now();
    fetch_stream(url, |chunk| {
        file.write_all(chunk)
            .map_err(|e| format!("Cannot write the download: {e}"))?;
        hasher.update(chunk);
        done += chunk.len() as u64;
        if last_note.elapsed().as_millis() >= 250 {
            last_note = std::time::Instant::now();
            progress(done);
        }
        Ok(())
    })?;
    drop(file);
    progress(done);

    if done < 1_000_000 {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "The download was only {done} bytes — that's not the app. Try again later."
        ));
    }

    let sums = fetch_text(sums_url).map_err(|e| {
        format!(
            "Downloaded v{} but could not fetch SHA256SUMS ({}), so nothing was installed.",
            current(),
            e
        )
    })?;
    let want = sums
        .lines()
        .find_map(|line| {
            let mut it = line.split_whitespace();
            let hash = it.next()?;
            let name = it.next()?.trim_start_matches('*');
            (name == asset).then(|| hash.to_ascii_lowercase())
        })
        .ok_or_else(|| format!("SHA256SUMS has no entry for {asset} — nothing installed."))?;
    let got = hasher.hex();
    if got != want {
        let _ = std::fs::remove_file(&tmp);
        return Err(
            "Checksum mismatch: the download was corrupted or tampered with. Nothing was installed."
                .to_string(),
        );
    }
    Ok(tmp)
}

/// Swap the verified binary into place. On Unix the running image keeps its
/// inode, so rename() over it is safe. Windows locks the running image against
/// writes but allows a rename, so the old one steps aside first.
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
fn install_binary(tmp: &std::path::Path, exe: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(tmp, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("Cannot make the new build executable: {e}"))?;
    std::fs::rename(tmp, exe).map_err(|e| {
        format!(
            "Cannot replace {} ({}). If the app lives in /usr/bin or Program Files, install the update by hand from the release page.",
            exe.display(),
            e
        )
    })
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn install_binary(tmp: &std::path::Path, exe: &std::path::Path) -> Result<(), String> {
    let old = exe.with_extension("old.exe");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).map_err(|e| {
        format!(
            "Cannot move the running app aside ({}). Install the update by hand from the release page.",
            e
        )
    })?;
    if let Err(e) = std::fs::rename(tmp, exe) {
        // Put the working app back before reporting failure.
        let _ = std::fs::rename(&old, exe);
        return Err(format!(
            "Cannot put the new build in place ({}). Install the update by hand from the release page.",
            e
        ));
    }
    // The old image is still mapped; Windows frees it when this process exits.
    Ok(())
}

/// Remove leftovers from a previous update (staging folder, `.old.exe`).
pub fn cleanup_stale() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(exe) = std::env::current_exe() {
        #[cfg(target_os = "windows")]
        let _ = std::fs::remove_file(exe.with_extension("old.exe"));
        if let Some(dir) = exe.parent() {
            let _ = std::fs::remove_dir_all(dir.join(".fomako-update"));
        }
    }
}

// ---------------------------------------------------------------- events → state

/// Drain worker events and run the automatic check on schedule.
/// Call from `AppSpec::poll`.
pub fn poll(app: &mut App, clock: f64, tx: &mut rune::Tx) -> bool {
    let events: Vec<UpdateEvent> = app
        .update_slot
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default();
    let mut changed = false;
    for ev in events {
        changed = true;
        apply(app, ev, clock);
    }

    if app.update_next_check > 0.0
        && clock >= app.update_next_check
        && !auto_check_disabled()
        && !matches!(
            app.update_phase,
            UpdatePhase::Checking | UpdatePhase::Downloading
        )
    {
        start_check(app);
    }

    if matches!(
        app.update_phase,
        UpdatePhase::Checking | UpdatePhase::Downloading
    ) {
        tx.wake_after(std::time::Duration::from_millis(250));
    }
    changed
}

fn apply(app: &mut App, ev: UpdateEvent, clock: f64) {
    match ev {
        UpdateEvent::UpToDate => {
            app.update_phase = UpdatePhase::Idle;
            app.update_version.clear();
            app.update_msg = format!("v{} is the latest release.", current());
            app.update_next_check = clock + CHECK_EVERY;
        }
        UpdateEvent::Available {
            version,
            url,
            sums_url,
            page_url,
            asset,
        } => {
            app.update_version = version.clone();
            app.update_url = url;
            app.update_sums_url = sums_url;
            app.update_release_url = page_url;
            app.update_asset = asset;
            app.update_phase = UpdatePhase::Available;
            #[cfg(target_arch = "wasm32")]
            {
                app.update_msg = format!(
                    "v{} is out (browsers here run v{}). The web build updates when the relay serves the new files — reload after that.",
                    version,
                    current()
                );
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                app.update_msg = format!(
                    "v{} is available — you have v{}. Your cameras and settings stay as they are.",
                    version,
                    current()
                );
            }
            app.update_next_check = clock + CHECK_EVERY;
            app.push_log(format!("Update available: v{}", version));
        }
        UpdateEvent::Failed(e) => {
            app.update_phase = UpdatePhase::Failed;
            app.update_msg = e.clone();
            app.update_next_check = clock + RETRY_AFTER;
            app.push_log(format!("Update: {}", e));
        }
        UpdateEvent::Progress { done } => {
            app.update_phase = UpdatePhase::Downloading;
            app.update_done = done;
            app.update_msg = format!(
                "Downloading v{} … {:.1} MB",
                app.update_version,
                done as f64 / 1_000_000.0
            );
        }
        UpdateEvent::Installed { path } => {
            app.update_phase = UpdatePhase::Ready;
            app.update_ready_path = path;
            app.update_msg = format!(
                "v{} is installed. Restart when it suits you — nothing changes until then.",
                app.update_version
            );
            app.update_next_check = 0.0;
            app.push_log(format!("Update installed: v{}", app.update_version));
        }
    }
}

fn deliver(slot: &Arc<Mutex<Vec<UpdateEvent>>>, wake: &Option<rune::Waker>, ev: UpdateEvent) {
    if let Ok(mut q) = slot.lock() {
        // Keep progress fresh but never lose a terminal event behind it.
        q.push(ev);
    }
    if let Some(w) = wake {
        w();
    }
}

// ---------------------------------------------------------------- HTTP (no new crates)

#[cfg(not(target_arch = "wasm32"))]
fn which(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            #[cfg(target_os = "windows")]
            let cand = dir.join(format!("{name}.exe"));
            #[cfg(not(target_os = "windows"))]
            let cand = dir.join(name);
            cand.is_file()
        })
    })
}

/// Stream a URL through curl or wget, handing chunks to `sink`.
#[cfg(not(target_arch = "wasm32"))]
fn fetch_stream(url: &str, mut sink: impl FnMut(&[u8]) -> Result<(), String>) -> Result<(), String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut cmd = if which("curl") {
        let mut c = Command::new("curl");
        c.args([
            "-fsSL",
            "--max-time",
            "300",
            "-A",
            &format!("fomako-updater/{}", current()),
            "--",
            url,
        ]);
        c
    } else if which("wget") {
        let mut c = Command::new("wget");
        c.args(["-q", "-O", "-", "--", url]);
        c
    } else {
        return Err(
            "No curl or wget found to download with — install curl and try again.".to_string(),
        );
    };
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start the downloader: {e}"))?;
    let mut out = child.stdout.take().ok_or("No download output")?;
    let mut err = child.stderr.take();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = out.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        sink(&buf[..n])?;
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        let mut detail = String::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_string(&mut detail);
        }
        let brief: String = detail.trim().chars().take(200).collect();
        return Err(if brief.is_empty() {
            "Download failed (no network?).".to_string()
        } else {
            format!("Download failed: {}", brief)
        });
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_text(url: &str) -> Result<String, String> {
    let mut body = Vec::new();
    fetch_stream(url, |chunk| {
        body.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(target_arch = "wasm32")]
async fn fetch_text(url: &str) -> Result<String, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestCache, RequestInit};

    let window = web_sys::window().ok_or("no window")?;
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_cache(RequestCache::NoStore);
    let req = Request::new_with_str_and_init(url, &opts).map_err(|_| "bad update request")?;
    let _ = req.headers().set("Accept", "application/vnd.github+json");
    let resp: web_sys::Response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req))
        .await
        .map_err(|_| "network error — is this machine online?".to_string())?
        .dyn_into()
        .map_err(|_| "bad update response")?;
    if !resp.ok() {
        return Err(format!("GitHub returned HTTP {}", resp.status()));
    }
    let text = wasm_bindgen_futures::JsFuture::from(
        resp.text().map_err(|_| "no update body")?,
    )
    .await
    .map_err(|_| "could not read the update response")?;
    Ok(String::from(js_sys::JsString::from(text)))
}

// ---------------------------------------------------------------- SHA-256

/// Minimal SHA-256 so a verified update needs no extra crate and no system tool.
/// Native only: the browser build never installs anything.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

#[cfg(not(target_arch = "wasm32"))]
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[cfg(not(target_arch = "wasm32"))]
impl Sha256 {
    pub fn new() -> Self {
        Sha256 {
            h: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0; 64],
            buf_len: 0,
            total: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.block(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.block(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn block(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in self.h.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(v);
        }
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.total.wrapping_mul(8);
        let mut pad = vec![0x80u8];
        let rem = (self.total as usize + 1) % 64;
        let zeros = if rem <= 56 { 56 - rem } else { 64 - rem + 56 };
        pad.extend(std::iter::repeat_n(0u8, zeros));
        pad.extend_from_slice(&bits.to_be_bytes());
        self.update(&pad);
        let mut out = [0u8; 32];
        for (i, word) in self.h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    pub fn hex(self) -> String {
        self.finish().iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience for tests and one-shot hashing.
#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Byte-at-a-time must match one-shot (streaming path used by downloads).
        let mut h = Sha256::new();
        for b in b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq" {
            h.update(&[*b]);
        }
        assert_eq!(
            h.hex(),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn file_url(p: &std::path::Path) -> String {
        format!("file://{}", p.display())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn wait_event(slot: &Arc<Mutex<Vec<UpdateEvent>>>) -> UpdateEvent {
        for _ in 0..200 {
            if let Ok(mut q) = slot.lock() {
                if !q.is_empty() {
                    return q.remove(0);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("update worker produced no event");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn check_worker_reads_a_local_release() {
        let dir = std::env::temp_dir().join(format!("fomako-update-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let json = r#"{
            "tag_name": "v9.9.9",
            "html_url": "https://example.com/releases/v9.9.9",
            "assets": [
                {"name": "SHA256SUMS", "browser_download_url": "https://example.com/SHA256SUMS"},
                {"name": "fomako-church-controller-linux-x64", "browser_download_url": "https://example.com/app"}
            ]
        }"#;
        let api = dir.join("release.json");
        std::fs::write(&api, json).unwrap();
        // The worker shells out (curl/wget), so a file:// URL exercises the real path.
        std::env::set_var("FOMAKO_UPDATE_API", file_url(&api));
        let slot = Arc::new(Mutex::new(Vec::new()));
        check_worker(slot.clone(), None);
        let ev = wait_event(&slot);
        std::env::remove_var("FOMAKO_UPDATE_API");
        let _ = std::fs::remove_dir_all(&dir);
        match ev {
            UpdateEvent::Available { version, asset, .. } => {
                assert_eq!(version, "9.9.9");
                assert_eq!(asset, "fomako-church-controller-linux-x64");
            }
            other => panic!("expected Available, got {:?}", other),
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn download_verifies_sha256_and_refuses_a_bad_hash() {
        let dir = std::env::temp_dir().join(format!("fomako-update-dl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let asset = "fomako-church-controller-linux-x64";
        let payload: Vec<u8> = (0..1_200_000u32).map(|i| (i % 251) as u8).collect();
        let bin = dir.join(asset);
        std::fs::write(&bin, &payload).unwrap();
        let hash = sha256_hex(&payload);
        let sums = dir.join("SHA256SUMS");
        // Same format the release job writes: "<hash>  <name>".
        std::fs::write(&sums, format!("{hash}  {asset}\n")).unwrap();
        let staging = dir.join("stage");
        std::fs::create_dir_all(&staging).unwrap();

        let mut seen = 0u64;
        let got = download_verified(&file_url(&bin), &file_url(&sums), asset, &staging, |n| seen = n)
            .expect("verified download");
        assert_eq!(std::fs::read(&got).unwrap(), payload);
        assert!(seen >= payload.len() as u64, "progress reported {seen}");

        // Tampered payload, stale checksum: must refuse and leave nothing installed.
        std::fs::write(&bin, vec![0u8; 1_200_000]).unwrap();
        let err = download_verified(&file_url(&bin), &file_url(&sums), asset, &staging, |_| {})
            .unwrap_err();
        assert!(err.contains("Checksum mismatch"), "{err}");

        // A release without SHA256SUMS must not install blindly either.
        let err = download_verified(&file_url(&bin), "", asset, &staging, |_| {}).unwrap_err();
        assert!(err.contains("no SHA256SUMS"), "{err}");

        // An error page instead of a binary is caught by the size guard.
        let tiny = dir.join("tiny");
        std::fs::write(&tiny, b"<html>404</html>").unwrap();
        let tiny_sums = dir.join("TINYSUMS");
        std::fs::write(&tiny_sums, format!("{}  {asset}\n", sha256_hex(b"<html>404</html>"))).unwrap();
        let err = download_verified(&file_url(&tiny), &file_url(&tiny_sums), asset, &staging, |_| {})
            .unwrap_err();
        assert!(err.contains("not the app"), "{err}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn help_page_offers_the_update_card() {
        let mut rt = crate::model::App::with_test_runtime_sized((1120.0, 1600.0));
        rt.update(|s, _| s.page = crate::model::Page::Help);
        rt.frame(0.0);
        assert!(rt.find("update-check").is_some(), "check button missing");
        assert!(rt.find("update-releases").is_some(), "release link missing");

        // Available swaps in the one-click install (reload on web).
        rt.update(|s, _| {
            s.update_phase = UpdatePhase::Available;
            s.update_version = "9.9.9".to_string();
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.find("update-install").is_some(), "install button missing");

        // Installed adds the restart step instead.
        rt.update(|s, _| s.update_phase = UpdatePhase::Ready);
        rt.frame(2.0 / 60.0);
        assert!(rt.find("update-restart").is_some(), "restart button missing");
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.5.0", "0.4.0"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("0.4.1", "0.4.0"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("v0.3.9", "0.4.0"));
        assert!(!is_newer("0.4.0", "v0.4.0"));
        assert!(!is_newer("garbage", "0.4.0"));
    }

    #[test]
    fn parses_github_release() {
        let json = r#"{
            "tag_name": "v0.5.0",
            "html_url": "https://github.com/Jemo69/fomako-church-controller/releases/tag/v0.5.0",
            "assets": [
                {"name": "SHA256SUMS", "browser_download_url": "https://example.com/SHA256SUMS"},
                {"name": "fomako-church-controller-linux-x64", "browser_download_url": "https://example.com/app"},
                {"name": "fomako-church-controller-windows-x64.exe", "browser_download_url": "https://example.com/app.exe"}
            ]
        }"#;
        let rel = parse_release(json, "fomako-church-controller-linux-x64").unwrap();
        assert_eq!(rel.version, "0.5.0");
        assert_eq!(rel.asset, "fomako-church-controller-linux-x64");
        assert_eq!(rel.url, "https://example.com/app");
        assert_eq!(rel.sums_url, "https://example.com/SHA256SUMS");
        assert!(rel.page_url.ends_with("v0.5.0"));

        // A release without this platform's asset must fail loudly, not silently.
        let err = parse_release(json, "fomako-church-controller-mac-arm64").unwrap_err();
        assert!(err.contains("no fomako-church-controller-mac-arm64 build"));
        // API errors carry GitHub's message.
        let err = parse_release(r#"{"message": "Not Found"}"#, "x").unwrap_err();
        assert!(err.contains("Not Found"));
    }
}
