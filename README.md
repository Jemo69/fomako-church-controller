# FoMaKo Church PTZ Controller

Control your church's FoMaKo PTZ cameras from Windows / macOS / Linux — pan, tilt, zoom, focus, presets, full image settings, and a photo-style **Picture Edit** with live preview.

Built with **Rune** — a native GPU-rendered Rust UI (`github.com/anomalyco/rune`, `dev` rev `4a072bb06d1b410989f444caf0c9095b8596a565`).

Manual: [FoMaKo NDI/SDI/HDMI PTZ User Manual V3.0 (PDF)](https://fomako.net/uploads/20230817/fd1bfd181b89ae798bee1f7ed260a7b3.pdf)

![Control](shots/control.png) ![Picture Edit](shots/picture.png)

## What it does

- **Control page**: PTZ pad (↖ ↑ ↗ ← HOME → ↙ ↓ ↘ + STOP), Zoom Tele/Wide + Stop, Focus Near/Far + Auto/Manual/1-Push, Pan/Tilt/Zoom speeds, Power ON/Standby, Presets 0–254 (quick 0–9 grid), multi-camera switcher.
- **Image page**: every FoMaKo setting from manual §5.2 — Exposure (Full Auto/Manual/Shutter/Iris/Bright), Shutter, Iris, Gain limit, Bright, ExpComp, Backlight, WDR, NR 2D/3D, Flicker, Gamma, Low-light, Aperture/Sharpness, Brightness, Contrast, Saturation (60–200%), WB + R/B Gain, Flip H/V, B/W, etc. Each row sends real VISCA.
- **Picture Edit**: photo-style Brightness / Contrast / Saturation / Sharpness sliders (-100…+100) with **live simulated preview** (color bars + gray ramp). **Apply to Camera** sends VISCA A1/A2/49/42. This is the “make the picture look like I want” feature.
- **Cameras page**: add/remove, Select, Test (sends Stop + shows ACK), protocol toggle (Sony VISCA UDP :52381 recommended, or IP-VISCA TCP :5678), saved to `fomako-cameras.json` beside the app (copy to every booth PC). Shows RTSP/RTMP/Web UI URLs for VLC/OBS/vMix.
- **Help page**: 5-minute church setup (wire, lock IP/DHCP OFF, protocol, Sunday preset workflow, picture).

### Protocols (from manual p1-2)

- IP Visca port **5678** (TCP, raw `81 … FF`)
- Sony Visca port **52381** (UDP, `01 00 len seq + payload`)
- Onvif **2000**, user `admin` / `admin`, default IP `192.168.5.163`, LCD shows DHCP IP
- RTSP `rtsp://IP:554/live/av0` (main) / `av1` (sub), RTMP `rtmp://IP:1935/live/av0`, SRT `:9000`, NDI (NDI models)

This app is **control-only** for video — keep VLC/OBS open with the RTSP beside it (FoMaKo streams via RTSP/RTMP/SRT/NDI, manual §4.3).

## Run

```sh
cargo run
# headless check (no window, same renderer):
cargo run -- --snap shots/control.png --scene control
cargo run -- --snap shots/image.png --scene image
cargo run -- --snap shots/picture.png --scene picture
```

Scenes: `control image picture cameras help light`.

## Build (in this directory)

```sh
scripts/build-linux.sh    # dist/fomako-church-controller-linux-x64
scripts/build-windows.sh  # dist/*.exe (x86_64-pc-windows-gnu, needs mingw)
scripts/build-mac.sh      # run on a Mac: dist/*-mac-*
scripts/build-all.sh
```

Linux needs: `libxkbcommon-dev libwayland-dev` (+ `libxkbcommon-x11-0 mesa-vulkan-drivers` to run).

Windows cross from Linux needs: `mingw-w64` (`x86_64-w64-mingw32-gcc`) + `rustup target add x86_64-pc-windows-gnu`.
Mac builds must run on a Mac (`rustup target add x86_64-apple-darwin aarch64-apple-darwin`).

Config file `fomako-cameras.json` is created next to the binary on first run.

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
- `scripts/` — linux/windows/mac builds
- `shots/` — headless renders
