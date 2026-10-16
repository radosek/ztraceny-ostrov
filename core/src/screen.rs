//! The game's frame buffers and blitters (RGB565, 640 wide), docs/FORMATS.md §5 and ENGINE_D.

use crate::geom::dround;
use crate::gfx::{Image, Palette};

pub const W: usize = 640;
pub const H: usize = 480;
pub const VIEW_H: usize = 420;

/// 0x44ed18 for an RGB565 surface.
pub fn lut(p: &Palette) -> [u16; 256] {
    let mut l = [0u16; 256];
    for (o, c) in l.iter_mut().zip(p) {
        *o = ((c[0] as u16 >> 3) << 11) | ((c[1] as u16 >> 2) << 5) | (c[2] as u16 >> 3);
    }
    l
}

pub fn rgb565_to_rgba(src: &[u16], dst: &mut [u8]) {
    for (s, d) in src.iter().zip(dst.chunks_exact_mut(4)) {
        let r = ((s >> 11) & 31) as u8;
        let g = ((s >> 5) & 63) as u8;
        let b = (s & 31) as u8;
        d[0] = (r << 3) | (r >> 2);
        d[1] = (g << 2) | (g >> 4);
        d[2] = (b << 3) | (b >> 2);
        d[3] = 255;
    }
}

pub struct Screen {
    /// Back buffer presented every frame.
    pub back: Vec<u16>,
    /// 0x468b30: clean room background plus region images and stamped anim frames.
    pub bg: Vec<u16>,
    /// 0x468b38: foreground/priority mask, 0 where nothing occludes.
    pub mask: Vec<u8>,
    /// 0x468b3c: 8-bit index mirror (FLC palette redraws).
    pub index: Vec<u8>,
    pub palette: Palette,
    pub lut: [u16; 256],
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            back: vec![0; W * H],
            bg: vec![0; W * H],
            mask: vec![0; W * H],
            index: vec![0; W * H],
            palette: [[0; 3]; 256],
            lut: [0; 256],
        }
    }
}

pub enum Target {
    Back,
    Bg,
}

impl Screen {
    pub fn set_palette(&mut self, p: Palette) {
        self.palette = p;
        self.lut = lut(&p);
    }

    /// Overwrite colours `first..256` from `p` (scene palette load 0x44edb0 uses 56).
    pub fn load_palette_from(&mut self, p: &Palette, first: usize) {
        let mut np = self.palette;
        np[first..].copy_from_slice(&p[first..]);
        self.set_palette(np);
    }

    fn buf(&mut self, t: Target) -> &mut Vec<u16> {
        match t {
            Target::Back => &mut self.back,
            Target::Bg => &mut self.bg,
        }
    }

    /// Opaque (0x44fec4) or transparent (0x44ffa0) blit, clipped to `max_h` rows.
    pub fn blit(&mut self, t: Target, img: &Image, x: i32, y: i32, transparent: bool, max_h: usize) {
        let lut = self.lut;
        let buf = self.buf(t);
        for sy in 0..img.height as i32 {
            let dy = y + sy;
            if dy < 0 || dy >= max_h as i32 {
                continue;
            }
            for sx in 0..img.width as i32 {
                let dx = x + sx;
                if dx < 0 || dx >= W as i32 {
                    continue;
                }
                let p = img.pixels[(sy * img.width as i32 + sx) as usize];
                if transparent && p == img.transparent {
                    continue;
                }
                buf[dy as usize * W + dx as usize] = lut[p as usize];
            }
        }
    }

    /// Restore a rectangle of the back buffer from the background (0x44eea4).
    pub fn restore(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y.max(0)..(y + h).min(H as i32) {
            let r = yy as usize * W;
            let (a, b) = (x.max(0) as usize, (x + w).clamp(0, W as i32) as usize);
            if a < b {
                self.back[r + a..r + b].copy_from_slice(&self.bg[r + a..r + b]);
            }
        }
    }

    pub fn back_from_bg(&mut self) {
        self.back.copy_from_slice(&self.bg);
    }

    /// Character sprite (0x44fe5c → 0x44fcac): scaled by 1/`scale`, nearest sample, optional
    /// mirror, drawn only where `mask <= depth`.
    pub fn sprite(&mut self, img: &Image, x: i32, y: i32, scale: f64, mirror: bool, depth: u8) {
        let dw = dround(img.width as f64 / scale).max(0);
        let dh = dround(img.height as f64 / scale).max(0);
        for oy in 0..dh {
            let py = y + oy;
            if py < 0 || py >= VIEW_H as i32 {
                continue;
            }
            let sy = ((oy as f64 * scale) as i32).min(img.height as i32 - 1);
            for ox in 0..dw {
                let px = x + ox;
                if px < 0 || px >= W as i32 {
                    continue;
                }
                let mut sx = ((ox as f64 * scale) as i32).min(img.width as i32 - 1);
                if mirror {
                    sx = img.width as i32 - 1 - sx;
                }
                let p = img.pixels[(sy * img.width as i32 + sx) as usize];
                let o = py as usize * W + px as usize;
                if p == img.transparent || self.mask[o] > depth {
                    continue;
                }
                self.back[o] = self.lut[p as usize];
            }
        }
    }
}
