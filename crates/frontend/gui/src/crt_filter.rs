//! CRT filter effects for post-processing emulator output
//!
//! This module provides various CRT (Cathode Ray Tube) display effects that can be applied
//! to the raw emulator frame buffer to simulate the appearance of old CRT monitors and TVs.

use serde::{Deserialize, Serialize};

/// Available CRT/LCD display filter types based on real-world products
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrtFilter {
    /// No filter - raw pixels
    #[default]
    None,
    /// Sony Trinitron - Color CRT TV/Monitor (1968-2008)
    /// Famous for aperture grille, RGB phosphor triads, and vivid colors
    SonyTrinitron,
    /// IBM 5151 - Green monochrome PC monitor (1981)
    /// MDA text mode with green phosphor glow and persistence
    Ibm5151,
    /// Commodore 1702 - Color CRT monitor (1985)
    /// Popular for C64/Amiga, shadow mask, moderate scanlines
    Commodore1702,
    /// Sharp PC-1500 - B/W LCD laptop (1982)
    /// Early passive matrix LCD with blur and low contrast
    SharpLcd,
    /// RCA Victor - B/W CRT TV (1950s-60s)
    /// Vintage black and white TV with heavy scanlines and vignette
    RcaVictor,
}

impl CrtFilter {
    /// Get the name of the filter for display
    pub fn name(&self) -> &str {
        match self {
            CrtFilter::None => "None",
            CrtFilter::SonyTrinitron => "Sony Trinitron",
            CrtFilter::Ibm5151 => "IBM 5151",
            CrtFilter::Commodore1702 => "Commodore 1702",
            CrtFilter::SharpLcd => "Sharp LCD",
            CrtFilter::RcaVictor => "RCA Victor",
        }
    }

    /// Cycle to the next filter in the sequence
    pub fn next(&self) -> Self {
        match self {
            CrtFilter::None => CrtFilter::SonyTrinitron,
            CrtFilter::SonyTrinitron => CrtFilter::Ibm5151,
            CrtFilter::Ibm5151 => CrtFilter::Commodore1702,
            CrtFilter::Commodore1702 => CrtFilter::SharpLcd,
            CrtFilter::SharpLcd => CrtFilter::RcaVictor,
            CrtFilter::RcaVictor => CrtFilter::None,
        }
    }

    /// Apply the filter to a frame buffer
    ///
    /// # Arguments
    /// * `buffer` - The input frame buffer (will be modified in place)
    /// * `width` - Width of the frame
    /// * `height` - Height of the frame
    pub fn apply(&self, buffer: &mut [u32], width: usize, height: usize) {
        match self {
            CrtFilter::None => {
                // No processing needed
            }
            CrtFilter::SonyTrinitron => {
                apply_sony_trinitron(buffer, width, height);
            }
            CrtFilter::Ibm5151 => {
                apply_ibm5151(buffer, width, height);
            }
            CrtFilter::Commodore1702 => {
                apply_commodore1702(buffer, width, height);
            }
            CrtFilter::SharpLcd => {
                apply_sharp_lcd(buffer, width, height);
            }
            CrtFilter::RcaVictor => {
                apply_rca_victor(buffer, width, height);
            }
        }
    }
}

/// Extract RGB components from a 0xRRGGBB color
#[inline]
fn unpack_rgb(color: u32) -> (u8, u8, u8) {
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    (r, g, b)
}

/// Pack RGB components into 0xRRGGBB color
#[inline]
fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Blend two colors with a given ratio (0.0 = all color1, 1.0 = all color2)
#[allow(dead_code)] // Still used in tests
#[inline]
fn blend_colors(color1: u32, color2: u32, ratio: f32) -> u32 {
    let (r1, g1, b1) = unpack_rgb(color1);
    let (r2, g2, b2) = unpack_rgb(color2);

    let r = (r1 as f32 * (1.0 - ratio) + r2 as f32 * ratio) as u8;
    let g = (g1 as f32 * (1.0 - ratio) + g2 as f32 * ratio) as u8;
    let b = (b1 as f32 * (1.0 - ratio) + b2 as f32 * ratio) as u8;

    pack_rgb(r, g, b)
}

