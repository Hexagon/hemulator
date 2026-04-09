//! Software RDP Renderer – CPU-based rasterisation for unit tests
//!
//! This module implements the `RdpRenderer` trait using pure-Rust software
//! rasterisation so that the N64 test suite can run in CI without a real
//! OpenGL context.
//!
//! # Design
//!
//! All drawing operates on a `Frame` (RGBA pixel array) and a `ZBuffer` that
//! live in ordinary heap memory.  There are no GL calls of any kind.
//!
//! Triangle rasterisation uses a standard top-left fill-convention scanline
//! algorithm: sort vertices by Y, then walk two edge pairs (long edge v0→v2
//! plus short edges v0→v1 and v1→v2) and fill horizontal spans.  Attributes
//! (depth, colour, texture coordinates) are linearly interpolated.

use super::rdp_renderer::{RdpRenderer, ScissorBox};
use emu_core::graphics::ZBuffer;
use emu_core::types::Frame;

// ────────────────────────────────────────────────────────────────────────────
// Renderer struct
// ────────────────────────────────────────────────────────────────────────────

/// CPU-based RDP renderer.  Used by tests in place of `OpenGLRdpRenderer`.
pub struct SoftwareRdpRenderer {
    frame: Frame,
    zbuffer: ZBuffer,
    zbuffer_enabled: bool,
    alpha_blend: bool,
}

impl SoftwareRdpRenderer {
    /// Create a new software renderer at the given resolution.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            frame: Frame {
                pixels: vec![0u32; (width * height) as usize],
                width,
                height,
            },
            zbuffer: ZBuffer::new(width, height),
            zbuffer_enabled: false,
            alpha_blend: false,
        }
    }

    /// Composite `src` over `dst` using standard src-alpha blending.
    #[inline]
    fn blend(src: u32, dst: u32) -> u32 {
        let sa = (src >> 24) & 0xFF;
        if sa == 0xFF {
            return src;
        }
        if sa == 0x00 {
            return dst;
        }
        let inv = 255 - sa;
        let r = (((src >> 16) & 0xFF) * sa + ((dst >> 16) & 0xFF) * inv) / 255;
        let g = (((src >> 8) & 0xFF) * sa + ((dst >> 8) & 0xFF) * inv) / 255;
        let b = ((src & 0xFF) * sa + (dst & 0xFF) * inv) / 255;
        0xFF000000 | (r << 16) | (g << 8) | b
    }

    /// Write one pixel, respecting depth test and blending.
    #[inline]
    fn put(&mut self, x: i32, y: i32, z: u16, color: u32, use_z: bool) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u32, y as u32);
        if x >= self.frame.width || y >= self.frame.height {
            return;
        }
        if use_z && !self.zbuffer.test_and_update(x, y, z) {
            return;
        }
        let idx = (y * self.frame.width + x) as usize;
        self.frame.pixels[idx] = if self.alpha_blend {
            Self::blend(color, self.frame.pixels[idx])
        } else {
            color
        };
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Interpolation helpers
// ────────────────────────────────────────────────────────────────────────────

#[inline]
fn lf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn lu16(a: u16, b: u16, t: f32) -> u16 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 65535.0) as u16
}

#[inline]
fn lc(c0: u32, c1: u32, t: f32) -> u32 {
    let ch = |mask: u32, shift: u32| -> u32 {
        let a = ((c0 >> shift) & mask) as f32;
        let b = ((c1 >> shift) & mask) as f32;
        ((a + (b - a) * t).clamp(0.0, mask as f32) as u32) & mask
    };
    (ch(0xFF, 24) << 24) | (ch(0xFF, 16) << 16) | (ch(0xFF, 8) << 8) | ch(0xFF, 0)
}

/// Multiply texel colour by shade colour (MODULATE combine mode).
#[inline]
fn modulate(tex: u32, shade: u32) -> u32 {
    let m = |ts: u32, ss: u32| -> u32 { (((tex >> ts) & 0xFF) * ((shade >> ss) & 0xFF)) / 255 };
    (m(24, 24) << 24) | (m(16, 16) << 16) | (m(8, 8) << 8) | m(0, 0)
}

// ────────────────────────────────────────────────────────────────────────────
// Vertex type
// ────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct V {
    x: i32,
    y: i32,
    z: u16,
    c: u32, // ARGB colour / shade
    s: f32,
    t: f32,
}

// ────────────────────────────────────────────────────────────────────────────
// Core triangle rasteriser
// ────────────────────────────────────────────────────────────────────────────

