//! oas-codec: PNG <-> Frame conversion (pure Rust `png` crate: no C deps, so it
//! also builds for Android).

use std::io::Cursor;

use oas_types::Frame;

/// Decode a PNG into an RGBA frame.
///
/// `screencap -p` produces RGB8 or RGBA8; both are handled, plus grayscale.
/// Indexed PNGs are rejected loudly instead of being silently mis-decoded.
pub fn decode_png(bytes: &[u8]) -> Result<Frame, String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("png read_info: {e}"))?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("png next_frame: {e}"))?;
    let data = &buf[..info.buffer_size()];
    let (w, h) = (info.width, info.height);
    let n = (w as usize) * (h as usize);
    let mut rgba = vec![0u8; n * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(data),
        png::ColorType::Rgb => {
            for i in 0..n {
                rgba[i * 4] = data[i * 3];
                rgba[i * 4 + 1] = data[i * 3 + 1];
                rgba[i * 4 + 2] = data[i * 3 + 2];
                rgba[i * 4 + 3] = 255;
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..n {
                let v = data[i];
                rgba[i * 4] = v;
                rgba[i * 4 + 1] = v;
                rgba[i * 4 + 2] = v;
                rgba[i * 4 + 3] = 255;
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..n {
                let v = data[i * 2];
                rgba[i * 4] = v;
                rgba[i * 4 + 1] = v;
                rgba[i * 4 + 2] = v;
                rgba[i * 4 + 3] = data[i * 2 + 1];
            }
        }
        png::ColorType::Indexed => {
            return Err("indexed PNG is not supported (would be mis-decoded)".into())
        }
    }
    Ok(Frame::new(w, h, rgba))
}

/// Encode an RGBA frame as PNG.
pub fn encode_png(frame: &Frame) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, frame.width, frame.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
        writer
            .write_image_data(&frame.rgba)
            .map_err(|e| format!("png data: {e}"))?;
    }
    Ok(out)
}

/// Stable content hash for frame comparisons (FNV-1a 64; not crypto, just
/// equality/ordering of identical buffers).
pub fn frame_hash(frame: &Frame) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in &frame.rgba {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_roundtrip_preserves_pixels() {
        let f = Frame::new(4, 3, (0..(4 * 3 * 4)).map(|i| i as u8).collect());
        let png = encode_png(&f).expect("encode");
        let back = decode_png(&png).expect("decode");
        assert_eq!(back.width, 4);
        assert_eq!(back.height, 3);
        assert_eq!(back.rgba, f.rgba);
        assert_eq!(frame_hash(&back), frame_hash(&f));
    }

    #[test]
    fn garbage_is_rejected_loudly() {
        assert!(decode_png(b"not a png").is_err());
    }
}
