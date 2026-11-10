//! Guild emblem BMP encode/decode.
//!
//! Hercules `grfio_decode_zip` is zlib (`uncompress`), not gzip. A valid emblem
//! is a 24×24 24-bit bottom-up BMP. Magenta (`r >= 0xF8`, `g <= 0x07`,
//! `b >= 0xF8`) is transparent.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

pub const EMBLEM_SIZE: u32 = 24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedEmblem {
    pub width: u32,
    pub height: u32,
    /// Top-down RGBA.
    pub pixels: Vec<u8>,
}

pub fn zlib_compress(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).expect("zlib encode");
    encoder.finish().expect("zlib finish")
}

pub fn zlib_decompress(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).ok()?;
    if output.is_empty() { None } else { Some(output) }
}

fn is_magenta(red: u8, green: u8, blue: u8) -> bool {
    red >= 0xF8 && green <= 0x07 && blue >= 0xF8
}

/// Encode top-down RGBA as a 24-bit bottom-up BMP. Width and height must be
/// positive. Rows are padded to 4 bytes.
pub fn encode_bmp_24(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = (width * 3).div_ceil(4) * 4;
    let pixel_bytes = stride * height;
    let file_size = 54 + pixel_bytes;
    let mut bmp = vec![0u8; file_size as usize];
    bmp[0] = b'B';
    bmp[1] = b'M';
    bmp[2..6].copy_from_slice(&file_size.to_le_bytes());
    bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&width.to_le_bytes());
    bmp[22..26].copy_from_slice(&height.to_le_bytes());
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
    let pixels = width as usize * height as usize;
    for index in 0..pixels {
        let source = index * 4;
        if source + 3 >= rgba.len() {
            break;
        }
        let x = (index as u32) % width;
        let y = (index as u32) / width;
        let bottom_up_y = height - 1 - y;
        let offset = 54 + (bottom_up_y * stride + x * 3) as usize;
        bmp[offset] = rgba[source + 2];
        bmp[offset + 1] = rgba[source + 1];
        bmp[offset + 2] = rgba[source];
    }
    bmp
}

pub fn decode_emblem(bytes: &[u8]) -> Option<DecodedEmblem> {
    if bytes.len() >= 2 && bytes[0] == b'B' && bytes[1] == b'M' {
        return decode_bmp(bytes);
    }
    let inflated = zlib_decompress(bytes)?;
    decode_bmp(&inflated)
}

fn decode_bmp(bytes: &[u8]) -> Option<DecodedEmblem> {
    if bytes.len() < 54 || bytes[0] != b'B' || bytes[1] != b'M' {
        return None;
    }
    let offbits = u32::from_le_bytes(bytes[10..14].try_into().ok()?) as usize;
    let width = i32::from_le_bytes(bytes[18..22].try_into().ok()?);
    let height_raw = i32::from_le_bytes(bytes[22..26].try_into().ok()?);
    let bit_count = u16::from_le_bytes(bytes[28..30].try_into().ok()?);
    if width <= 0 || height_raw == 0 || offbits >= bytes.len() {
        return None;
    }
    let top_down = height_raw < 0;
    let height = height_raw.unsigned_abs();
    let width = width as u32;
    let mut pixels = vec![0u8; (width * height) as usize * 4];
    match bit_count {
        24 => decode_24(bytes, offbits, width, height, top_down, &mut pixels)?,
        8 => decode_8(bytes, offbits, width, height, top_down, &mut pixels)?,
        _ => return None,
    }
    Some(DecodedEmblem { width, height, pixels })
}

fn pixel_index(x: u32, y: u32, width: u32, height: u32, top_down: bool) -> usize {
    let row = if top_down { y } else { height - 1 - y };
    (row * width + x) as usize * 4
}

fn decode_24(bytes: &[u8], offbits: usize, width: u32, height: u32, top_down: bool, pixels: &mut [u8]) -> Option<()> {
    let stride = (width * 3).div_ceil(4) as usize * 4;
    for y in 0..height {
        for x in 0..width {
            let source = offbits + y as usize * stride + x as usize * 3;
            if source + 2 >= bytes.len() {
                return None;
            }
            let blue = bytes[source];
            let green = bytes[source + 1];
            let red = bytes[source + 2];
            let index = pixel_index(x, y, width, height, top_down);
            pixels[index] = red;
            pixels[index + 1] = green;
            pixels[index + 2] = blue;
            pixels[index + 3] = if is_magenta(red, green, blue) { 0 } else { 255 };
        }
    }
    Some(())
}