/// Rasterise a single triangle.
///
/// When `texture` is `Some`, texture coordinates (s, t) are looked up via the
/// callback and the result is modulated by the per-vertex shade colour.  Pass
/// `c = 0xFFFFFFFF` for all vertices to get unmodulated (DECAL) texture.
fn draw_tri(
    r: &mut SoftwareRdpRenderer,
    mut p: [V; 3],
    sci: &ScissorBox,
    use_z: bool,
    texture: Option<&dyn Fn(f32, f32) -> u32>,
) {
    // Sort by ascending Y.
    if p[1].y < p[0].y {
        p.swap(0, 1);
    }
    if p[2].y < p[1].y {
        p.swap(1, 2);
    }
    if p[1].y < p[0].y {
        p.swap(0, 1);
    }

    let (a, b, c) = (p[0], p[1], p[2]);
    let total_h = (c.y - a.y).max(1) as f32;

    // Process each segment (top-half and bottom-half of the triangle).
    for seg in 0..2u32 {
        let (y0, y1, near, far) = if seg == 0 {
            (a.y, b.y, a, b)
        } else {
            (b.y, c.y, b, c)
        };
        let seg_h = (far.y - near.y).max(1) as f32;

        for y in y0..y1 {
            if y < sci.y_min as i32 || y >= sci.y_max as i32 {
                continue;
            }
            // Parameter along the long edge a→c
            let ta = (y - a.y) as f32 / total_h;
            // Parameter along the short edge near→far
            let tb = (y - near.y) as f32 / seg_h;

            // X positions on each edge
            let xa = lf(a.x as f32, c.x as f32, ta);
            let xb = lf(near.x as f32, far.x as f32, tb);

            // Determine which edge is left vs right
            let (xl, xr) = if xa < xb {
                (xa as i32, xb as i32)
            } else {
                (xb as i32, xa as i32)
            };

            // Per-scanline interpolated attributes at left and right X
            let zl = lu16(a.z, c.z, if xa < xb { ta } else { tb });
            let zr = lu16(near.z, far.z, if xa < xb { tb } else { ta });
            let cl = lc(a.c, c.c, if xa < xb { ta } else { tb });
            let cr = lc(near.c, far.c, if xa < xb { tb } else { ta });
            let (sl, sr) = if xa < xb {
                (lf(a.s, c.s, ta), lf(near.s, far.s, tb))
            } else {
                (lf(near.s, far.s, tb), lf(a.s, c.s, ta))
            };
            let (stl, str_) = if xa < xb {
                (lf(a.t, c.t, ta), lf(near.t, far.t, tb))
            } else {
                (lf(near.t, far.t, tb), lf(a.t, c.t, ta))
            };

            // Clamp span to scissor
            let px0 = xl.max(sci.x_min as i32);
            let px1 = xr.min(sci.x_max as i32 - 1);
            if px0 > px1 {
                continue;
            }

            let span = (xr - xl).max(1) as f32;
            for px in px0..=px1 {
                let ts = (px - xl) as f32 / span;
                let z = lu16(zl, zr, ts);
                let color = match texture {
                    Some(tex) => {
                        let s = lf(sl, sr, ts);
                        let t = lf(stl, str_, ts);
                        let texel = tex(s, t);
                        let shade = lc(cl, cr, ts);
                        if shade == 0xFFFFFFFF {
                            texel
                        } else {
                            modulate(texel, shade)
                        }
                    }
                    None => lc(cl, cr, ts),
                };
                r.put(px, y, z, color, use_z);
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// RdpRenderer implementation
// ────────────────────────────────────────────────────────────────────────────

impl RdpRenderer for SoftwareRdpRenderer {
    fn init(&mut self, width: u32, height: u32) {
        self.frame = Frame {
            pixels: vec![0u32; (width * height) as usize],
            width,
            height,
        };
        self.zbuffer = ZBuffer::new(width, height);
    }

    fn get_frame(&self) -> &Frame {
        &self.frame
    }

    fn get_frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }

    fn clear(&mut self, color: u32) {
        self.frame.pixels.fill(color);
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
        let x0 = x.max(scissor.x_min);
        let y0 = y.max(scissor.y_min);
        let x1 = (x + width).min(scissor.x_max).min(self.frame.width);
        let y1 = (y + height).min(scissor.y_max).min(self.frame.height);
        for py in y0..y1 {
            for px in x0..x1 {
                let idx = (py * self.frame.width + px) as usize;
                self.frame.pixels[idx] = if self.alpha_blend {
                    Self::blend(color, self.frame.pixels[idx])
                } else {
                    color
                };
            }
        }
    }

    fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.frame.width && y < self.frame.height {
            let idx = (y * self.frame.width + x) as usize;
            self.frame.pixels[idx] = if self.alpha_blend {
                Self::blend(color, self.frame.pixels[idx])
            } else {
                color
            };
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
        let verts = [
            V {
                x: x0,
                y: y0,
                z: 0x8000,
                c: color,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x1,
                y: y1,
                z: 0x8000,
                c: color,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x2,
                y: y2,
                z: 0x8000,
                c: color,
                s: 0.0,
                t: 0.0,
            },
        ];
        draw_tri(self, verts, scissor, false, None);
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
        let verts = [
            V {
                x: x0,
                y: y0,
                z: z0,
                c: color,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x1,
                y: y1,
                z: z1,
                c: color,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x2,
                y: y2,
                z: z2,
                c: color,
                s: 0.0,
                t: 0.0,
            },
        ];
        draw_tri(self, verts, scissor, self.zbuffer_enabled, None);
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
        let verts = [
            V {
                x: x0,
                y: y0,
                z: 0x8000,
                c: c0,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x1,
                y: y1,
                z: 0x8000,
                c: c1,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x2,
                y: y2,
                z: 0x8000,
                c: c2,
                s: 0.0,
                t: 0.0,
            },
        ];
        draw_tri(self, verts, scissor, false, None);
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
        let verts = [
            V {
                x: x0,
                y: y0,
                z: z0,
                c: c0,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x1,
                y: y1,
                z: z1,
                c: c1,
                s: 0.0,
                t: 0.0,
            },
            V {
                x: x2,
                y: y2,
                z: z2,
                c: c2,
                s: 0.0,
                t: 0.0,
            },
        ];
        draw_tri(self, verts, scissor, self.zbuffer_enabled, None);
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
        let verts = [
            V {
                x: x0,
                y: y0,
                z: 0x8000,
                c: 0xFFFFFFFF,
                s: s0,
                t: t0,
            },
            V {
                x: x1,
                y: y1,
                z: 0x8000,
                c: 0xFFFFFFFF,
                s: s1,
                t: t1,
            },
            V {
                x: x2,
                y: y2,
                z: 0x8000,
                c: 0xFFFFFFFF,
                s: s2,
                t: t2,
            },
        ];
        draw_tri(self, verts, scissor, false, Some(texture));
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
        let verts = [
            V {
                x: x0,
                y: y0,
                z: z0,
                c: 0xFFFFFFFF,
                s: s0,
                t: t0,
            },
            V {
                x: x1,
                y: y1,
                z: z1,
                c: 0xFFFFFFFF,
                s: s1,
                t: t1,
            },
            V {
                x: x2,
                y: y2,
                z: z2,
                c: 0xFFFFFFFF,
                s: s2,
                t: t2,
            },
        ];
        draw_tri(self, verts, scissor, self.zbuffer_enabled, Some(texture));
    }

    fn draw_triangle_textured_shaded_zbuffer(
        &mut self,
        x0: i32,
        y0: i32,
        z0: u16,
        s0: f32,
        t0: f32,
        c0: u32,
        x1: i32,
        y1: i32,
        z1: u16,
        s1: f32,
        t1: f32,
        c1: u32,
        x2: i32,
        y2: i32,
        z2: u16,
        s2: f32,
        t2: f32,
        c2: u32,
        texture: &dyn Fn(f32, f32) -> u32,
        scissor: &ScissorBox,
    ) {
        let verts = [
            V {
                x: x0,
                y: y0,
                z: z0,
                c: c0,
                s: s0,
                t: t0,
            },
            V {
                x: x1,
                y: y1,
                z: z1,
                c: c1,
                s: s1,
                t: t1,
            },
            V {
                x: x2,
                y: y2,
                z: z2,
                c: c2,
                s: s2,
                t: t2,
            },
        ];
        draw_tri(self, verts, scissor, self.zbuffer_enabled, Some(texture));
    }

    fn clear_zbuffer(&mut self) {
        self.zbuffer.clear();
    }

    fn set_zbuffer_enabled(&mut self, enabled: bool) {
        self.zbuffer_enabled = enabled;
        self.zbuffer.set_enabled(enabled);
    }

    fn set_alpha_blend(&mut self, enabled: bool) {
        self.alpha_blend = enabled;
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.init(width, height);
    }

    fn reset(&mut self) {
        let (w, h) = (self.frame.width, self.frame.height);
        self.frame.pixels.fill(0);
        self.zbuffer = ZBuffer::new(w, h);
        self.zbuffer_enabled = false;
        self.alpha_blend = false;
    }

    fn name(&self) -> &str {
        "Software"
    }

    fn is_hardware_accelerated(&self) -> bool {
        false
    }
}
