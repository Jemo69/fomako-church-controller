//! "Match this picture": read a reference image, measure its look, and tune the
//! camera's picture settings so the live image looks like it.
//!
//! This is deliberately separate from the camera's own AI tracking: nothing here
//! pans, tilts or zooms. Only image settings move — brightness, contrast,
//! saturation, sharpness and (optionally) colour balance via R/B gain.
//!
//! The controller is closed-loop: the reference gives target statistics, the
//! live preview gives current statistics, and each pass nudges the camera a
//! little toward the target. That copes with different lighting and firmware
//! gamma, where a one-shot absolute mapping would miss.

/// What an image looks like, all in 0..1 (warmth is -1..1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Mean luma: the overall brightness.
    pub luma: f32,
    /// Luma standard deviation: contrast.
    pub contrast: f32,
    /// Mean chroma: saturation.
    pub saturation: f32,
    /// Mean neighbour gradient: how much fine detail (sharpness) is visible.
    pub detail: f32,
    /// Red-vs-blue balance: > 0 is warm, < 0 is cool.
    pub warmth: f32,
}

/// Camera picture values this feature drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub brightness: u8,
    pub contrast: u8,
    pub saturation: u8,
    pub aperture: u8,
}

fn aspect_size(w: u32, h: u32, max_w: u32) -> (u32, u32) {
    let scale = (max_w as f32 / w.max(1) as f32).min(1.0);
    (
        ((w.max(1) as f32 * scale).round() as u32).max(1),
        ((h.max(1) as f32 * scale).round() as u32).max(1),
    )
}

/// Measure one decoded image. Shared by the reference picture and live frames.
pub fn analyze(img: &image::DynamicImage) -> Stats {
    let (w, h) = aspect_size(img.width(), img.height(), 160);
    let rgb = img
        .resize_exact(w, h, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let (w, h) = (w as usize, h as usize);
    let px = rgb.as_raw();

    let mut luma_sum = 0.0f32;
    let mut luma_sq = 0.0f32;
    let mut sat_sum = 0.0f32;
    let mut r_sum = 0.0f32;
    let mut b_sum = 0.0f32;
    let mut rows: Vec<Vec<f32>> = Vec::with_capacity(h);
    for y in 0..h {
        let mut row = Vec::with_capacity(w);
        for x in 0..w {
            let i = (y * w + x) * 3;
            let (r, g, b) = (px[i] as f32 / 255.0, px[i + 1] as f32 / 255.0, px[i + 2] as f32 / 255.0);
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            let hi = r.max(g).max(b);
            let lo = r.min(g).min(b);
            sat_sum += if hi > 1e-4 { (hi - lo) / hi } else { 0.0 };
            luma_sum += luma;
            luma_sq += luma * luma;
            r_sum += r;
            b_sum += b;
            row.push(luma);
        }
        rows.push(row);
    }

    let n = (w * h).max(1) as f32;
    let luma = luma_sum / n;
    let contrast = (luma_sq / n - luma * luma).max(0.0).sqrt();

    // Neighbour differences, skipping flat edges, as a detail/sharpness proxy.
    let mut grad_sum = 0.0f32;
    let mut grad_n = 0.0f32;
    for y in 0..h {
        for x in 0..w {
            if x + 1 < w {
                grad_sum += (rows[y][x + 1] - rows[y][x]).abs();
                grad_n += 1.0;
            }
            if y + 1 < h {
                grad_sum += (rows[y + 1][x] - rows[y][x]).abs();
                grad_n += 1.0;
            }
        }
    }

    let (r_mean, b_mean) = (r_sum / n, b_sum / n);
    Stats {
        luma,
        contrast,
        saturation: (sat_sum / n).clamp(0.0, 1.0),
        detail: if grad_n > 0.0 { grad_sum / grad_n } else { 0.0 },
        warmth: ((r_mean - b_mean) / (r_mean + b_mean + 1e-3)).clamp(-1.0, 1.0),
    }
}

pub fn analyze_bytes(bytes: &[u8]) -> Result<Stats, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("picture: {e}"))?;
    Ok(analyze(&img))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn analyze_file(path: &str) -> Result<Stats, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    analyze_bytes(&bytes)
}

#[cfg(target_arch = "wasm32")]
pub fn analyze_file(_path: &str) -> Result<Stats, String> {
    Err("the browser build can't read local files — use the desktop app".into())
}

/// Reference pictures dropped into `dir` (the app's `looks` folder), ready to tap.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_pictures(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp") {
                if let Some(s) = path.to_str() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

#[cfg(target_arch = "wasm32")]
pub fn list_pictures(_dir: &str) -> Vec<String> {
    Vec::new()
}

