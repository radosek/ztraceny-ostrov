//! TFlic (0x44f744): Autodesk FLC as the game plays it. Accepts magic 0xAF12 and the
//! headerless delta-only variant 0x00FF; full-frame RLE is fixed at 640×420; custom chunk
//! 0x80 carries opaque raw rectangles drawn to the screen only (not the index buffer).

#[derive(Debug, thiserror::Error)]
pub enum FlcError {
    #[error("not a FLC/FLI file (magic {0:#06x})")]
    Magic(u16),
    #[error("truncated at {0}")]
    Truncated(usize),
}

pub struct Flc<'a> {
    data: &'a [u8],
    pub width: u16,
    pub height: u16,
    pub frames: u16,
    /// Delay per frame in milliseconds (FLI jiffies converted).
    pub speed_ms: u32,
    first_frame: usize,
}

/// Chunk 0x80 rectangle: drawn opaque to the screen, never to the index buffer.
pub struct Rect<'a> {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub pixels: &'a [u8],
}

pub struct Decoder<'a> {
    flc: &'a Flc<'a>,
    pos: usize,
    frame: u32,
    /// 8-bit index buffer (the game's mirror at 0x468b3c).
    pub pixels: Vec<u8>,
    /// RGB, 8 bits per channel.
    pub palette: [[u8; 3]; 256],
    /// Apply COLOR_256 chunks (flic+0x94); only set for full-screen cut-scenes.
    pub palette_enabled: bool,
    /// COLOR_256 leaves indices below this alone (0x468b2c: 0 or 0x38).
    pub palette_floor: usize,
    pub palette_changed: bool,
    /// Rectangles from chunk 0x80 in the last decoded frame.
    pub rects: Vec<Rect<'a>>,
}

pub const PLAYFIELD_W: usize = 640;
pub const PLAYFIELD_H: usize = 420;

