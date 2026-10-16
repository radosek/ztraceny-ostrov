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

    /// Character sprite 0x44fe5c → 0x44fcac: Bresenham decimation by 1/`scale` (never upscales),
    /// source mirrored first, pixel drawn where `depth >= mask`.
    pub fn sprite(&mut self, img: &Image, x: i32, y: i32, scale: f64, flip: bool, depth: u8, use_trans: bool) {
        let (w, h) = (img.width as i32, img.height as i32);
        let wd = dround(w as f64 / scale).min(w);
        let hd = dround(h as f64 / scale).min(h);
        if wd <= 0 || hd <= 0 {
            return;
        }
        let key = if use_trans { img.transparent } else { 0xff };
        let mut ey = 2 * hd - h;
        let mut dy = y;
        for sy in 0..h {
            if ey < 0 {
                ey += 2 * hd;
                continue;
            }
            ey += 2 * (hd - h);
            if (0..H as i32).contains(&dy) {
                let mut ex = 2 * wd - w;
                let mut dx = x;
                for sx in 0..w {
                    if ex < 0 {
                        ex += 2 * wd;
                        continue;
                    }
                    ex += 2 * (wd - w);
                    let sx = if flip { w - 1 - sx } else { sx };
                    let p = img.pixels[(sy * w + sx) as usize];
                    if (0..W as i32).contains(&dx) {
                        let o = dy as usize * W + dx as usize;
                        if depth >= self.mask[o] && p != key {
                            self.back[o] = self.lut[p as usize];
                        }
                    }
                    dx += 1;
                }
            }
            dy += 1;
        }
    }

    /// Copy a rectangle out of the back buffer (save-under).
    pub fn save_rect(&self, x: i32, y: i32, w: i32, h: i32) -> SavedRect {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(W as i32), (y + h).min(H as i32));
        let mut px = Vec::new();
        for yy in y0..y1 {
            if x0 < x1 {
                px.extend_from_slice(&self.back[yy as usize * W + x0 as usize..yy as usize * W + x1 as usize]);
            }
        }
        SavedRect { x: x0, y: y0, w: (x1 - x0).max(0), h: (y1 - y0).max(0), px }
    }

    pub fn put_rect(&mut self, r: &SavedRect) {
        for row in 0..r.h {
            let o = (r.y + row) as usize * W + r.x as usize;
            let s = (row * r.w) as usize;
            self.back[o..o + r.w as usize].copy_from_slice(&r.px[s..s + r.w as usize]);
        }
    }

    /// 0x44ee68: background rows 0..419 → screen.
    pub fn bg_to_back(&mut self) {
        self.back[..VIEW_H * W].copy_from_slice(&self.bg[..VIEW_H * W]);
    }

    /// 0x44ef8c (rows 0..479) / 0x44f008 (rows 0..419).
    pub fn clear(&mut self, rows: usize) {
        self.back[..rows * W].fill(0);
    }
}

#[derive(Clone, Debug)]
pub struct SavedRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    px: Vec<u16>,
}
