//! Pictures off the web, decoded into pixels a canvas can take.
//!
//! Three formats are read — PNG, JPEG and WebP — which between them are what
//! a page's pictures are almost always in. Neither an animated GIF nor a
//! vector drawing is one of them, and neither is fetched: a page that has
//! only those is a page with no picture on it, which is honest.

/// A picture as pixels: red, green, blue and alpha, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    /// `w * h * 4` bytes, the rows one after another.
    pub rgba: Vec<u8>,
}

impl Picture {
    /// The pixel at (`x`, `y`), if the picture has one.
    pub fn at(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.w || y >= self.h {
            return None;
        }
        let at = ((y * self.w + x) * 4) as usize;
        let px = self.rgba.get(at..at + 4)?;
        Some([px[0], px[1], px[2], px[3]])
    }
}

/// What a picture is, told by the bytes themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Png,
    Jpeg,
    Webp,
    /// Something this build does not draw.
    Other,
}

/// The format of `bytes`, read from its first few rather than from what the
/// server called it: servers get this wrong, the bytes less often.
pub fn kind_of(bytes: &[u8]) -> Kind {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.starts_with(PNG) {
        return Kind::Png;
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Kind::Jpeg;
    }
    // "RIFF" <size:4> "WEBP"
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Kind::Webp;
    }
    Kind::Other
}

/// How large a picture may be before it is left out: a page with a 30
/// megapixel photograph on it would cost more memory than the recovery has.
const MOST_PIXELS: u64 = 16_000_000;

/// The picture `bytes` holds, or why it could not be read.
pub fn decode(bytes: &[u8]) -> Result<Picture, String> {
    let picture = match kind_of(bytes) {
        Kind::Png => png(bytes),
        Kind::Jpeg => jpeg(bytes),
        Kind::Webp => webp(bytes),
        Kind::Other => return Err("not a picture this browser draws".to_string()),
    }?;
    if picture.w as u64 * picture.h as u64 > MOST_PIXELS {
        return Err(format!("a picture of {} by {} is too large to show", picture.w, picture.h));
    }
    Ok(picture)
}

fn png(bytes: &[u8]) -> Result<Picture, String> {
    let mut decoder = png::Decoder::new(bytes);
    // Palettes and greys arrive as RGB or RGBA, which is what a canvas takes.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("png: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("png: {e}"))?;
    let (w, h) = (info.width, info.height);
    let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
    let px = &buf[..info.buffer_size()];
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(px),
        png::ColorType::Rgb => {
            for rgb in px.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for ga in px.as_chunks::<2>().0 {
                rgba.extend_from_slice(&[ga[0], ga[0], ga[0], ga[1]]);
            }
        }
        png::ColorType::Grayscale => {
            for &g in px {
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
        }
        png::ColorType::Indexed => return Err("png: an indexed picture came through unexpanded".to_string()),
    }
    Ok(Picture { w, h, rgba })
}

fn jpeg(bytes: &[u8]) -> Result<Picture, String> {
    let mut decoder = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(bytes));
    let pixels = decoder.decode().map_err(|e| format!("jpeg: {e:?}"))?;
    let (w, h) = decoder.dimensions().ok_or_else(|| "jpeg: no size".to_string())?;
    let (w, h) = (w as u32, h as u32);
    let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
    match decoder.output_colorspace().map(|c| c.num_components()) {
        Some(1) => {
            for &g in &pixels {
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
        }
        _ => {
            for rgb in pixels.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    Ok(Picture { w, h, rgba })
}

fn webp(bytes: &[u8]) -> Result<Picture, String> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).map_err(|e| format!("webp: {e}"))?;
    let (w, h) = decoder.dimensions();
    let mut rgba = vec![0; (w as usize) * (h as usize) * 4];
    decoder.read_image(&mut rgba).map_err(|e| format!("webp: {e}"))?;
    Ok(Picture { w, h, rgba })
}

/// `picture` drawn into a `w` by `h` box at (`x`, `y`), scaled to fit and
/// centred, its transparent parts letting what is under them through.
///
/// Returns the box it actually covered, which is nothing where the picture
/// falls outside the canvas.
pub fn fitted(picture: &Picture, box_w: f32, box_h: f32) -> (f32, f32) {
    if picture.w == 0 || picture.h == 0 || box_w <= 0.0 || box_h <= 0.0 {
        return (0.0, 0.0);
    }
    let scale = (box_w / picture.w as f32).min(box_h / picture.h as f32);
    ((picture.w as f32 * scale).round(), (picture.h as f32 * scale).round())
}

