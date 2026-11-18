//! CPU stand-in for CSS `filter: blur(90px) saturate(130%) brightness(.6)` (GPUI has no filter /
//! backdrop-filter): downscale, adjust, gaussian-blur once, and let the GPU upscale it bilinearly.
use anyhow::Result;
use image::{imageops, DynamicImage, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

pub fn blurred(src: &Path, cache: &Path, id: &str, brightness: f32, saturate: f32) -> Result<PathBuf> {
    blurred_as(src, cache, id, "bd", brightness, saturate)
}

pub fn blurred_as(src: &Path, cache: &Path, id: &str, tag: &str, brightness: f32, saturate: f32) -> Result<PathBuf> {
    let out = cache.join(format!("{id}-{tag}.png"));
    if out.exists() {
        return Ok(out);
    }
    let img: DynamicImage = image::open(src)?;
    // 64 px wide: sigma 4 here is about 90 css px on a ~1500 px wide surface
    let small = imageops::resize(&img.to_rgba8(), 64, 64, imageops::FilterType::Triangle);
    let mut adj = RgbaImage::new(64, 64);
    for (x, y, p) in small.enumerate_pixels() {
        let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
        let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let f = |c: f32| (((l + (c - l) * saturate) * brightness).clamp(0., 255.)) as u8;
        adj.put_pixel(x, y, Rgba([f(r), f(g), f(b), 255]));
    }
    let blurred = imageops::blur(&adj, 4.0);
    blurred.save(&out)?;
    Ok(out)
}
