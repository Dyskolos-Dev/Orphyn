//! Now Playing backdrop: the cover fetched small, heavily blurred ONCE on the CPU (three box
//! blur passes ≈ gaussian, plus a saturation boost), cached per album, then shown stretched.
//! Nothing is filtered at runtime, so opening the sheet stays smooth.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use dioxus::prelude::*;

use super::{track_art, use_base_url, use_music};

const SIZE: u32 = 128;
const RADIUS: usize = 10;
const SATURATION: f32 = 1.6;
const CACHE_MAX: usize = 24;

fn cache() -> &'static Mutex<HashMap<String, String>> {
    static C: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// One horizontal box blur pass (edge-clamped) over `w`×`h` RGB data; `src` → `dst`.
fn box_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let n = (2 * r + 1) as f32;
    for y in 0..h {
        for c in 0..3 {
            let at = |x: isize| src[(y * w + x.clamp(0, w as isize - 1) as usize) * 3 + c];
            let mut sum = 0.0;
            for k in -(r as isize)..=(r as isize) {
                sum += at(k);
            }
            for x in 0..w {
                dst[(y * w + x) * 3 + c] = sum / n;
                sum += at((x + r + 1) as isize) - at(x as isize - r as isize);
            }
        }
    }
}

fn transpose(src: &[f32], dst: &mut [f32], w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                dst[(x * h + y) * 3 + c] = src[(y * w + x) * 3 + c];
            }
        }
    }
}

/// Blur + saturate encoded cover bytes into a PNG data URI.
pub fn blur_to_data_uri(bytes: &[u8]) -> Option<String> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = img.resize_exact(SIZE, SIZE, image::imageops::FilterType::Triangle).to_rgb8();
    let (w, h) = (SIZE as usize, SIZE as usize);
    let mut a: Vec<f32> = img.as_raw().iter().map(|&v| v as f32).collect();
    let mut b = vec![0.0; a.len()];
    // 3 passes per axis; square image, so one transpose swaps axes cleanly.
    for _axis in 0..2 {
        for _ in 0..3 {
            box_pass(&a, &mut b, w, h, RADIUS);
            std::mem::swap(&mut a, &mut b);
        }
        transpose(&a, &mut b, w, h);
        std::mem::swap(&mut a, &mut b);
    }
    let mut out = Vec::with_capacity(a.len());
    for px in a.as_chunks::<3>().0 {
        let l = 0.299 * px[0] + 0.587 * px[1] + 0.114 * px[2];
        for &v in px {
            out.push((l + (v - l) * SATURATION).clamp(0.0, 255.0) as u8);
        }
    }
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, SIZE, SIZE);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().ok()?.write_image_data(&out).ok()?;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes)
    ))
}

/// The blurred backdrop of the current track as `(album key, data URI)`; keeps the previous one
/// until the new cover is ready, so the crossfade never flashes empty.
pub fn use_backdrop() -> Option<(String, String)> {
    let music = use_music();
    let base = use_base_url();
    let res = use_resource(move || {
        let base = base.clone();
        async move {
            let track = (music.state)().current().map(|q| q.track.clone())?;
            let key = track.album_id.clone().unwrap_or_else(|| track.id.clone());
            if let Some(hit) = cache().lock().ok()?.get(&key).cloned() {
                return Some((key, hit));
            }
            let url = track_art(&base, &track, SIZE)?;
            let bytes = reqwest::get(&url).await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
            let uri = blur_to_data_uri(&bytes)?;
            let mut c = cache().lock().ok()?;
            if c.len() >= CACHE_MAX {
                c.clear();
            }
            c.insert(key.clone(), uri.clone());
            Some((key, uri))
        }
    });
    let v = res.read().clone().flatten();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blurs_a_cover_to_a_png_data_uri() {
        let mut raw = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut raw, 8, 8);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let data: Vec<u8> = (0..8 * 8).flat_map(|i| if i % 2 == 0 { [255, 0, 0] } else { [0, 0, 255] }).collect();
            enc.write_header().unwrap().write_image_data(&data).unwrap();
        }
        let uri = blur_to_data_uri(&raw).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
    }
}