/// Sony Trinitron - Color CRT with aperture grille, RGB phosphor triads, bloom
/// Known for vertical RGB stripes instead of shadow mask dots
fn apply_sony_trinitron(buffer: &mut [u32], width: usize, height: usize) {
    // First pass: Apply subtle horizontal scanlines (Trinitron had very fine scanlines)
    for y in 0..height {
        if y % 2 == 1 {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    let color = buffer[idx];
                    let r = ((color >> 16) & 0xFF) as u8;
                    let g = ((color >> 8) & 0xFF) as u8;
                    let b = (color & 0xFF) as u8;

                    // Very subtle scanlines (92% brightness)
                    let r = ((r as u16 * 235) >> 8) as u8;
                    let g = ((g as u16 * 235) >> 8) as u8;
                    let b = ((b as u16 * 235) >> 8) as u8;
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }

    // Second pass: RGB phosphor stripe effect (vertical aperture grille)
    // Trinitron's distinctive feature: vertical RGB stripes
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                let color = buffer[idx];
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;

                // Simulate RGB stripe pattern (every 3 pixels)
                let stripe = x % 3;
                let (r, g, b) = match stripe {
                    0 => (
                        r,
                        ((g as u16 * 230) >> 8) as u8,
                        ((b as u16 * 230) >> 8) as u8,
                    ), // Red stripe
                    1 => (
                        ((r as u16 * 230) >> 8) as u8,
                        g,
                        ((b as u16 * 230) >> 8) as u8,
                    ), // Green stripe
                    _ => (
                        ((r as u16 * 230) >> 8) as u8,
                        ((g as u16 * 230) >> 8) as u8,
                        b,
                    ), // Blue stripe
                };

                buffer[idx] = pack_rgb(r, g, b);
            }
        }
    }

    // Third pass: Subtle bloom on bright pixels
    let mut bloom_buffer = vec![0u32; buffer.len()];
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            if idx < buffer.len() {
                let color = buffer[idx];
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;

                // Calculate brightness
                let brightness = (r as u16 + g as u16 + b as u16) / 3;

                // Add bloom to bright pixels (threshold at 180)
                if brightness > 180 {
                    let bloom_amount = ((brightness - 180) * 2).min(255) as u8;

                    // Add bloom to neighbors
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = (x as i32 + dx) as usize;
                            let ny = (y as i32 + dy) as usize;
                            let nidx = ny * width + nx;
                            if nidx < bloom_buffer.len() {
                                let current_bloom = bloom_buffer[nidx];
                                let br = ((current_bloom >> 16) & 0xFF) as u8;
                                let bg = ((current_bloom >> 8) & 0xFF) as u8;
                                let bb = (current_bloom & 0xFF) as u8;

                                let scale = bloom_amount / 8; // Subtle bloom
                                bloom_buffer[nidx] = pack_rgb(
                                    br.saturating_add(scale),
                                    bg.saturating_add(scale),
                                    bb.saturating_add(scale),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Apply bloom
    for idx in 0..buffer.len() {
        let base = buffer[idx];
        let bloom = bloom_buffer[idx];

        let r = ((base >> 16) & 0xFF) as u8;
        let g = ((base >> 8) & 0xFF) as u8;
        let b = (base & 0xFF) as u8;

        let br = ((bloom >> 16) & 0xFF) as u8;
        let bg = ((bloom >> 8) & 0xFF) as u8;
        let bb = (bloom & 0xFF) as u8;

        buffer[idx] = pack_rgb(
            r.saturating_add(br),
            g.saturating_add(bg),
            b.saturating_add(bb),
        );
    }
}

/// IBM 5151 - Green monochrome PC monitor with phosphor glow and persistence
/// MDA text mode with distinctive green phosphor
fn apply_ibm5151(buffer: &mut [u32], width: usize, height: usize) {
    // Convert to grayscale and apply green phosphor
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                let color = buffer[idx];
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;

                // Convert to luminance
                let lum = ((r as u16 * 77 + g as u16 * 150 + b as u16 * 29) >> 8) as u8;

                // Apply green phosphor (green-dominant)
                let r = (lum as u16 * 30) >> 8; // Very little red
                let g = lum; // Full green
                let b = (lum as u16 * 40) >> 8; // Slight blue for authenticity

                buffer[idx] = pack_rgb(r as u8, g, b as u8);
            }
        }
    }

    // Apply phosphor glow (horizontal bleeding)
    for y in 0..height {
        let row_start = y * width;
        let row_end = (row_start + width).min(buffer.len());

        if row_end <= row_start {
            continue;
        }

        // Right to left pass
        for x in (1..width).rev() {
            let idx = row_start + x;
            if idx >= row_end {
                continue;
            }

            let current = buffer[idx];
            let left = buffer[idx - 1];

            let (_, g_curr, _) = unpack_rgb(current);
            let (_, g_left, _) = unpack_rgb(left);

            // Strong glow effect (20% from left neighbor)
            let g = ((g_curr as u16 * 204 + g_left as u16 * 51) >> 8) as u8;

            buffer[idx] = pack_rgb(
                ((g as u16 * 30) >> 8) as u8,
                g,
                ((g as u16 * 40) >> 8) as u8,
            );
        }
    }

    // Apply scanlines
    for y in 0..height {
        if y % 2 == 1 {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    let color = buffer[idx];
                    let (r, g, b) = unpack_rgb(color);

                    // Moderate scanlines (80% brightness)
                    let r = ((r as u16 * 204) >> 8) as u8;
                    let g = ((g as u16 * 204) >> 8) as u8;
                    let b = ((b as u16 * 204) >> 8) as u8;
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }
}

/// Commodore 1702 - Color CRT monitor with shadow mask and moderate scanlines
/// Popular for C64/Amiga gaming
fn apply_commodore1702(buffer: &mut [u32], width: usize, height: usize) {
    // Apply horizontal color bleeding (phosphor glow)
    for y in 0..height {
        let row_start = y * width;
        let row_end = (row_start + width).min(buffer.len());

        if row_end <= row_start {
            continue;
        }

        // Process right to left
        for x in (1..width).rev() {
            let idx = row_start + x;
            if idx >= row_end {
                continue;
            }

            let current = buffer[idx];
            let left = buffer[idx - 1];

            let (r_curr, g_curr, b_curr) = unpack_rgb(current);
            let (r_left, g_left, b_left) = unpack_rgb(left);

            // Medium blend (15% from left)
            let r = ((r_curr as u16 * 217 + r_left as u16 * 38) >> 8) as u8;
            let g = ((g_curr as u16 * 217 + g_left as u16 * 38) >> 8) as u8;
            let b = ((b_curr as u16 * 217 + b_left as u16 * 38) >> 8) as u8;

            buffer[idx] = pack_rgb(r, g, b);
        }
    }

    // Apply moderate scanlines
    for y in 0..height {
        if y % 2 == 1 {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    let color = buffer[idx];
                    let (r, g, b) = unpack_rgb(color);

                    // Moderate scanlines (75% brightness)
                    let r = ((r as u16 * 191) >> 8) as u8;
                    let g = ((g as u16 * 191) >> 8) as u8;
                    let b = ((b as u16 * 191) >> 8) as u8;
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }

    // Shadow mask pattern (subtle RGB dots in triangular pattern)
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                let color = buffer[idx];
                let (r, g, b) = unpack_rgb(color);

                // Create dot pattern based on position
                let dot_x = x % 3;
                let dot_y = y % 3;

                // Triangular shadow mask pattern
                let (r, g, b) = match (dot_x, dot_y) {
                    (0, 0) => (
                        r,
                        ((g as u16 * 240) >> 8) as u8,
                        ((b as u16 * 240) >> 8) as u8,
                    ), // Red dot
                    (1, 1) => (
                        ((r as u16 * 240) >> 8) as u8,
                        g,
                        ((b as u16 * 240) >> 8) as u8,
                    ), // Green dot
                    (2, 2) => (
                        ((r as u16 * 240) >> 8) as u8,
                        ((g as u16 * 240) >> 8) as u8,
                        b,
                    ), // Blue dot
                    _ => (r, g, b), // No dot
                };

                buffer[idx] = pack_rgb(r, g, b);
            }
        }
    }
}

