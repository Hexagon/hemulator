//! Parser for the .mgw ROM container format (LCD-Game-Shrinker / gw-libretro).
//!
//! The .mgw format packages an SM510-family CPU program ROM together with
//! LCD segment artwork, background image, keyboard mapping, and melody data.
//!
//! ## File structure
//!
//! The file may be compressed (LZ4, ZLIB, or LZMA). After decompression:
//!
//! | Offset | Size  | Field                       |
//! |--------|-------|-----------------------------|
//! | 0x00   | 8     | CPU name (e.g. "SM510\0\0") |
//! | 0x08   | 8     | ROM signature               |
//! | 0x10   | 7     | Time address fields + PM    |
//! | 0x17   | 1     | Spare byte                  |
//! | 0x18   | 4     | Flags (u32 LE)              |
//! | 0x1C   | 80    | 10 data section descriptors |
//! | 0x6C   | ...   | Data sections               |
//!
//! Each data section descriptor is (offset: u32, size: u32).
//! Sections: background, segments_pixel, segments_offset, segments_x,
//! segments_y, segments_height, segments_width, melody, program, keyboard.

use thiserror::Error;

/// The native display resolution for .mgw files (matches gw-libretro).
pub const MGW_SCREEN_WIDTH: u32 = 320;
pub const MGW_SCREEN_HEIGHT: u32 = 240;

/// Maximum number of LCD segments.
pub const MAX_SEGMENTS: usize = 256;

/// LZ4 frame magic bytes.
const LZ4_FRAME_MAGIC: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];

/// Header flag bits.
const FLAG_RENDERING_LCD_INVERTED: u32 = 0x01;
const FLAG_SEGMENTS_4BITS: u32 = 0x10;
const FLAG_SEGMENTS_2BITS: u32 = 0x100;

/// Sound mode flags (bits 1-3).
const FLAG_SOUND_MASK: u32 = 0x0E;

#[derive(Error, Debug)]
pub enum MgwError {
    #[error("File too small ({0} bytes)")]
    FileTooSmall(usize),

    #[error("Unknown compression format")]
    UnknownFormat,

    #[error("Decompression failed: {0}")]
    DecompressFailed(String),

    #[error("Invalid header: CPU name doesn't start with SM5")]
    InvalidHeader,

    #[error(
        "Section out of bounds: {section} at offset {offset} size {size}, data len {data_len}"
    )]
    SectionOutOfBounds {
        section: &'static str,
        offset: u32,
        size: u32,
        data_len: usize,
    },
}

/// A single LCD segment's artwork and position.
#[derive(Clone)]
pub struct MgwSegment {
    /// X position on the 320×240 screen.
    pub x: u16,
    /// Y position on the 320×240 screen.
    pub y: u16,
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Grayscale pixel data (8-bit per pixel, 0=transparent/background, 255=opaque segment).
    pub pixels: Vec<u8>,
}

/// Parsed .mgw ROM data.
pub struct MgwRom {
    /// CPU type string (e.g. "SM510", "SM511", "SM5A").
    pub cpu_type: String,
    /// ROM signature.
    pub signature: String,
    /// Header flags.
    pub flags: u32,
    /// Whether LCD rendering is inverted (tabletop/panorama).
    pub lcd_inverted: bool,
    /// Sound mode (from flag bits 1-3).
    pub sound_mode: u8,
    /// Background image as ARGB8888 pixels (320×240), or empty if none.
    pub background: Vec<u32>,
    /// LCD segments (up to 256).
    pub segments: Vec<Option<MgwSegment>>,
    /// CPU program ROM bytes.
    pub program: Vec<u8>,
    /// Melody ROM bytes (may be empty).
    pub melody: Vec<u8>,
    /// Keyboard mapping: 10 × u32 entries.
    /// keyboard[0..8] = S1..S8 (K4..K1 bits per button)
    /// keyboard[8] = BA, keyboard[9] = B
    pub keyboard: [u32; 10],
}

/// Read a little-endian u32 from a byte slice at the given offset.
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    if offset + 4 > data.len() {
        return 0;
    }
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// Read a little-endian u16 from a byte slice at the given offset.
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    if offset + 2 > data.len() {
        return 0;
    }
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