fn u16_at(d: &[u8], o: usize) -> Result<u16, FlcError> {
    d.get(o..o + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(FlcError::Truncated(o))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32, FlcError> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(FlcError::Truncated(o))
}
fn byte(d: &[u8], o: usize) -> Result<u8, FlcError> {
    d.get(o).copied().ok_or(FlcError::Truncated(o))
}

impl<'a> Flc<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, FlcError> {
        let magic = u16_at(data, 4)?;
        let (speed_ms, first_frame) = match magic {
            0xAF12 | 0x00FF => {
                let o = u32_at(data, 80)? as usize;
                (u32_at(data, 16)?, if o == 0 { 128 } else { o })
            }
            m => return Err(FlcError::Magic(m)),
        };
        Ok(Self {
            data,
            frames: u16_at(data, 6)?,
            width: u16_at(data, 8)?,
            height: u16_at(data, 10)?,
            speed_ms,
            first_frame,
        })
    }

    pub fn is_delta_only(&self) -> bool {
        u16::from_le_bytes([self.data[4], self.data[5]]) == 0x00FF
    }

    /// Decoder starting from `base` (640×480 indices; for delta-only files the current scene).
    pub fn decoder_on(&'a self, base: Vec<u8>, palette: [[u8; 3]; 256]) -> Decoder<'a> {
        Decoder {
            flc: self,
            pos: self.first_frame,
            frame: 0,
            pixels: base,
            palette,
            palette_enabled: true,
            palette_floor: 0,
            palette_changed: false,
            rects: Vec::new(),
        }
    }

    pub fn decoder(&'a self) -> Decoder<'a> {
        self.decoder_on(vec![0; PLAYFIELD_W * 480], [[0; 3]; 256])
    }
}

impl<'a> Decoder<'a> {
    /// Decode the next frame into `pixels`/`palette`. Returns false at the end of the
    /// file (after `frames` frames; the trailing ring frame is not decoded).
    pub fn next_frame(&mut self) -> Result<bool, FlcError> {
        let d = self.flc.data;
        loop {
            if self.frame >= self.flc.frames as u32 || self.pos + 6 > d.len() {
                return Ok(false);
            }
            let size = u32_at(d, self.pos)? as usize;
            let kind = u16_at(d, self.pos + 4)?;
            let start = self.pos;
            if size < 6 {
                return Err(FlcError::Truncated(start));
            }
            self.pos += size;
            if kind == 0xF100 {
                continue;
            }
            self.rects.clear();
            self.palette_changed = false;
            let chunks = u16_at(d, start + 6)?;
            let mut c = start + 16;
            for _ in 0..chunks {
                let csize = u32_at(d, c)? as usize;
                let ctype = u16_at(d, c + 4)?;
                let body = d.get(c + 6..c + csize).ok_or(FlcError::Truncated(c))?;
                self.chunk(ctype, body)?;
                c += csize.max(6);
            }
            self.frame += 1;
            return Ok(true);
        }
    }

    fn chunk(&mut self, ctype: u16, b: &'a [u8]) -> Result<(), FlcError> {
        let w = PLAYFIELD_W;
        match ctype {
            4 => {
                if !self.palette_enabled {
                    return Ok(());
                }
                self.palette_changed = true;
                let packets = u16_at(b, 0)?;
                let mut p = 2;
                let mut idx = 0usize;
                for _ in 0..packets {
                    idx += byte(b, p)? as usize;
                    let mut n = byte(b, p + 1)? as usize;
                    p += 2;
                    if n == 0 {
                        n = 256;
                    }
                    for _ in 0..n {
                        if idx < 256 && idx >= self.palette_floor {
                            for k in 0..3 {
                                self.palette[idx][k] = byte(b, p + k)?;
                            }
                        }
                        p += 3;
                        idx += 1;
                    }
                }
            }
            7 => {
                let mut lines = u16_at(b, 0)? as usize;
                let mut p = 2;
                let mut y = 0usize;
                while lines > 0 {
                    let word = u16_at(b, p)?;
                    p += 2;
                    match word >> 14 {
                        0 => {
                            let mut x = 0usize;
                            for _ in 0..word {
                                x += byte(b, p)? as usize;
                                let cnt = byte(b, p + 1)? as i8;
                                p += 2;
                                let row = y * w;
                                if cnt >= 0 {
                                    let n = cnt as usize * 2;
                                    let src = b.get(p..p + n).ok_or(FlcError::Truncated(p))?;
                                    self.pixels[row + x..row + x + n].copy_from_slice(src);
                                    p += n;
                                    x += n;
                                } else {
                                    let pair = [byte(b, p)?, byte(b, p + 1)?];
                                    p += 2;
                                    for _ in 0..(-(cnt as i32)) as usize {
                                        self.pixels[row + x..row + x + 2].copy_from_slice(&pair);
                                        x += 2;
                                    }
                                }
                            }
                            y += 1;
                            lines -= 1;
                        }
                        3 => y += (word as i16).unsigned_abs() as usize,
                        _ => {}
                    }
                }
            }
            0x80 => {
                let n = u16_at(b, 0)?;
                let mut p = 2;
                for _ in 0..n {
                    let (x, y, rw, rh) = (u16_at(b, p)?, u16_at(b, p + 2)?, u16_at(b, p + 4)?, u16_at(b, p + 6)?);
                    let len = rw as usize * rh as usize;
                    let pixels = b.get(p + 8..p + 8 + len).ok_or(FlcError::Truncated(p))?;
                    self.rects.push(Rect { x, y, w: rw, h: rh, pixels });
                    p += 8 + len;
                }
            }
            15 => {
                let mut p = 0;
                for y in 0..PLAYFIELD_H {
                    p += 1;
                    let row = &mut self.pixels[y * w..(y + 1) * w];
                    let mut x = 0;
                    while x < w {
                        let cnt = byte(b, p)? as i8;
                        p += 1;
                        if cnt >= 0 {
                            let n = (cnt as usize).min(w - x);
                            row[x..x + n].fill(byte(b, p)?);
                            p += 1;
                            x += n;
                        } else {
                            let n = ((-(cnt as i32)) as usize).min(w - x);
                            let src = b.get(p..p + n).ok_or(FlcError::Truncated(p))?;
                            row[x..x + n].copy_from_slice(src);
                            p += n;
                            x += n;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
