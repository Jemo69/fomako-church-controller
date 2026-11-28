# FoMaKo Church PTZ Controller

Control your church's FoMaKo PTZ cameras from Windows / macOS / Linux — pan, tilt, zoom, focus, presets, full image settings, and a photo-style **Picture Edit** with live preview.

Built with **Rune** — a native GPU-rendered Rust UI (`github.com/anomalyco/rune`, `dev` rev `4a072bb06d1b410989f444caf0c9095b8596a565`).

Manual: [FoMaKo NDI/SDI/HDMI PTZ User Manual V3.0 (PDF)](https://fomako.net/uploads/20230817/fd1bfd181b89ae798bee1f7ed260a7b3.pdf)

![Control](shots/control.png) ![Picture Edit](shots/picture.png)

## What it does

- **Control page**: PTZ pad (↖ ↑ ↗ ← HOME → ↙ ↓ ↘ + STOP), Zoom Tele/Wide + Stop, Focus Near/Far + Auto/Manual/1-Push, Pan/Tilt/Zoom speeds, Power ON/Standby, Presets 0–254 (quick 0–9 grid), multi-camera switcher, **live preview** (continuous RTSP stream through ffmpeg at up to 30 fps; HTTP stills only as a fallback when ffmpeg is missing), and **AI auto tracking** on models that have it (Tracking ON/OFF + real-time/area mode, see below).
- **Image page**: every FoMaKo setting from manual §5.2 — Exposure (Full Auto/Manual/Shutter/Iris/Bright), Shutter, Iris, Gain limit, Bright, ExpComp, Backlight, WDR, NR 2D/3D, Flicker, Gamma, Low-light, Aperture/Sharpness, Brightness, Contrast, Saturation (60–200%), WB + R/B Gain, Flip H/V, B/W, etc. Each setting is one plain-language row — name, what it does, the valid range, a decimal value with − / +, and its own Apply button right beside the value; the VISCA code (4A, 4B, …) is in the small hint text for anyone who wants it. Switches name their next action ("Turn on", "Next: horizontal") instead of a generic Apply. Same layout on the Android app.
- **Picture Edit**: photo-style Brightness / Contrast / Saturation / Sharpness sliders (-100…+100) with **live simulated preview** (color bars + gray ramp). **Apply to Camera** sends VISCA A1/A2/49/42. This is the “make the picture look like I want” feature.
- **Match a picture (image look)**: load a reference photo and the **app** measures it (brightness, contrast, saturation, detail, warmth) and tunes the camera’s picture settings to match — Brightness, Contrast, Saturation and Sharpness, plus optional colour via R/B Gain. **Match now** does one pass; **Auto match** keeps nudging about every 2.5 s while the preview runs. This changes image settings only: the camera’s position never moves. Separate from the camera’s own AI tracking below.
- **Cameras page**: **auto-find** (subnet scan: TCP 5678 + UDP 52381 + HTTP sniff + ONVIF discovery, one-tap Add), add/remove, Select, Test (sends Stop + shows ACK), protocol toggle (Sony VISCA UDP :52381 recommended, or IP-VISCA TCP :5678), **per-camera login** (RTSP/web use it; VISCA needs none), saved to `fomako-cameras.json` beside the app (copy to every booth PC). Shows RTSP/RTMP/Web UI URLs for VLC/OBS/vMix.
- **Cross-camera AI follow**: pick a leader and a follower, point both at one landmark, **Calibrate**, then **Start**. The leader's built-in AI tracking moves it; the follower repeats that pan/tilt aim over VISCA (absolute position + position inquiry). Both cameras then frame the same subject. The follower's own AI tracking must be off. This mirrors the leader's *aim* — FoMaKo cameras expose no subject coordinates or tracking status over VISCA, so a true shared-subject solve is not possible without a vision tracker. See [Cross-camera AI follow](#cross-camera-ai-follow).
- **Android (Native APK — recommended for Android)**: standalone Jetpack Compose app (`android/`) powered by the Rust VISCA & LAN discovery engine via JNI (`src/android_jni.rs`). Connects **directly** over WiFi to FoMaKo cameras (Sony VISCA UDP `:52381`, IP-VISCA TCP `:5678`, ONVIF + subnet scan) and plays the **live RTSP preview at the camera's frame rate** (`/live/av1` via Android MediaPlayer), with an optional relay MJPEG stream when relay is on — **no PC or relay needed**. Build with `scripts/build-android.sh` → `dist/fomako-church-controller-android.apk`.
- **Phones (iPhone / Android browser)**: open the web build in the phone browser (same WiFi). Because Android Chrome requires HTTPS for WebGPU on LAN IPs, pass `--https` to the LAN relay: `python3 scripts/visca-relay.py --port 8443 --https --serve-web dist/web`, then open the printed `https://<PC-LAN-IP>:8443/#relay=...&subnet=...&autostart=scan` URL (accept the self-signed cert prompt once; put the URL in a booth QR code). Rune has no native Android/iOS backend yet (`web → iOS → Android`), so the native Android app uses Jetpack Compose + Rust JNI while iOS uses the browser + relay path.
- **Auto-updates**: at launch (and every 6 hours) the app asks GitHub for the newest release and puts **⬆ Update vX** in the title bar and an update card on the Help page. One click downloads this platform's binary, checks it against the release's `SHA256SUMS`, swaps it beside the running app and then asks before restarting — cameras and `fomako-cameras.json` are untouched, and the app never restarts while you're on air. `FOMAKO_NO_UPDATE_CHECK=1` turns the automatic check off; `FOMAKO_UPDATE_API=<url>` points it at a mirror.
- **Help page**: 5-minute church setup (wire, lock IP/DHCP OFF, protocol, Sunday preset workflow, picture).

### Protocols (from manual p1-2)

- IP Visca port **5678** (TCP, raw `81 … FF`)
- Sony Visca port **52381** (UDP, `01 00 len seq + payload`)
- Onvif **2000**, user `admin` / `admin`, default IP `192.168.5.163`, LCD shows DHCP IP
- RTSP `rtsp://IP:554/live/av0` (main) / `rtsp://IP:554/live/av1` (sub, used by the preview); some older firmware uses `/1` and `/2`. RTMP `rtmp://IP:1935/live/av0`, SRT `:9000`, NDI (NDI models)

### Match a picture (image look — app-side analysis)

Picture Edit → **Match a picture** takes a reference image and makes the camera's picture match it. The app reads the file locally, measures brightness (mean luma), contrast (luma standard deviation), saturation (mean chroma), detail (mean neighbour gradient) and warmth (red-vs-blue balance), then compares those against the live preview frame. It only changes image settings:

- Brightness → VISCA A1, Contrast → A2, Saturation → 49, Sharpness/Aperture → 42
- Optional **Colour** matching → WB Manual (35) with R/B Gain trims (43/44)

**Match now** does one closed-loop pass; **Auto match** repeats every ~2.5 s so gradual lighting changes are followed. Nothing here pans, tilts or zooms — that is the separate camera AI tracking feature below. Colour matching leaves WB in Manual, so set it back to Auto on the Image page when you're done. Drop reference pictures into the app's `looks/` folder (or type a path and press Load); Picture Edit lists up to eight as one-tap buttons. The app reads the file locally — it is never uploaded to the camera.

### AI auto tracking (AI-tracking models)

FoMaKo's AI-tracking models (FMK/K600/K630 Gen 3 and similar) do the following inside the camera. The Control page has a **Tracking ON / Tracking OFF** card that sends the documented command from the K600/K630 manual §5.2:

- Real-time tracking ON — `81 0A 01 32 00 00 02 00 FF`
- Area tracking ON — `81 0A 01 32 00 00 02 01 FF`
- Tracking OFF — `81 0A 01 32 00 00 03 00 FF`

Real-time mode follows one person; press **F4** on the IR remote (or click their box in the camera's web page) to switch target. Area mode follows movement inside up to four areas, which are configured in the camera web page first (at least two). While tracking is on the camera ignores pan/tilt — the app's status line says so, and **Tracking OFF** hands control back. Non-AI models reject the command; `Cameras → Test` shows the camera's answer.

### Cross-camera AI follow

FoMaKo Gen-3 models (K20/K30/K600N/K800N, FMK/NDI `AI` models) auto-track a person inside the camera. Cross-camera subject sharing is **not** available on the hardware. The app can switch the camera's tracking on and off (§5.2, `8x 0A 01 32 …`), but the camera publishes no subject coordinates and no tracking-status query. There is nothing for camera 2 to receive except pan/tilt.

This app therefore mirrors the leader's aim instead of sharing the subject:

1. Turn tracking ON for the leader with the Control page card, its remote (`F2`), or its web page.
2. Turn the follower's own AI tracking **off**, or the two controllers fight.
3. Point both cameras at the same landmark and press **Calibrate** — the app reads both pan/tilt positions and stores the offset (`fomako-follow.json`, working directory first, then beside the app).
4. Press **Start** on the Control page. The app polls the leader's `8x 09 06 12 FF` position about 8×/s while it answers (slower when it does not) and sends the follower `8x 01 06 02 …`.

Use the **Pan/Tilt arrow** nudge buttons (100-count steps ≈ 0.5°) to fine-tune the framing without moving the leader. Works best with both cameras near each other, a distant subject, and similar zoom. Parallax grows with baseline and zoom, and the follower only sees what the leader sees — it does not re-acquire the subject after a cut or a complete occlusion.

A true shared-subject tracker would need frames from both cameras, a person detector/ReID model and a control loop in this app. That is the next step, not something the cameras can do themselves.

### Live preview + camera passwords

The Control page shows a **live preview** at up to **30 fps**. Native builds run one long-lived ffmpeg that decodes the camera's RTSP sub stream (`/2` or `/live/av1`, falling back to the main stream `/1`/`/live/av0`) and transcodes it to MJPEG frames; the app splits and draws them, dropping frames instead of lagging behind. The web build asks the relay for the same stream (`GET /api/stream`) and paints it on a browser-native JPEG canvas over the UI, so browser decode/compositing keeps video at 30 fps even when Rune's software canvas is slow. If ffmpeg is absent (or RTSP fails), it falls back to polling `http://IP/action_snapshot` as fast as the camera answers. Install ffmpeg for full frame rate (`apt install ffmpeg` / `brew install ffmpeg` / `winget install ffmpeg`, or drop `ffmpeg(.exe)` next to the app). Pause stops the stream. The status line shows the measured **fps**.

**Passwords:** VISCA control needs **no** password. The **video stream often does**: set each camera's login on the Cameras page (default `admin` / `admin` — change it there if the camera says 401). The preview then shows `retrying… 401 Unauthorized — check camera login` instead of failing silently. Passwords are stored in your local `fomako-cameras.json` only, never logged, and URL-encoded safely (`p@ss` can't break the RTSP URL). `FOMAKO_RTSP_PORT` env overrides port 554 for remapped/NVR setups.

Full streams still live in VLC/OBS (FoMaKo speaks RTSP/RTMP/SRT/NDI, manual §4.3).

## Auto-updates

Windows, macOS and Linux builds update themselves; the browser build updates when the relay serves a new `dist/web` (the app tells you a newer release exists and links the release page).

- **Check** — `GET /repos/Jemo69/fomako-church-controller/releases/latest` a few seconds after launch and every 6 hours, plus a **Check for updates** button on Help. Nothing downloads until you click **Install**.
- **Download** — the release asset matching this OS/arch (`-linux-x64`, `-windows-x64.exe`, `-mac-arm64`, `-mac-x64`), streamed with `curl` (or `wget`) and hashed while it arrives. No new Rust crates and no system hash tool needed.
- **Verify** — the SHA-256 must match the release's `SHA256SUMS` entry. A mismatch, an error page, or a release without `SHA256SUMS` installs nothing.
- **Swap** — the verified binary goes in place of the running one (Unix: `rename()` over it; Windows: the old image steps aside as `*.old.exe`, deleted on next launch). No installer, no admin prompt, no temp service. If the app lives somewhere read-only (`/usr/bin`, `Program Files`), it says so and you install by hand.
- **Restart** — a separate click, so a download finishing never interrupts a service. `Restart now` opens the new build; config stays beside the app.

The release job publishes `SHA256SUMS` beside the binaries; if a release ever ships without it, the updater refuses to install and points at the release page instead.

## Run

```sh
cargo run
# headless check (no window, same renderer):
cargo run -- --snap shots/control.png --scene control
cargo run -- --snap shots/image.png --scene image
cargo run -- --snap shots/picture.png --scene picture
cargo run -- --snap shots/cameras.png --scene scan
# network tests (needs no camera; scan_live hits your real LAN):
cargo test
cargo test scan_live -- --ignored --nocapture
```

Scenes: `control image picture cameras help light scan`.

## Build (in this directory)

Status (v0.6.0):
- Linux x64: `dist/fomako-church-controller-linux-x64` + `.tar.gz` (app + static ffmpeg bundled — unzip and run, no install)
- Windows x64: `dist/fomako-church-controller-windows-x64.exe` + `.zip` (app + `ffmpeg.exe` bundled), cross-built here via `cargo zigbuild --target x86_64-pc-windows-gnu`
- Android: `dist/fomako-church-controller-android.apk` (standalone APK with Rust JNI VISCA/discovery core — direct WiFi control + JPEG preview, no PC needed)
- Web / phones: `dist/fomako-church-controller-web.zip` (UI + relay + setup sheet)
- Mac (Apple Silicon + Intel): `dist/fomako-church-controller-mac-arm64` / `-mac-x64` — both built from one `macos-15` runner in CI. Locally it needs a Mac (cross from Linux fails: no Apple `MacOSX.sdk` / `libobjc.tbd`); run `scripts/build-mac.sh` there. Binaries are unsigned, so right-click → Open the first time.

Prebuilt for everyone at [Releases](https://github.com/Jemo69/fomako-church-controller/releases) — current: **v0.6.0**.

`scripts/fetch-ffmpeg.sh [dest]` drops a static ffmpeg next to the app anywhere (used for the bundled zips). The app itself finds ffmpeg via `$FFMPEG_BIN`, `PATH`, its own folder, then per-OS install spots — see Help → "0. Live picture" for what it resolved to on your machine.

```sh
scripts/build-linux.sh    # dist/fomako-church-controller-linux-x64
scripts/build-windows.sh  # zig cross, no mingw needed
scripts/build-mac.sh      # run ON a Mac: dist/*-mac-*
scripts/build-android.sh  # dist/fomako-church-controller-android.apk
scripts/build-web.sh      # dist/web (phone + desktop browsers)
scripts/build-all.sh
```

Phone flow: on the LAN PC, `python3 scripts/visca-relay.py --port 8080 --serve-web dist/web`
(no pip packages needed), then open the printed URL + `#relay=...&subnet=...&autostart=scan`
on the phone. See `dist/web-release/PHONE_SETUP.txt`.

Linux needs: `libxkbcommon-dev libwayland-dev` (+ `libxkbcommon-x11-0 mesa-vulkan-drivers` to run).

Windows cross from Linux: `cargo install cargo-zigbuild` + `zig` (already used here) + `rustup target add x86_64-pc-windows-gnu`. Old mingw path also works.
Mac builds must run on a Mac (`rustup target add x86_64-apple-darwin aarch64-apple-darwin`), or via GitHub Actions (`macos-14`/`macos-15` runners).

Config file `fomako-cameras.json` is created next to the binary on first run.

### Every platform, portable

All five builds are portable: nothing is installed, no admin rights, no registry entries. Unzip to any folder — including a USB stick or external hard drive — and run it from there; `fomako-cameras.json` and the bundled ffmpeg travel with the folder, so the same drive works on any booth PC. Android is a plain sideloadable APK. If any platform behaves worse than the others on your machine (GPU rendering, preview, discovery), open an issue with the OS and GPU driver — parity across Linux / Windows / macOS / Android is the goal, and reports are what make it happen.

## Sunday workflow

1. Frame pulpit → Preset Sel `1` → **Set**. Choir → `2` → Set. Wide → `3` → Set.
2. During service tap **Recall 1/2/3** (or quick 0–9).
3. Keep Pan 10–16 / Tilt 10–14 for smooth moves. Preset 0 = power-on position if saved.

## Repo layout

- `src/main.rs` — Rune `AppSpec` + scenes
- `src/view.rs` — all UI (control/image/picture/cameras/help)
- `src/model.rs` — cameras, pages, image state, `fomako-cameras.json` persistence
- `src/actions.rs` — one-line VISCA send + log
- `src/visca.rs` — VISCA builders (§5.2) + Sony-UDP / TCP transport + tests
- `src/discover.rs` — LAN auto-find (subnet scan + ONVIF) + relay client + tests
- `src/preview.rs` — live preview (persistent RTSP → MJPEG stream via ffmpeg, relay stream on web, HTTP-still fallback)
- `scripts/` — linux/windows/mac/web builds + `visca-relay.py` (phone relay: scan, MJPEG stream, snapshot fallback, static web UI)
- `shots/` — headless renders + boaz desktop captures (`preview-live.png`, `preview-drive.mp4`)