/// One closed-loop step: move the camera a little toward the reference look.
/// Returns the new values and whether anything changed.
pub fn nudge(target: &Stats, live: &Stats, cur: Settings) -> (Settings, bool) {
    let step = |err: f32, cur: u8, max: u8, gain: f32, deadband: f32| -> u8 {
        if err.abs() <= deadband {
            return cur;
        }
        let d = (err * gain).round() as i32;
        (cur as i32 + d.clamp(-2, 2)).clamp(0, max as i32) as u8
    };
    let next = Settings {
        brightness: step(target.luma - live.luma, cur.brightness, 14, 30.0, 0.02),
        contrast: step(target.contrast - live.contrast, cur.contrast, 14, 55.0, 0.015),
        saturation: step(target.saturation - live.saturation, cur.saturation, 14, 45.0, 0.02),
        aperture: step(target.detail - live.detail, cur.aperture, 15, 180.0, 0.004),
    };
    (next, next != cur)
}

/// One colour step: warmer needs more R gain and less B gain, cooler the reverse.
pub fn nudge_color(target_warmth: f32, live_warmth: f32, r_gain: u8, b_gain: u8) -> (u8, u8, bool) {
    let err = target_warmth - live_warmth;
    if err.abs() <= 0.02 {
        return (r_gain, b_gain, false);
    }
    let d = (err * 90.0).round() as i32;
    let d = d.clamp(-6, 6);
    let r = (r_gain as i32 + d).clamp(0, 255) as u8;
    let b = (b_gain as i32 - d).clamp(0, 255) as u8;
    (r, b, r != r_gain || b != b_gain)
}

/// Human-readable summary for the status line.
pub fn describe(s: &Stats) -> String {
    format!(
        "brightness {:.0}% · contrast {:.0}% · saturation {:.0}% · detail {:.0}% · warmth {:+.0}%",
        s.luma * 100.0,
        s.contrast * 100.0,
        s.saturation * 100.0,
        s.detail * 100.0,
        s.warmth * 100.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(r: u8, g: u8, b: u8) -> Stats {
        let img = image::RgbImage::from_fn(32, 32, |_, _| image::Rgb([r, g, b]));
        analyze(&image::DynamicImage::ImageRgb8(img))
    }

    #[test]
    fn black_and_white_reflect_brightness() {
        let dark = solid(10, 10, 10);
        let light = solid(240, 240, 240);
        assert!(dark.luma < 0.1);
        assert!(light.luma > 0.9);
        assert!(dark.contrast < 0.02 && light.contrast < 0.02);
    }

    #[test]
    fn saturated_red_is_more_saturated_than_gray() {
        let red = solid(220, 20, 20);
        let gray = solid(120, 120, 120);
        assert!(red.saturation > 0.5, "{}", red.saturation);
        assert!(gray.saturation < 0.02);
        assert!(red.warmth > 0.5);
        assert!(gray.warmth.abs() < 0.02);
    }

    #[test]
    fn checkerboard_has_more_detail_than_flat() {
        let checker = image::RgbImage::from_fn(32, 32, |x, y| {
            if (x / 4 + y / 4) % 2 == 0 {
                image::Rgb([230, 230, 230])
            } else {
                image::Rgb([20, 20, 20])
            }
        });
        let flat = image::RgbImage::from_fn(32, 32, |_, _| image::Rgb([120, 120, 120]));
        let a = analyze(&image::DynamicImage::ImageRgb8(checker));
        let b = analyze(&image::DynamicImage::ImageRgb8(flat));
        assert!(a.detail > b.detail * 10.0);
    }

    #[test]
    fn nudge_moves_toward_target_and_settles() {
        let target = Stats { luma: 0.6, contrast: 0.25, saturation: 0.4, detail: 0.05, warmth: 0.2 };
        let cur = Settings { brightness: 7, contrast: 7, saturation: 4, aperture: 8 };
        let live = Stats { luma: 0.4, ..target };
        let (next, changed) = nudge(&target, &live, cur);
        assert!(changed);
        assert!(next.brightness > cur.brightness, "{} -> {}", cur.brightness, next.brightness);

        let settled = nudge(&target, &target, cur);
        assert!(!settled.1, "identical looks need no change");
        assert_eq!(settled.0, cur);
    }

    #[test]
    fn lists_and_reads_pictures_from_a_folder() {
        let dir = std::env::temp_dir().join(format!("fomako-looks-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        image::RgbImage::from_fn(16, 16, |_, _| image::Rgb([200, 100, 50]))
            .save(&png)
            .unwrap();
        std::fs::write(dir.join("notes.txt"), b"not a picture").unwrap();
        let found = list_pictures(dir.to_str().unwrap());
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("a.png"));
        let stats = analyze_file(&found[0]).expect("analyze file");
        assert!(stats.luma > 0.2, "{}", stats.luma);
        assert!(stats.warmth > 0.0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn colour_step_goes_warm_and_cool() {
        let (r, b, changed) = nudge_color(0.3, 0.0, 32, 32);
        assert!(changed);
        assert!(r > 32 && b < 32);
        let (r2, b2, _) = nudge_color(-0.3, 0.0, 32, 32);
        assert!(r2 < 32 && b2 > 32);
        assert_eq!(nudge_color(0.0, 0.0, 32, 32), (32, 32, false));
    }
}