/// The pixel of `picture` at the scaled point (`u`, `v`) of a drawing `w` by
/// `h` pixels, found by taking the nearest source pixel.
pub fn sample(picture: &Picture, u: u32, v: u32, w: u32, h: u32) -> Option<[u8; 4]> {
    if w == 0 || h == 0 {
        return None;
    }
    let x = (u as u64 * picture.w as u64 / w as u64) as u32;
    let y = (v as u64 * picture.h as u64 / h as u64) as u32;
    picture.at(x.min(picture.w.saturating_sub(1)), y.min(picture.h.saturating_sub(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG of `w` by `h` pixels, every one of them `rgba`.
    fn a_png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("header");
            let px: Vec<u8> = std::iter::repeat_n(rgba, (w * h) as usize).flatten().collect();
            writer.write_image_data(&px).expect("data");
            writer.finish().expect("finish");
        }
        out
    }

    #[test]
    fn a_picture_is_told_by_its_bytes() {
        assert_eq!(kind_of(&a_png(1, 1, [1, 2, 3, 255])), Kind::Png);
        assert_eq!(kind_of(&[0xff, 0xd8, 0xff, 0xe0, 0, 0]), Kind::Jpeg);
        assert_eq!(kind_of(b"RIFF\0\0\0\0WEBPVP8 "), Kind::Webp);
        assert_eq!(kind_of(b"GIF89a"), Kind::Other);
        assert_eq!(kind_of(b"<html>"), Kind::Other);
        assert_eq!(kind_of(&[]), Kind::Other);
    }

    #[test]
    fn a_png_is_read_into_pixels() {
        let png = a_png(3, 2, [10, 20, 30, 255]);
        let picture = decode(&png).expect("a picture");
        assert_eq!((picture.w, picture.h), (3, 2));
        assert_eq!(picture.rgba.len(), 3 * 2 * 4);
        assert_eq!(picture.at(0, 0), Some([10, 20, 30, 255]));
        assert_eq!(picture.at(2, 1), Some([10, 20, 30, 255]));
        assert_eq!(picture.at(3, 0), None, "past its width there is nothing");
    }

    #[test]
    fn what_is_not_a_picture_is_turned_down_rather_than_guessed_at() {
        assert!(decode(b"<html>not a picture</html>").is_err());
        assert!(decode(b"GIF89a....").is_err());
        assert!(decode(&[]).is_err());
    }

    #[test]
    fn a_picture_is_scaled_to_fit_its_box_and_keeps_its_shape() {
        let picture = Picture { w: 100, h: 50, rgba: vec![0; 100 * 50 * 4] };
        // In a wide box it fills the height.
        assert_eq!(fitted(&picture, 400.0, 100.0), (200.0, 100.0));
        // In a tall one it fills the width.
        assert_eq!(fitted(&picture, 200.0, 400.0), (200.0, 100.0));
        // A box smaller than it shrinks it.
        assert_eq!(fitted(&picture, 50.0, 50.0), (50.0, 25.0));
        assert_eq!(fitted(&picture, 0.0, 100.0), (0.0, 0.0), "no box, no picture");
    }

    #[test]
    fn a_scaled_picture_takes_the_nearest_pixel_of_its_own() {
        // Two pixels wide, one pixel each of black then white.
        let mut rgba = vec![0, 0, 0, 255];
        rgba.extend_from_slice(&[255, 255, 255, 255]);
        let picture = Picture { w: 2, h: 1, rgba };
        assert_eq!(sample(&picture, 0, 0, 2, 1), Some([0, 0, 0, 255]));
        assert_eq!(sample(&picture, 1, 0, 2, 1), Some([255, 255, 255, 255]));
        // Drawn twice as wide, the left half is the left pixel.
        assert_eq!(sample(&picture, 0, 0, 4, 1), Some([0, 0, 0, 255]));
        assert_eq!(sample(&picture, 2, 0, 4, 1), Some([255, 255, 255, 255]));
    }
}
