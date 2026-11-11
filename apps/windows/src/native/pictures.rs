//! Pictures: picked from a file, shaped into the square JPEG the server keeps (the website does the same), and shown
//! round (profile pictures) or square (playlists).
use std::path::Path;

/// A picture file as the square JPEG to send for a profile picture: the middle of the picture, 256 px.
pub fn prepare(path: &Path) -> Result<Vec<u8>, String> {
    prepare_square(path, 256)
}

/// The same at any size (a playlist's picture is 512 px).
pub fn prepare_square(path: &Path, size: u32) -> Result<Vec<u8>, String> {
    let img = ::image::open(path).map_err(|e| format!("Couldn't read that picture ({e})."))?;
    let square = img.resize_to_fill(size, size, ::image::imageops::FilterType::Lanczos3).to_rgb8();
    let mut out = Vec::new();
    ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode(square.as_raw(), size, size, ::image::ColorType::Rgb8)
        .map_err(|e| format!("Couldn't prepare that picture ({e})."))?;
    if out.len() > 900_000 {
        return Err("That picture is too big.".into());
    }
    Ok(out)
}

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