/// Extract a section slice from the decompressed ROM data.
fn get_section<'a>(
    data: &'a [u8],
    offset: u32,
    size: u32,
    name: &'static str,
) -> Result<&'a [u8], MgwError> {
    let o = offset as usize;
    let s = size as usize;
    if s == 0 {
        return Ok(&[]);
    }
    if o + s > data.len() {
        return Err(MgwError::SectionOutOfBounds {
            section: name,
            offset,
            size,
            data_len: data.len(),
        });
    }
    Ok(&data[o..o + s])
}

/// Decompress the .mgw file data.
///
/// Returns the decompressed ROM data (header + sections).
fn decompress(data: &[u8]) -> Result<Vec<u8>, MgwError> {
    if data.len() < 8 {
        return Err(MgwError::FileTooSmall(data.len()));
    }

    // Check if uncompressed (starts with "SM5")
    if data.len() >= 3 && &data[0..3] == b"SM5" {
        return Ok(data.to_vec());
    }

    // Check for LZ4 frame magic
    if data.len() >= 4 && data[0..4] == LZ4_FRAME_MAGIC {
        return decompress_lz4(data);
    }

    // Check for ZLIB header ("ZLIB" + 4-byte compressed size)
    if data.len() >= 8 && &data[0..4] == b"ZLIB" {
        let compressed_size = read_u32_le(data, 4) as usize;
        let compressed = &data[8..8 + compressed_size.min(data.len() - 8)];
        return decompress_zlib(compressed);
    }

    // Check for LZMA header ("LZMA" + 4-byte compressed size)
    if data.len() >= 8 && &data[0..4] == b"LZMA" {
        // LZMA decompression not implemented — fall through to error
        return Err(MgwError::DecompressFailed(
            "LZMA compression not supported".to_string(),
        ));
    }

    Err(MgwError::UnknownFormat)
}

/// Decompress LZ4 frame data.
fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>, MgwError> {
    // lz4_flex's decompress_size_prepended expects (uncompressed_size: u32 LE, compressed_data).
    // But LZ4 frame format is different. We need to use the frame decoder.
    // lz4_flex provides frame decompression.
    use lz4_flex::frame::FrameDecoder;
    use std::io::Read;

    let mut decoder = FrameDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| MgwError::DecompressFailed(format!("LZ4: {}", e)))?;
    Ok(decompressed)
}

/// Decompress ZLIB/DEFLATE data.
fn decompress_zlib(compressed: &[u8]) -> Result<Vec<u8>, MgwError> {
    // The LCD-Game-Shrinker uses raw DEFLATE (wbits=-15), not zlib wrapper.
    let decompressed = miniz_oxide::inflate::decompress_to_vec(compressed)
        .map_err(|e| MgwError::DecompressFailed(format!("ZLIB: {:?}", e)))?;
    Ok(decompressed)
}

/// Unpack 4-bit packed segment pixels into 8-bit grayscale.
/// Each byte contains 2 pixels: high nibble first, low nibble second.
/// Each nibble is scaled from 0..15 to 0..255.
fn unpack_4bit_pixels(packed: &[u8], total_pixels: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(total_pixels);
    for &byte in packed {
        let hi = (byte >> 4) & 0x0F;
        let lo = byte & 0x0F;
        out.push(hi * 17); // Scale 0..15 to 0..255
        out.push(lo * 17);
    }
    out.truncate(total_pixels);
    out
}

/// Unpack 2-bit packed segment pixels into 8-bit grayscale.
/// Each byte contains 4 pixels (from MSB to LSB, 2 bits each).
/// Each 2-bit value is scaled from 0..3 to 0..255.
fn unpack_2bit_pixels(packed: &[u8], total_pixels: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(total_pixels);
    for &byte in packed {
        out.push(((byte >> 6) & 0x03) * 85);
        out.push(((byte >> 4) & 0x03) * 85);
        out.push(((byte >> 2) & 0x03) * 85);
        out.push((byte & 0x03) * 85);
    }
    out.truncate(total_pixels);
    out
}

