//! Software RDP Renderer - CPU-based rasterization
//!
//! This module implements the `RdpRenderer` trait using software (CPU-based)
//! rasterization. This is the default and most accurate renderer.

use super::rdp_renderer::{RdpRenderer, ScissorBox};
use emu_core::graphics::{ColorOps, ZBuffer};
use emu_core::types::Frame;

/// Software-based RDP renderer
pub struct SoftwareRdpRenderer {
    framebuffer: Frame,
    zbuffer: ZBuffer,
    width: u32,
    height: u32,
}

impl SoftwareRdpRenderer {
    /// Create a new software renderer
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            framebuffer: Frame::new(width, height),
            zbuffer: ZBuffer::new(width, height),
            width,
            height,
        }
    }
}

impl RdpRenderer for SoftwareRdpRenderer {
    fn init(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.framebuffer = Frame::new(width, height);
        self.zbuffer = ZBuffer::new(width, height);
    }

    fn get_frame(&self) -> &Frame {
        &self.framebuffer
    }

    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.framebuffer
    }

    fn clear(&mut self, color: u32) {
        for pixel in &mut self.framebuffer.pixels {
            *pixel = color;
        }
    }

    fn fill_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: u32,
        scissor: &ScissorBox,
    ) {
        // Apply scissor clipping
        let x_start = x.max(scissor.x_min);
        let y_start = y.max(scissor.y_min);
        let x_end = (x + width).min(scissor.x_max).min(self.width);
        let y_end = (y + height).min(scissor.y_max).min(self.height);

        // Skip if rectangle is completely clipped
        if x_start >= x_end || y_start >= y_end {
            return;
        }

        for py in y_start..y_end {
            for px in x_start..x_end {
                let idx = (py * self.width + px) as usize;
                if idx < self.framebuffer.pixels.len() {
                    self.framebuffer.pixels[idx] = color;
                }
            }
        }
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            if idx < self.framebuffer.pixels.len() {
                self.framebuffer.pixels[idx] = color;
            }
        }
    }

    fn draw_triangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: u32,
        scissor: &ScissorBox,
    ) {
        // Sort vertices by Y coordinate (y0 <= y1 <= y2)
        let (mut x0, mut y0, mut x1, mut y1, mut x2, mut y2) = (x0, y0, x1, y1, x2, y2);

        if y0 > y1 {
            std::mem::swap(&mut y0, &mut y1);
            std::mem::swap(&mut x0, &mut x1);
        }
        if y1 > y2 {
            std::mem::swap(&mut y1, &mut y2);
            std::mem::swap(&mut x1, &mut x2);
        }
        if y0 > y1 {
            std::mem::swap(&mut y0, &mut y1);
            std::mem::swap(&mut x0, &mut x1);
        }

        // Edge walking - simplified scanline rasterization
        let total_height = y2 - y0;
        if total_height == 0 {
            return; // Degenerate triangle
        }

        // Split triangle into top and bottom halves
        for y in y0..=y2 {
            let segment_height = if y < y1 { y1 - y0 } else { y2 - y1 };
            if segment_height == 0 {
                continue;
            }

            let alpha = (y - y0) as f32 / total_height as f32;
            let beta = if y < y1 {
                (y - y0) as f32 / (y1 - y0) as f32
            } else {
                (y - y1) as f32 / (y2 - y1) as f32
            };

            let xa = x0 as f32 + (x2 - x0) as f32 * alpha;
            let xb = if y < y1 {
                x0 as f32 + (x1 - x0) as f32 * beta
            } else {
                x1 as f32 + (x2 - x1) as f32 * beta
            };

            let x_start = xa.min(xb) as i32;
            let x_end = xa.max(xb) as i32;

            // Clip to scissor bounds
            let clip_x_start = x_start.max(scissor.x_min as i32);
            let clip_x_end = x_end.min(scissor.x_max as i32);
            let clip_y = y.max(scissor.y_min as i32).min(scissor.y_max as i32);

            if clip_y < 0 || clip_y >= self.height as i32 {
                continue;
            }

            for x in clip_x_start..=clip_x_end {
                if x >= 0 && x < self.width as i32 {
                    self.set_pixel(x as u32, clip_y as u32, color);
                }
            }
        }
    }

    fn draw_triangle_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        x1: i32,
        y1: i32,
        z1: u16,
        x2: i32,
        y2: i32,
        z2: u16,
        color: u32,
        scissor: &ScissorBox,
    ) {
        // Sort vertices by Y coordinate
        let mut verts = [(x0, y0, z0), (x1, y1, z1), (x2, y2, z2)];
        verts.sort_by_key(|v| v.1);
        let [(x0, y0, z0), (x1, y1, z1), (x2, y2, z2)] = verts;

        let total_height = y2 - y0;
        if total_height == 0 {
            return; // Degenerate triangle
        }

        // Split triangle into top and bottom halves
        for y in y0..=y2 {
            let segment_height = if y < y1 { y1 - y0 } else { y2 - y1 };
            if segment_height == 0 {
                continue;
            }

            let alpha = (y - y0) as f32 / total_height as f32;
            let beta = if y < y1 {
                (y - y0) as f32 / (y1 - y0) as f32
            } else {
                (y - y1) as f32 / (y2 - y1) as f32
            };

            let xa = x0 as f32 + (x2 - x0) as f32 * alpha;
            let za = z0 as f32 + (z2 as f32 - z0 as f32) * alpha;

            let (xb, zb) = if y < y1 {
                (
                    x0 as f32 + (x1 - x0) as f32 * beta,
                    z0 as f32 + (z1 as f32 - z0 as f32) * beta,
                )
            } else {
                (
                    x1 as f32 + (x2 - x1) as f32 * beta,
                    z1 as f32 + (z2 as f32 - z1 as f32) * beta,
                )
            };

            let (x_start, x_end, z_start, z_end) = if xa < xb {
                (xa as i32, xb as i32, za, zb)
            } else {
                (xb as i32, xa as i32, zb, za)
            };

            // Clip to scissor bounds
            let clip_x_start = x_start.max(scissor.x_min as i32);
            let clip_x_end = x_end.min(scissor.x_max as i32);
            let clip_y = y.max(scissor.y_min as i32).min(scissor.y_max as i32);

            if clip_y < 0 || clip_y >= self.height as i32 {
                continue;
            }

            // Interpolate Z across scanline
            let span_width = x_end - x_start;
            for x in clip_x_start..=clip_x_end {
                if x >= 0 && x < self.width as i32 {
                    let t = if span_width > 0 {
                        (x - x_start) as f32 / span_width as f32
                    } else {
                        0.0
                    };
                    let z = (z_start + (z_end - z_start) * t) as u16;

                    // Z-buffer test
                    if self.zbuffer.test_and_update(x as u32, clip_y as u32, z) {
                        self.set_pixel(x as u32, clip_y as u32, color);
                    }
                }
            }
        }
    }

    fn draw_triangle_shaded(
        &mut self,
        x0: i32,
        y0: i32,
        c0: u32,
        x1: i32,
        y1: i32,
        c1: u32,
        x2: i32,
        y2: i32,
        c2: u32,
        scissor: &ScissorBox,
    ) {
        // Sort vertices by Y coordinate
        let mut verts = [(x0, y0, c0), (x1, y1, c1), (x2, y2, c2)];
        verts.sort_by_key(|v| v.1);
        let [(x0, y0, c0), (x1, y1, c1), (x2, y2, c2)] = verts;

        let total_height = y2 - y0;
        if total_height == 0 {
            return; // Degenerate triangle
        }

        // Split triangle into top and bottom halves
        for y in y0..=y2 {
            let segment_height = if y < y1 { y1 - y0 } else { y2 - y1 };
            if segment_height == 0 {
                continue;
            }

            let alpha = (y - y0) as f32 / total_height as f32;
            let beta = if y < y1 {
                (y - y0) as f32 / (y1 - y0) as f32
            } else {
                (y - y1) as f32 / (y2 - y1) as f32
            };

            let xa = x0 as f32 + (x2 - x0) as f32 * alpha;
            let ca = ColorOps::lerp(c0, c2, alpha);

            let (xb, cb) = if y < y1 {
                (
                    x0 as f32 + (x1 - x0) as f32 * beta,
                    ColorOps::lerp(c0, c1, beta),
                )
            } else {
                (
                    x1 as f32 + (x2 - x1) as f32 * beta,
                    ColorOps::lerp(c1, c2, beta),
                )
            };

            let (x_start, x_end, c_start, c_end) = if xa < xb {
                (xa as i32, xb as i32, ca, cb)
            } else {
                (xb as i32, xa as i32, cb, ca)
            };

            // Clip to scissor bounds
            let clip_x_start = x_start.max(scissor.x_min as i32);
            let clip_x_end = x_end.min(scissor.x_max as i32);
            let clip_y = y.max(scissor.y_min as i32).min(scissor.y_max as i32);

            if clip_y < 0 || clip_y >= self.height as i32 {
                continue;
            }

            // Interpolate color across scanline
            let span_width = x_end - x_start;
            for x in clip_x_start..=clip_x_end {
                if x >= 0 && x < self.width as i32 {
                    let t = if span_width > 0 {
                        (x - x_start) as f32 / span_width as f32
                    } else {
                        0.0
                    };
                    let color = ColorOps::lerp(c_start, c_end, t);
                    self.set_pixel(x as u32, clip_y as u32, color);
                }
            }
        }
    }

    fn draw_triangle_shaded_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        c0: u32,
        x1: i32,
        y1: i32,
        z1: u16,
        c1: u32,
        x2: i32,
        y2: i32,
        z2: u16,
        c2: u32,
        scissor: &ScissorBox,
    ) {
        // Sort vertices by Y coordinate
        let mut verts = [(x0, y0, z0, c0), (x1, y1, z1, c1), (x2, y2, z2, c2)];
        verts.sort_by_key(|v| v.1);
        let [(x0, y0, z0, c0), (x1, y1, z1, c1), (x2, y2, z2, c2)] = verts;

        let total_height = y2 - y0;
        if total_height == 0 {
            return; // Degenerate triangle
        }

        // Split triangle into top and bottom halves
        for y in y0..=y2 {
            let segment_height = if y < y1 { y1 - y0 } else { y2 - y1 };
            if segment_height == 0 {
                continue;
            }

            let alpha = (y - y0) as f32 / total_height as f32;
            let beta = if y < y1 {
                (y - y0) as f32 / (y1 - y0) as f32
            } else {
                (y - y1) as f32 / (y2 - y1) as f32
            };

            let xa = x0 as f32 + (x2 - x0) as f32 * alpha;
            let za = z0 as f32 + (z2 as f32 - z0 as f32) * alpha;
            let ca = ColorOps::lerp(c0, c2, alpha);

            let (xb, zb, cb) = if y < y1 {
                (
                    x0 as f32 + (x1 - x0) as f32 * beta,
                    z0 as f32 + (z1 as f32 - z0 as f32) * beta,
                    ColorOps::lerp(c0, c1, beta),
                )
            } else {
                (
                    x1 as f32 + (x2 - x1) as f32 * beta,
                    z1 as f32 + (z2 as f32 - z1 as f32) * beta,
                    ColorOps::lerp(c1, c2, beta),
                )
            };

            let (x_start, x_end, z_start, z_end, c_start, c_end) = if xa < xb {
                (xa as i32, xb as i32, za, zb, ca, cb)
            } else {
                (xb as i32, xa as i32, zb, za, cb, ca)
            };

            // Clip to scissor bounds
            let clip_x_start = x_start.max(scissor.x_min as i32);
            let clip_x_end = x_end.min(scissor.x_max as i32);
            let clip_y = y.max(scissor.y_min as i32).min(scissor.y_max as i32);

            if clip_y < 0 || clip_y >= self.height as i32 {
                continue;
            }

            // Interpolate Z and color across scanline
            let span_width = x_end - x_start;
            for x in clip_x_start..=clip_x_end {
                if x >= 0 && x < self.width as i32 {
                    let t = if span_width > 0 {
                        (x - x_start) as f32 / span_width as f32
                    } else {
                        0.0
                    };
                    let z = (z_start + (z_end - z_start) * t) as u16;
                    let color = ColorOps::lerp(c_start, c_end, t);

                    // Z-buffer test
                    if self.zbuffer.test_and_update(x as u32, clip_y as u32, z) {
                        self.set_pixel(x as u32, clip_y as u32, color);
                    }
                }
            }
        }
    }

    fn draw_triangle_textured(
        &mut self,
        x0: i32,
        y0: i32,
        s0: f32,
        t0: f32,
        x1: i32,
        y1: i32,
        s1: f32,
        t1: f32,
        x2: i32,
        y2: i32,
        s2: f32,
        t2: f32,
        texture: &dyn Fn(f32, f32) -> u32,
        scissor: &ScissorBox,
    ) {
        // Sort vertices by Y coordinate
        let mut verts = [(x0, y0, s0, t0), (x1, y1, s1, t1), (x2, y2, s2, t2)];
        verts.sort_by_key(|v| v.1);
        let [(x0, y0, s0, t0), (x1, y1, s1, t1), (x2, y2, s2, t2)] = verts;

        let total_height = y2 - y0;
        if total_height == 0 {
            return; // Degenerate triangle
        }

        // Split triangle into top and bottom halves
        for y in y0..=y2 {
            let segment_height = if y < y1 { y1 - y0 } else { y2 - y1 };
            if segment_height == 0 {
                continue;
            }

            let alpha = (y - y0) as f32 / total_height as f32;
            let beta = if y < y1 {
                (y - y0) as f32 / (y1 - y0) as f32
            } else {
                (y - y1) as f32 / (y2 - y1) as f32
            };

            let xa = x0 as f32 + (x2 - x0) as f32 * alpha;
            let sa = s0 + (s2 - s0) * alpha;
            let ta = t0 + (t2 - t0) * alpha;

            let (xb, sb, tb) = if y < y1 {
                (
                    x0 as f32 + (x1 - x0) as f32 * beta,
                    s0 + (s1 - s0) * beta,
                    t0 + (t1 - t0) * beta,
                )
            } else {
                (
                    x1 as f32 + (x2 - x1) as f32 * beta,
                    s1 + (s2 - s1) * beta,
                    t1 + (t2 - t1) * beta,
                )
            };

            let (x_start, x_end, s_start, s_end, t_start, t_end) = if xa < xb {
                (xa as i32, xb as i32, sa, sb, ta, tb)
            } else {
                (xb as i32, xa as i32, sb, sa, tb, ta)
            };

            // Clip to scissor bounds
            let clip_x_start = x_start.max(scissor.x_min as i32);
            let clip_x_end = x_end.min(scissor.x_max as i32);
            let clip_y = y.max(scissor.y_min as i32).min(scissor.y_max as i32);

            if clip_y < 0 || clip_y >= self.height as i32 {
                continue;
            }

            // Interpolate texture coordinates across scanline
            let span_width = x_end - x_start;
            for x in clip_x_start..=clip_x_end {
                if x >= 0 && x < self.width as i32 {
                    let t_param = if span_width > 0 {
                        (x - x_start) as f32 / span_width as f32
                    } else {
                        0.0
                    };
                    let s = s_start + (s_end - s_start) * t_param;
                    let t = t_start + (t_end - t_start) * t_param;

                    let color = texture(s, t);
                    self.set_pixel(x as u32, clip_y as u32, color);
                }
            }
        }
    }

    fn draw_triangle_textured_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        s0: f32,
        t0: f32,
        x1: i32,
        y1: i32,
        z1: u16,
        s1: f32,
        t1: f32,
        x2: i32,
        y2: i32,
        z2: u16,
        s2: f32,
        t2: f32,
        texture: &dyn Fn(f32, f32) -> u32,
        scissor: &ScissorBox,
    ) {
        // Sort vertices by Y coordinate
        let mut verts = [
            (x0, y0, z0, s0, t0),
            (x1, y1, z1, s1, t1),
            (x2, y2, z2, s2, t2),
        ];
        verts.sort_by_key(|v| v.1);
        let [(x0, y0, z0, s0, t0), (x1, y1, z1, s1, t1), (x2, y2, z2, s2, t2)] = verts;

        let total_height = y2 - y0;
        if total_height == 0 {
            return; // Degenerate triangle
        }

        // Split triangle into top and bottom halves
        for y in y0..=y2 {
            let segment_height = if y < y1 { y1 - y0 } else { y2 - y1 };
            if segment_height == 0 {
                continue;
            }

            let alpha = (y - y0) as f32 / total_height as f32;
            let beta = if y < y1 {
                (y - y0) as f32 / (y1 - y0) as f32
            } else {
                (y - y1) as f32 / (y2 - y1) as f32
            };

            let xa = x0 as f32 + (x2 - x0) as f32 * alpha;
            let za = z0 as f32 + (z2 as f32 - z0 as f32) * alpha;
            let sa = s0 + (s2 - s0) * alpha;
            let ta = t0 + (t2 - t0) * alpha;

            let (xb, zb, sb, tb) = if y < y1 {
                (
                    x0 as f32 + (x1 - x0) as f32 * beta,
                    z0 as f32 + (z1 as f32 - z0 as f32) * beta,
                    s0 + (s1 - s0) * beta,
                    t0 + (t1 - t0) * beta,
                )
            } else {
                (
                    x1 as f32 + (x2 - x1) as f32 * beta,
                    z1 as f32 + (z2 as f32 - z1 as f32) * beta,
                    s1 + (s2 - s1) * beta,
                    t1 + (t2 - t1) * beta,
                )
            };

            let (x_start, x_end, z_start, z_end, s_start, s_end, t_start, t_end) = if xa < xb {
                (xa as i32, xb as i32, za, zb, sa, sb, ta, tb)
            } else {
                (xb as i32, xa as i32, zb, za, sb, sa, tb, ta)
            };

            // Clip to scissor bounds
            let clip_x_start = x_start.max(scissor.x_min as i32);
            let clip_x_end = x_end.min(scissor.x_max as i32);
            let clip_y = y.max(scissor.y_min as i32).min(scissor.y_max as i32);

            if clip_y < 0 || clip_y >= self.height as i32 {
                continue;
            }

            // Interpolate Z and texture coordinates across scanline
            let span_width = x_end - x_start;
            for x in clip_x_start..=clip_x_end {
                if x >= 0 && x < self.width as i32 {
                    let t_param = if span_width > 0 {
                        (x - x_start) as f32 / span_width as f32
                    } else {
                        0.0
                    };
                    let z = (z_start + (z_end - z_start) * t_param) as u16;
                    let s = s_start + (s_end - s_start) * t_param;
                    let t = t_start + (t_end - t_start) * t_param;

                    // Z-buffer test
                    if self.zbuffer.test_and_update(x as u32, clip_y as u32, z) {
                        let color = texture(s, t);
                        self.set_pixel(x as u32, clip_y as u32, color);
                    }
                }
            }
        }
    }

    fn clear_zbuffer(&mut self) {
        self.zbuffer.clear();
    }

    fn set_zbuffer_enabled(&mut self, enabled: bool) {
        self.zbuffer.set_enabled(enabled);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.framebuffer = Frame::new(width, height);
        self.zbuffer.resize(width, height);
    }

    fn reset(&mut self) {
        self.framebuffer = Frame::new(self.width, self.height);
        self.zbuffer.clear();
        self.zbuffer.set_enabled(false);
    }

    fn name(&self) -> &str {
        "Software RDP Renderer"
    }

    fn is_hardware_accelerated(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_software_renderer_creation() {
        let renderer = SoftwareRdpRenderer::new(320, 240);
        assert_eq!(renderer.name(), "Software RDP Renderer");
        assert!(!renderer.is_hardware_accelerated());
        assert_eq!(renderer.get_frame().width, 320);
        assert_eq!(renderer.get_frame().height, 240);
    }

    #[test]
    fn test_software_renderer_clear() {
        let mut renderer = SoftwareRdpRenderer::new(320, 240);
        renderer.clear(0xFFFF0000);

        let frame = renderer.get_frame();
        for pixel in &frame.pixels {
            assert_eq!(*pixel, 0xFFFF0000);
        }
    }

    #[test]
    fn test_software_renderer_fill_rect() {
        let mut renderer = SoftwareRdpRenderer::new(320, 240);
        let scissor = ScissorBox {
            x_min: 0,
            y_min: 0,
            x_max: 320,
            y_max: 240,
        };

        renderer.fill_rect(10, 10, 20, 20, 0xFF00FF00, &scissor);

        let frame = renderer.get_frame();
        // Check pixel inside rectangle
        let idx = (15 * 320 + 15) as usize;
        assert_eq!(frame.pixels[idx], 0xFF00FF00);

        // Check pixel outside rectangle
        assert_eq!(frame.pixels[0], 0);
    }

    #[test]
    fn test_software_renderer_set_pixel() {
        let mut renderer = SoftwareRdpRenderer::new(320, 240);
        renderer.set_pixel(100, 100, 0xFFFFFFFF);

        let frame = renderer.get_frame();
        let idx = (100 * 320 + 100) as usize;
        assert_eq!(frame.pixels[idx], 0xFFFFFFFF);
    }

    #[test]
    fn test_software_renderer_triangle() {
        let mut renderer = SoftwareRdpRenderer::new(320, 240);
        let scissor = ScissorBox {
            x_min: 0,
            y_min: 0,
            x_max: 320,
            y_max: 240,
        };

        renderer.draw_triangle(100, 50, 150, 150, 50, 150, 0xFF00FF00, &scissor);

        let frame = renderer.get_frame();
        // Check center of triangle should be green
        let idx = (116 * 320 + 100) as usize;
        assert_eq!(frame.pixels[idx], 0xFF00FF00);
    }

    #[test]
    fn test_software_renderer_zbuffer() {
        let mut renderer = SoftwareRdpRenderer::new(320, 240);
        renderer.set_zbuffer_enabled(true);

        let scissor = ScissorBox {
            x_min: 0,
            y_min: 0,
            x_max: 320,
            y_max: 240,
        };

        // Draw near triangle
        renderer.draw_triangle_zbuffer(
            100, 50, 0x4000, 150, 150, 0x4000, 50, 150, 0x4000, 0xFF00FF00, &scissor,
        );

        // Draw far triangle (should be occluded)
        renderer.draw_triangle_zbuffer(
            100, 50, 0xC000, 150, 150, 0xC000, 50, 150, 0xC000, 0xFFFF0000, &scissor,
        );

        let frame = renderer.get_frame();
        // Pixel should be green (near triangle visible)
        let idx = (116 * 320 + 100) as usize;
        assert_eq!(frame.pixels[idx], 0xFF00FF00);
    }

    #[test]
    fn test_software_renderer_textured_triangle() {
        let mut renderer = SoftwareRdpRenderer::new(320, 240);
        let scissor = ScissorBox {
            x_min: 0,
            y_min: 0,
            x_max: 320,
            y_max: 240,
        };

        // Simple checkerboard texture sampler
        let texture = |s: f32, t: f32| -> u32 {
            let si = s as u32;
            let ti = t as u32;
            if (si ^ ti) & 1 == 0 {
                0xFFFFFFFF // White
            } else {
                0xFF000000 // Black
            }
        };

        // Draw textured triangle
        renderer.draw_triangle_textured(
            100, 50, 0.0, 0.0, // v0: top
            150, 150, 10.0, 10.0, // v1: bottom-right
            50, 150, 0.0, 10.0, // v2: bottom-left
            &texture, &scissor,
        );

        let frame = renderer.get_frame();
        // Check that triangle was rendered (some pixels should be non-zero)
        let idx = (100 * 320 + 100) as usize;
        let pixel = frame.pixels[idx];
        // Should be either white or black from texture
        assert!(pixel == 0xFFFFFFFF || pixel == 0xFF000000);
    }

    #[test]
    fn test_software_renderer_textured_triangle_zbuffer() {
        let mut renderer = SoftwareRdpRenderer::new(320, 240);
        renderer.set_zbuffer_enabled(true);

        let scissor = ScissorBox {
            x_min: 0,
            y_min: 0,
            x_max: 320,
            y_max: 240,
        };

        // Red texture sampler
        let red_texture = |_s: f32, _t: f32| -> u32 { 0xFFFF0000 };

        // Green texture sampler
        let green_texture = |_s: f32, _t: f32| -> u32 { 0xFF00FF00 };

        // Draw far red textured triangle
        renderer.draw_triangle_textured_zbuffer(
            100,
            50,
            0xC000,
            0.0,
            0.0,
            150,
            150,
            0xC000,
            10.0,
            10.0,
            50,
            150,
            0xC000,
            0.0,
            10.0,
            &red_texture,
            &scissor,
        );

        // Draw near green textured triangle (should occlude red)
        renderer.draw_triangle_textured_zbuffer(
            100,
            50,
            0x4000,
            0.0,
            0.0,
            150,
            150,
            0x4000,
            10.0,
            10.0,
            50,
            150,
            0x4000,
            0.0,
            10.0,
            &green_texture,
            &scissor,
        );

        let frame = renderer.get_frame();
        // Center pixel should be green (near triangle visible)
        let idx = (100 * 320 + 100) as usize;
        assert_eq!(frame.pixels[idx], 0xFF00FF00);
    }
}