fn decode_8(bytes: &[u8], offbits: usize, width: u32, height: u32, top_down: bool, pixels: &mut [u8]) -> Option<()> {
    let mut colors = u32::from_le_bytes(bytes.get(46..50)?.try_into().ok()?);
    if colors == 0 {
        colors = 256;
    }
    if colors > 256 {
        return None;
    }
    let palette = 54;
    let stride = width.div_ceil(4) as usize * 4;
    for y in 0..height {
        for x in 0..width {
            let source = offbits + y as usize * stride + x as usize;
            let index_byte = *bytes.get(source)?;
            if u32::from(index_byte) >= colors {
                return None;
            }
            let entry = palette + index_byte as usize * 4;
            let blue = *bytes.get(entry)?;
            let green = *bytes.get(entry + 1)?;
            let red = *bytes.get(entry + 2)?;
            let index = pixel_index(x, y, width, height, top_down);
            pixels[index] = red;
            pixels[index + 1] = green;
            pixels[index + 2] = blue;
            pixels[index + 3] = if is_magenta(red, green, blue) { 0 } else { 255 };
        }
    }
    Some(())
}

/// Normalize an image file to a zlib-compressed 24×24 BMP for `0x0153`.
pub fn prepare_upload(file_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(file_bytes).map_err(|error| error.to_string())?;
    let image = image
        .resize_exact(EMBLEM_SIZE, EMBLEM_SIZE, image::imageops::FilterType::Nearest)
        .to_rgba8();
    let bmp = encode_bmp_24(EMBLEM_SIZE, EMBLEM_SIZE, image.as_raw());
    Ok(zlib_compress(&bmp))
}

pub fn load_upload_file() -> Result<Vec<u8>, String> {
    for name in ["guild-emblem.bmp", "guild-emblem.png", "emblem.bmp", "emblem.png"] {
        if let Ok(bytes) = std::fs::read(name) {
            return prepare_upload(&bytes);
        }
    }
    Err("Place guild-emblem.bmp (or guild-emblem.png) in the working directory to upload.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_up_bmp_round_trip_keeps_the_top_row_and_hides_magenta() {
        let rgba = [
            255, 0, 0, 255, // top-left red
            255, 0, 255, 255, // top-right magenta
            0, 0, 255, 255, // bottom-left blue
            0, 255, 0, 255, // bottom-right green
        ];
        let bmp = encode_bmp_24(2, 2, &rgba);
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 2);
        let decoded = decode_emblem(&bmp).unwrap();
        assert_eq!(decoded.width, 2);
        assert_eq!(&decoded.pixels[0..4], &[255, 0, 0, 255]);
        assert_eq!(&decoded.pixels[4..8], &[255, 0, 255, 0]);
        assert_eq!(&decoded.pixels[8..12], &[0, 0, 255, 255]);
        let compressed = zlib_compress(&bmp);
        assert_ne!(&compressed[0..2], b"BM");
        let inflated = decode_emblem(&compressed).unwrap();
        assert_eq!(inflated.pixels, decoded.pixels);
    }

    #[test]
    fn reduced_magenta_is_transparent() {
        let mut rgba = vec![0u8; 4];
        rgba[0] = 0xF8;
        rgba[1] = 0x07;
        rgba[2] = 0xF8;
        rgba[3] = 255;
        let decoded = decode_emblem(&encode_bmp_24(1, 1, &rgba)).unwrap();
        assert_eq!(decoded.pixels[3], 0);
    }

    #[test]
    fn prepare_upload_emits_a_24_square_zlib_bmp() {
        let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 255, 255]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let compressed = prepare_upload(&png).unwrap();
        let decoded = decode_emblem(&compressed).unwrap();
        assert_eq!((decoded.width, decoded.height), (24, 24));
        assert_eq!(decoded.pixels[3], 0);
    }
}