/// Sharp LCD - Early passive matrix LCD with blur, low contrast, and pixel grid
/// Simulates early 1980s portable LCD technology
fn apply_sharp_lcd(buffer: &mut [u32], width: usize, height: usize) {
    // Convert to grayscale (early LCDs were monochrome)
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                let color = buffer[idx];
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;

                // Convert to luminance with reduced contrast
                let lum = ((r as u16 * 77 + g as u16 * 150 + b as u16 * 29) >> 8) as u8;

                // Reduce contrast (compress range to 64-192 instead of 0-255)
                let lum = 64 + ((lum as u16 * 128) >> 8);

                // LCD has slight blue/green tint
                let r = ((lum * 90) >> 8) as u8; // Less red
                let g = ((lum * 100) >> 8) as u8; // Neutral green
                let b = ((lum * 95) >> 8) as u8; // Slight blue

                buffer[idx] = pack_rgb(r, g, b);
            }
        }
    }

    // Apply motion blur effect (LCD persistence/ghosting)
    let mut blur_buffer = vec![0u32; buffer.len()];
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                let mut r_sum = 0u16;
                let mut g_sum = 0u16;
                let mut b_sum = 0u16;
                let mut count = 0u16;

                // Sample 3x3 area for blur
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let nx = (x as i32 + dx).max(0).min((width - 1) as i32) as usize;
                        let ny = (y as i32 + dy).max(0).min((height - 1) as i32) as usize;
                        let nidx = ny * width + nx;

                        if nidx < buffer.len() {
                            let color = buffer[nidx];
                            let (r, g, b) = unpack_rgb(color);
                            r_sum += r as u16;
                            g_sum += g as u16;
                            b_sum += b as u16;
                            count += 1;
                        }
                    }
                }

                blur_buffer[idx] = pack_rgb(
                    (r_sum / count) as u8,
                    (g_sum / count) as u8,
                    (b_sum / count) as u8,
                );
            }
        }
    }

    buffer.copy_from_slice(&blur_buffer);

    // Add pixel grid (LCD has visible gaps between pixels)
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                // Darken pixel borders
                if x % 3 == 2 || y % 3 == 2 {
                    let color = buffer[idx];
                    let (r, g, b) = unpack_rgb(color);

                    // Darken border pixels
                    let r = ((r as u16 * 179) >> 8) as u8; // 70% brightness
                    let g = ((g as u16 * 179) >> 8) as u8;
                    let b = ((b as u16 * 179) >> 8) as u8;
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }
}

