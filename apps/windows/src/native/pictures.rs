//! Pictures: picked from a file, shaped into the square JPEG the server keeps (the website does the same), and shown
//! round (profile pictures) or square (playlists).
use std::path::Path;

/// A picture from the server, cut to a circle (the corners are made see-through), ready to show.
pub fn round_handle(bytes: &[u8]) -> Option<iced::widget::image::Handle> {
    let img = ::image::load_from_memory(bytes).ok()?.resize_to_fill(256, 256, ::image::imageops::FilterType::Lanczos3).to_rgba8();
    let (w, h) = img.dimensions();
    let mut raw = img.into_raw();
    let r = w as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            let coverage = (r - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4 + 3) as usize;
            raw[i] = (raw[i] as f32 * coverage) as u8;
        }
    }
    Some(iced::widget::image::Handle::from_rgba(w, h, raw))
}

/// Album art ready to show: shrunk to at most `max` px (a few hundred KB each, not the full picture) with its corners
/// rounded, so covers look like the website's. Done off the main thread; the answer is (width, height, RGBA bytes).
pub fn cover_rgba(bytes: &[u8], max: u32) -> Option<(u32, u32, Vec<u8>)> {
    let img = ::image::load_from_memory(bytes).ok()?;
    let k = (max as f32 / img.width().max(img.height()) as f32).min(1.0);
    let (w, h) = (((img.width() as f32 * k) as u32).max(1), ((img.height() as f32 * k) as u32).max(1));
    let mut px = if k < 1.0 { img.resize_exact(w, h, ::image::imageops::FilterType::Triangle).to_rgba8() } else { img.to_rgba8() };
    let r = w.min(h) as f32 * 0.045;
    let (hw, hh) = (w as f32 / 2.0, h as f32 / 2.0);
    let raw = px.as_mut();
    // only the corners need touching
    let reach = r.ceil() as u32 + 1;
    for y in (0..h).filter(|y| *y < reach || *y + reach >= h) {
        for x in (0..w).filter(|x| *x < reach || *x + reach >= w) {
            let (qx, qy) = (((x as f32 + 0.5) - hw).abs() - (hw - r), ((y as f32 + 0.5) - hh).abs() - (hh - r));
            let d = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt() + qx.max(qy).min(0.0) - r;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover < 1.0 {
                let i = ((y * w + x) * 4 + 3) as usize;
                raw[i] = (raw[i] as f32 * cover) as u8;
            }
        }
    }
    Some((w, h, px.into_raw()))
}

/// A chosen picture (a playlist's, or an album's that had no art) with its corners rounded, ready to show.
pub fn rounded_handle(bytes: &[u8]) -> Option<iced::widget::image::Handle> {
    let img = ::image::load_from_memory(bytes).ok()?;
    let (w, h) = (img.width().min(1024), img.height().min(1024));
    let mut px = if (w, h) == (img.width(), img.height()) { img.to_rgba8() } else { img.resize(w, h, ::image::imageops::FilterType::Triangle).to_rgba8() };
    let (w, h) = px.dimensions();
    let r = w.min(h) as f32 * 0.045;
    let (hw, hh) = (w as f32 / 2.0, h as f32 / 2.0);
    let raw = px.as_mut();
    for y in 0..h {
        for x in 0..w {
            let (qx, qy) = (((x as f32 + 0.5) - hw).abs() - (hw - r), ((y as f32 + 0.5) - hh).abs() - (hh - r));
            let d = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt() + qx.max(qy).min(0.0) - r;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            if cover < 1.0 {
                let i = ((y * w + x) * 4 + 3) as usize;
                raw[i] = (raw[i] as f32 * cover) as u8;
            }
        }
    }
    Some(iced::widget::image::Handle::from_rgba(w, h, px.into_raw()))
}

/// How the picture being adjusted will be shown, so the preview matches it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// album and playlist art: square with rounded corners
    Rounded,
    /// a profile picture
    Circle,
}

/// A picture file opened for adjusting (shrunk a little so moving and zooming it stays quick).
pub fn load(path: &Path) -> Result<::image::DynamicImage, String> {
    let img = ::image::open(path).map_err(|e| format!("Couldn't read that picture ({e})."))?;
    let (w, h) = (img.width(), img.height());
    let big = w.max(h);
    if big > 1800 {
        let k = 1800.0 / big as f32;
        return Ok(img.resize(((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1), ::image::imageops::FilterType::Triangle));
    }
    Ok(img)
}

/// The square of the picture centred on (cx, cy) with sides of `side` pixels, at `out` px.
fn cut(img: &::image::DynamicImage, cx: f32, cy: f32, side: f32, out: u32, filter: ::image::imageops::FilterType) -> ::image::RgbaImage {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let side = side.clamp(1.0, w.min(h));
    let x0 = (cx - side / 2.0).clamp(0.0, w - side);
    let y0 = (cy - side / 2.0).clamp(0.0, h - side);
    let part = ::image::imageops::crop_imm(img, x0 as u32, y0 as u32, side as u32, side as u32).to_image();
    ::image::imageops::resize(&part, out, out, filter)
}

/// What the adjust dialog shows: the chosen square, with the shape it will have (the corners / outside the circle see-through).
pub fn preview(img: &::image::DynamicImage, cx: f32, cy: f32, side: f32, out: u32, shape: Shape) -> iced::widget::image::Handle {
    let mut px = cut(img, cx, cy, side, out, ::image::imageops::FilterType::Triangle);
    let (w, h) = px.dimensions();
    let r = match shape {
        Shape::Circle => w as f32 / 2.0,
        Shape::Rounded => w as f32 * 0.047,
    };
    let half = w as f32 / 2.0;
    let raw = px.as_mut();
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // signed distance to the rounded square (negative inside), then a one-pixel soft edge
            let (qx, qy) = ((fx - half).abs() - (half - r), (fy - half).abs() - (half - r));
            let d = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt() + qx.max(qy).min(0.0) - r;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4 + 3) as usize;
            raw[i] = (raw[i] as f32 * cover) as u8;
        }
    }
    iced::widget::image::Handle::from_rgba(w, h, px.into_raw())
}

/// The chosen square as the JPEG the server keeps.
pub fn cut_jpeg(img: &::image::DynamicImage, cx: f32, cy: f32, side: f32, out: u32) -> Result<Vec<u8>, String> {
    let sq = ::image::DynamicImage::ImageRgba8(cut(img, cx, cy, side, out, ::image::imageops::FilterType::Lanczos3)).to_rgb8();
    let mut bytes = Vec::new();
    ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90)
        .encode(sq.as_raw(), out, out, ::image::ColorType::Rgb8)
        .map_err(|e| format!("Couldn't prepare that picture ({e})."))?;
    if bytes.len() > 900_000 {
        return Err("That picture is too big.".into());
    }
    Ok(bytes)
}