/// Convert RGB565 pixel (little-endian u16) to ARGB8888.
fn rgb565_to_argb(pixel: u16) -> u32 {
    let r = ((pixel >> 11) & 0x1F) as u32;
    let g = ((pixel >> 5) & 0x3F) as u32;
    let b = (pixel & 0x1F) as u32;

    let r8 = (r * 255 + 15) / 31;
    let g8 = (g * 255 + 31) / 63;
    let b8 = (b * 255 + 15) / 31;

    0xFF000000 | (r8 << 16) | (g8 << 8) | b8
}

/// Parse a .mgw file into its components.
pub fn parse_mgw(file_data: &[u8]) -> Result<MgwRom, MgwError> {
    let data = decompress(file_data)?;

    // Minimum header size: 8 (cpu) + 8 (sig) + 7 (time) + 1 (spare) + 4 (flags) + 80 (sections) = 108
    if data.len() < 108 {
        return Err(MgwError::FileTooSmall(data.len()));
    }

    // Validate header: first 3 bytes must be "SM5"
    if &data[0..3] != b"SM5" {
        return Err(MgwError::InvalidHeader);
    }

    // Parse CPU name (8 bytes at offset 0)
    let cpu_name_bytes = &data[0..8];
    let cpu_type = String::from_utf8_lossy(cpu_name_bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string();

    // ROM signature (8 bytes at offset 8)
    let sig_bytes = &data[8..16];
    let signature = String::from_utf8_lossy(sig_bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string();

    // Flags (4 bytes at offset 0x18)
    let flags = read_u32_le(&data, 0x18);
    let lcd_inverted = (flags & FLAG_RENDERING_LCD_INVERTED) != 0;
    let sound_mode = ((flags & FLAG_SOUND_MASK) >> 1) as u8;
    let segments_4bit = (flags & FLAG_SEGMENTS_4BITS) != 0;
    let segments_2bit = (flags & FLAG_SEGMENTS_2BITS) != 0;

    // Data section descriptors start at offset 0x1C
    // 10 sections × 2 fields (offset, size) × 4 bytes = 80 bytes
    let sec_base = 0x1C;

    let bg_offset = read_u32_le(&data, sec_base);
    let bg_size = read_u32_le(&data, sec_base + 4);

    let seg_pixel_offset = read_u32_le(&data, sec_base + 8);
    let seg_pixel_size = read_u32_le(&data, sec_base + 12);

    let seg_offset_offset = read_u32_le(&data, sec_base + 16);
    let seg_offset_size = read_u32_le(&data, sec_base + 20);

    let seg_x_offset = read_u32_le(&data, sec_base + 24);
    let seg_x_size = read_u32_le(&data, sec_base + 28);

    let seg_y_offset = read_u32_le(&data, sec_base + 32);
    let seg_y_size = read_u32_le(&data, sec_base + 36);

    let seg_h_offset = read_u32_le(&data, sec_base + 40);
    let seg_h_size = read_u32_le(&data, sec_base + 44);

    let seg_w_offset = read_u32_le(&data, sec_base + 48);
    let seg_w_size = read_u32_le(&data, sec_base + 52);

    let mel_offset = read_u32_le(&data, sec_base + 56);
    let mel_size = read_u32_le(&data, sec_base + 60);

    let prg_offset = read_u32_le(&data, sec_base + 64);
    let prg_size = read_u32_le(&data, sec_base + 68);

    let kbd_offset = read_u32_le(&data, sec_base + 72);
    let kbd_size = read_u32_le(&data, sec_base + 76);

    // Extract data sections
    let bg_data = get_section(&data, bg_offset, bg_size, "background")?;
    let seg_pixels_raw = get_section(&data, seg_pixel_offset, seg_pixel_size, "segments_pixel")?;
    let seg_offsets_data =
        get_section(&data, seg_offset_offset, seg_offset_size, "segments_offset")?;
    let seg_x_data = get_section(&data, seg_x_offset, seg_x_size, "segments_x")?;
    let seg_y_data = get_section(&data, seg_y_offset, seg_y_size, "segments_y")?;
    let seg_h_data = get_section(&data, seg_h_offset, seg_h_size, "segments_height")?;
    let seg_w_data = get_section(&data, seg_w_offset, seg_w_size, "segments_width")?;
    let mel_data = get_section(&data, mel_offset, mel_size, "melody")?;
    let prg_data = get_section(&data, prg_offset, prg_size, "program")?;
    let kbd_data = get_section(&data, kbd_offset, kbd_size, "keyboard")?;

    // --- Parse background (RGB565 → ARGB8888) ---
    let mut background = Vec::new();
    if bg_size > 0 {
        let npixels = (MGW_SCREEN_WIDTH * MGW_SCREEN_HEIGHT) as usize;
        background.reserve(npixels);
        for i in 0..npixels {
            let px_offset = i * 2;
            if px_offset + 1 < bg_data.len() {
                let rgb565 = u16::from_le_bytes([bg_data[px_offset], bg_data[px_offset + 1]]);
                background.push(rgb565_to_argb(rgb565));
            } else {
                background.push(0xFF000000); // Black
            }
        }
    }

    // --- Parse segment pixel data ---
    // Determine the raw 8-bit version of segment pixel data
    let seg_pixels_8bit: Vec<u8> = if segments_2bit {
        // Total pixel count = raw size as 8bpp
        // The packed data is 1/4 the size
        unpack_2bit_pixels(seg_pixels_raw, seg_pixels_raw.len() * 4)
    } else if segments_4bit {
        // The packed data is 1/2 the size
        unpack_4bit_pixels(seg_pixels_raw, seg_pixels_raw.len() * 2)
    } else {
        // Already 8-bit
        seg_pixels_raw.to_vec()
    };

    // --- Build segment array ---
    let num_seg_entries = seg_x_size as usize / 2; // Each entry is u16
    let num_segments = num_seg_entries.min(MAX_SEGMENTS);

    let mut segments: Vec<Option<MgwSegment>> = vec![None; MAX_SEGMENTS];

    #[allow(clippy::needless_range_loop)] // Segments are indexed by position, not iterated
    for i in 0..num_segments {
        let x = read_u16_le(seg_x_data, i * 2);
        let y = read_u16_le(seg_y_data, i * 2);
        let w = read_u16_le(seg_w_data, i * 2);
        let h = read_u16_le(seg_h_data, i * 2);

        // Skip empty segments
        if w == 0 || h == 0 {
            continue;
        }

        // Get offset into the pixel data for this segment
        let pixel_offset = if i * 4 < seg_offsets_data.len() {
            read_u32_le(seg_offsets_data, i * 4) as usize
        } else {
            continue;
        };

        let pixel_count = (w as usize) * (h as usize);
        let pixel_end = pixel_offset.min(seg_pixels_8bit.len());
        let pixel_start = pixel_offset.saturating_sub(pixel_count);

        // The offset in the segment offset table points to the END of this segment's data
        // (it's accumulated from the file building process)
        let actual_start = if pixel_end >= pixel_count {
            pixel_end - pixel_count
        } else {
            pixel_start
        };

        let pixels = if actual_start + pixel_count <= seg_pixels_8bit.len() {
            seg_pixels_8bit[actual_start..actual_start + pixel_count].to_vec()
        } else if actual_start < seg_pixels_8bit.len() {
            let mut px = seg_pixels_8bit[actual_start..].to_vec();
            px.resize(pixel_count, 0);
            px
        } else {
            vec![0; pixel_count]
        };

        segments[i] = Some(MgwSegment {
            x,
            y,
            width: w,
            height: h,
            pixels,
        });
    }

    // --- Parse keyboard mapping ---
    let mut keyboard = [0u32; 10];
    for (i, entry) in keyboard.iter_mut().enumerate() {
        if i * 4 + 3 < kbd_data.len() {
            *entry = read_u32_le(kbd_data, i * 4);
        }
    }

    Ok(MgwRom {
        cpu_type,
        signature,
        flags,
        lcd_inverted,
        sound_mode,
        background,
        segments,
        program: prg_data.to_vec(),
        melody: mel_data.to_vec(),
        keyboard,
    })
}

/// Detect whether the data is likely an .mgw container file.
///
/// Returns true if the data starts with known .mgw magic bytes
/// (SM5x header, LZ4 frame, ZLIB header, or LZMA header).
pub fn is_mgw_format(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    // Uncompressed: starts with "SM5"
    if &data[0..3] == b"SM5" {
        return true;
    }
    // LZ4 frame
    if data[0..4] == LZ4_FRAME_MAGIC {
        return true;
    }
    // ZLIB or LZMA
    if &data[0..4] == b"ZLIB" || &data[0..4] == b"LZMA" {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb565_to_argb() {
        // White (0xFFFF) = R:31, G:63, B:31 → 0xFFFFFFFF
        assert_eq!(rgb565_to_argb(0xFFFF), 0xFFFFFFFF);
        // Black (0x0000) → 0xFF000000
        assert_eq!(rgb565_to_argb(0x0000), 0xFF000000);
        // Pure red (5 bits max) → R=255
        let red = rgb565_to_argb(0xF800);
        assert_eq!((red >> 16) & 0xFF, 255);
    }

    #[test]
    fn test_unpack_4bit() {
        let packed = vec![0xAB, 0xCD];
        let unpacked = unpack_4bit_pixels(&packed, 4);
        assert_eq!(unpacked.len(), 4);
        assert_eq!(unpacked[0], 0xA * 17); // 170
        assert_eq!(unpacked[1], 0xB * 17); // 187
        assert_eq!(unpacked[2], 0xC * 17); // 204
        assert_eq!(unpacked[3], 0xD * 17); // 221
    }

    #[test]
    fn test_unpack_2bit() {
        let packed = vec![0b11_10_01_00];
        let unpacked = unpack_2bit_pixels(&packed, 4);
        assert_eq!(unpacked.len(), 4);
        assert_eq!(unpacked[0], 255); // 3 * 85
        assert_eq!(unpacked[1], 170); // 2 * 85
        assert_eq!(unpacked[2], 85); // 1 * 85
        assert_eq!(unpacked[3], 0); // 0 * 85
    }

    #[test]
    fn test_is_mgw_format() {
        assert!(is_mgw_format(b"SM510\0\0\0more_data"));
        assert!(is_mgw_format(&[0x04, 0x22, 0x4D, 0x18, 0x00]));
        assert!(is_mgw_format(b"ZLIBmore_data"));
        assert!(is_mgw_format(b"LZMAmore_data"));
        assert!(!is_mgw_format(b"NES\x1A"));
        assert!(!is_mgw_format(b"AB"));
    }

    #[test]
    fn test_read_helpers() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        assert_eq!(read_u16_le(&data, 0), 0x0201);
        assert_eq!(read_u32_le(&data, 0), 0x04030201);
        assert_eq!(read_u16_le(&data, 3), 0x0504);
    }

    /// Test parsing a minimal valid uncompressed .mgw structure.
    #[test]
    fn test_parse_minimal_mgw() {
        // Build a minimal valid .mgw file in memory
        let mut data = vec![0u8; 512];

        // CPU name at offset 0
        data[0..5].copy_from_slice(b"SM510");

        // Signature at offset 8
        data[8..16].copy_from_slice(b"test_rom");

        // Flags at offset 0x18 = 0
        // Section descriptors at offset 0x1C
        let sec_base = 0x1C;

        // All section offsets point past header (108 bytes)
        // We'll put a tiny program at offset 108
        let prg_start: u32 = 108;
        let prg_size: u32 = 2;

        // Program section (index 8): offset at sec_base + 64, size at sec_base + 68
        data[sec_base + 64..sec_base + 68].copy_from_slice(&prg_start.to_le_bytes());
        data[sec_base + 68..sec_base + 72].copy_from_slice(&prg_size.to_le_bytes());

        // Keyboard section needs to be set (index 9) for size validation
        let kbd_start = prg_start + prg_size;
        let kbd_size: u32 = 40; // 10 × 4 bytes
        data[sec_base + 72..sec_base + 76].copy_from_slice(&kbd_start.to_le_bytes());
        data[sec_base + 76..sec_base + 80].copy_from_slice(&kbd_size.to_le_bytes());

        // Write program data (NOP + loop)
        data[108] = 0x00; // SKIP
        data[109] = 0x00;

        let rom = parse_mgw(&data).expect("Should parse minimal MGW");
        assert_eq!(rom.cpu_type, "SM510");
        assert_eq!(rom.program.len(), 2);
        assert!(!rom.lcd_inverted);
    }
}