/// RCA Victor - Vintage B/W CRT TV with heavy scanlines and vignette
/// Simulates 1950s-60s black and white television
fn apply_rca_victor(buffer: &mut [u32], width: usize, height: usize) {
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let max_dist = (center_x * center_x + center_y * center_y).sqrt();

    // Convert to B/W and apply vignette
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                let color = buffer[idx];
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;

                // Convert to luminance
                let mut lum = ((r as u16 * 77 + g as u16 * 150 + b as u16 * 29) >> 8) as u8;

                // Calculate distance from center for vignette
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let vignette = 1.0 - ((dist / max_dist) * 0.4); // 40% darkening at edges

                // Apply vignette
                lum = (lum as f32 * vignette) as u8;

                buffer[idx] = pack_rgb(lum, lum, lum);
            }
        }
    }

    // Apply heavy scanlines (vintage CRTs had very prominent scanlines)
    for y in 0..height {
        if y % 2 == 1 {
            for x in 0..width {
                let idx = y * width + x;
                if idx < buffer.len() {
                    let color = buffer[idx];
                    let (r, g, b) = unpack_rgb(color);

                    // Heavy scanlines (50% brightness)
                    let r = r >> 1;
                    let g = g >> 1;
                    let b = b >> 1;
                    buffer[idx] = pack_rgb(r, g, b);
                }
            }
        }
    }

    // Reduce overall contrast for vintage look
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < buffer.len() {
                let color = buffer[idx];
                let (r, g, b) = unpack_rgb(color);

                // Compress dynamic range (vintage TVs had limited contrast)
                let r = 32 + ((r as u16 * 192) >> 8);
                let g = 32 + ((g as u16 * 192) >> 8);
                let b = 32 + ((b as u16 * 192) >> 8);

                buffer[idx] = pack_rgb(r as u8, g as u8, b as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cycling() {
        let filter = CrtFilter::None;
        assert_eq!(filter.next(), CrtFilter::SonyTrinitron);

        let filter = CrtFilter::SonyTrinitron;
        assert_eq!(filter.next(), CrtFilter::Ibm5151);

        let filter = CrtFilter::Ibm5151;
        assert_eq!(filter.next(), CrtFilter::Commodore1702);

        let filter = CrtFilter::Commodore1702;
        assert_eq!(filter.next(), CrtFilter::SharpLcd);

        let filter = CrtFilter::SharpLcd;
        assert_eq!(filter.next(), CrtFilter::RcaVictor);

        let filter = CrtFilter::RcaVictor;
        assert_eq!(filter.next(), CrtFilter::None);
    }

    #[test]
    fn test_filter_names() {
        assert_eq!(CrtFilter::None.name(), "None");
        assert_eq!(CrtFilter::SonyTrinitron.name(), "Sony Trinitron");
        assert_eq!(CrtFilter::Ibm5151.name(), "IBM 5151");
        assert_eq!(CrtFilter::Commodore1702.name(), "Commodore 1702");
        assert_eq!(CrtFilter::SharpLcd.name(), "Sharp LCD");
        assert_eq!(CrtFilter::RcaVictor.name(), "RCA Victor");
    }

    #[test]
    fn test_unpack_pack_rgb() {
        let color = 0xFFAABBCC;
        let (r, g, b) = unpack_rgb(color);
        assert_eq!(r, 0xAA);
        assert_eq!(g, 0xBB);
        assert_eq!(b, 0xCC);

        let packed = pack_rgb(r, g, b);
        assert_eq!(packed & 0x00FFFFFF, color & 0x00FFFFFF);
    }

    #[test]
    fn test_blend_colors() {
        let color1 = pack_rgb(100, 100, 100);
        let color2 = pack_rgb(200, 200, 200);

        let blended = blend_colors(color1, color2, 0.5);
        let (r, g, b) = unpack_rgb(blended);

        // Should be roughly midpoint
        assert!((r as i32 - 150).abs() <= 1);
        assert!((g as i32 - 150).abs() <= 1);
        assert!((b as i32 - 150).abs() <= 1);
    }

    #[test]
    fn test_sony_trinitron_filter() {
        let width = 4;
        let height = 4;
        let mut buffer = vec![0xFFFFFFFF; width * height];

        apply_sony_trinitron(&mut buffer, width, height);

        // Filter should have been applied (pixels modified)
        let all_same = buffer.iter().all(|&c| c == 0xFFFFFFFF);
        assert!(!all_same, "Sony Trinitron filter should modify pixels");
    }

    #[test]
    fn test_ibm5151_filter() {
        let width = 4;
        let height = 4;
        let mut buffer = vec![0xFFFFFFFF; width * height];

        apply_ibm5151(&mut buffer, width, height);

        // Should convert to green monochrome
        for &color in &buffer {
            let (r, g, b) = unpack_rgb(color);
            // Green channel should be dominant
            assert!(
                g >= r && g >= b,
                "IBM 5151 should have green-dominant output"
            );
        }
    }

    #[test]
    fn test_rca_victor_filter() {
        let width = 4;
        let height = 4;
        let mut buffer = vec![0xFFFFFFFF; width * height];

        apply_rca_victor(&mut buffer, width, height);

        // Should convert to grayscale
        for &color in &buffer {
            let (r, g, b) = unpack_rgb(color);
            // All channels should be equal (grayscale)
            assert_eq!(r, g);
            assert_eq!(g, b);
        }
    }

    #[test]
    fn test_default_filter() {
        assert_eq!(CrtFilter::default(), CrtFilter::None);
    }
}
