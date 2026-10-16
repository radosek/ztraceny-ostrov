//! TFlic (0x44f744): Autodesk FLC as the game plays it. Accepts magic 0xAF12 and the
//! delta-only variant 0x00FF; full-frame RLE is fixed at 640×420; the 0x8000 "last byte" word
//! is ignored; custom chunk 0x80 carries opaque raw rectangles drawn to the screen only.

#[derive(Debug, thiserror::Error)]
pub enum FlcError {
    #[error("not a FLC file (magic {0:#06x})")]
    Magic(u16),
    #[error("truncated at {0}")]
    Truncated(usize),
}

pub const PLAYFIELD_W: usize = 640;
pub const PLAYFIELD_H: usize = 420;

/// Receives what a frame changes. Offsets are `y * 640 + x`.
pub trait Sink {
    /// Index pixels (written to the screen and the index mirror).
    fn pixels(&mut self, offset: usize, px: &[u8]);
    fn fill(&mut self, offset: usize, n: usize, v: u8);
    /// Chunk 0x80 rectangle row: screen only.
    fn raw_row(&mut self, offset: usize, px: &[u8]);
    /// COLOR_256 entry (only reported when enabled by the player).
    fn color(&mut self, index: usize, rgb: [u8; 3]);
    /// Called after a frame that contained a palette chunk.
    fn palette_done(&mut self) {}
}

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub frames: u16,
    pub width: u16,
    pub height: u16,
    pub speed_ms: u32,
    pub first_frame: usize,
    pub loop_frame: usize,
    pub delta_only: bool,
}

fn u16_at(d: &[u8], o: usize) -> Result<u16, FlcError> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(FlcError::Truncated(o))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32, FlcError> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(FlcError::Truncated(o))
}
fn byte(d: &[u8], o: usize) -> Result<u8, FlcError> {
    d.get(o).copied().ok_or(FlcError::Truncated(o))
}

pub fn header(d: &[u8]) -> Result<Header, FlcError> {
    let magic = u16_at(d, 4)?;
    if !matches!(magic, 0xAF12 | 0x00FF) {
        return Err(FlcError::Magic(magic));
    }
    let o1 = u32_at(d, 0x50)? as usize;
    let o2 = u32_at(d, 0x54)? as usize;
    Ok(Header {
        frames: u16_at(d, 6)?,
        width: u16_at(d, 8)?,
        height: u16_at(d, 10)?,
        speed_ms: u32_at(d, 16)?,
        first_frame: if o1 == 0 { 128 } else { o1 },
        loop_frame: o2,
        delta_only: magic == 0x00FF,
    })
}

/// Playback position. `frame` counts decoded frames since the start or last rewind.
#[derive(Clone, Copy, Debug)]
pub struct Pos {
    pub offset: usize,
    pub frame: u32,
}

impl Header {
    pub fn start(&self) -> Pos {
        Pos { offset: self.first_frame, frame: 0 }
    }
}

/// Decode the frame at `pos` and advance. Returns false if there is no frame there.
pub fn decode_frame(d: &[u8], pos: &mut Pos, palette: bool, sink: &mut impl Sink) -> Result<bool, FlcError> {
    loop {
        if pos.offset + 6 > d.len() {
            return Ok(false);
        }
        let start = pos.offset;
        let size = u32_at(d, start)? as usize;
        let kind = u16_at(d, start + 4)?;
        if size < 6 {
            return Err(FlcError::Truncated(start));
        }
        pos.offset += size;
        if kind == 0xF100 {
            continue;
        }
        let chunks = u16_at(d, start + 6)?;
        let mut c = start + 16;
        let mut pal = false;
        for _ in 0..chunks {
            let csize = u32_at(d, c)? as usize;
            let ctype = u16_at(d, c + 4)?;
            let body = d.get(c + 6..c + csize).ok_or(FlcError::Truncated(c))?;
            if ctype == 4 && palette {
                pal = true;
            }
            chunk(ctype, body, palette, sink)?;
            c += csize.max(6);
        }
        if pal {
            sink.palette_done();
        }
        pos.frame += 1;
        return Ok(true);
    }
}

fn chunk(ctype: u16, b: &[u8], palette: bool, sink: &mut impl Sink) -> Result<(), FlcError> {
    let w = PLAYFIELD_W;
    match ctype {
        4 if palette => {
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
                    if idx < 256 {
                        sink.color(idx, [byte(b, p)?, byte(b, p + 1)?, byte(b, p + 2)?]);
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
                            if cnt >= 0 {
                                let n = cnt as usize * 2;
                                sink.pixels(y * w + x, b.get(p..p + n).ok_or(FlcError::Truncated(p))?);
                                p += n;
                                x += n;
                            } else {
                                let pair = [byte(b, p)?, byte(b, p + 1)?];
                                p += 2;
                                for _ in 0..-(cnt as i32) {
                                    sink.pixels(y * w + x, &pair);
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
        15 => {
            let mut p = 0;
            for y in 0..PLAYFIELD_H {
                p += 1;
                let mut x = 0;
                while x < w {
                    let cnt = byte(b, p)? as i8;
                    p += 1;
                    if cnt >= 0 {
                        let n = (cnt as usize).min(w - x);
                        sink.fill(y * w + x, n, byte(b, p)?);
                        p += 1;
                        x += n;
                    } else {
                        let n = (-(cnt as i32) as usize).min(w - x);
                        sink.pixels(y * w + x, b.get(p..p + n).ok_or(FlcError::Truncated(p))?);
                        p += n;
                        x += n;
                    }
                }
            }
        }
        0x80 => {
            let n = u16_at(b, 0)?;
            let mut p = 2;
            for _ in 0..n {
                let (x, y, rw, rh) =
                    (u16_at(b, p)? as usize, u16_at(b, p + 2)? as usize, u16_at(b, p + 4)? as usize, u16_at(b, p + 6)? as usize);
                p += 8;
                for row in 0..rh {
                    let px = b.get(p..p + rw).ok_or(FlcError::Truncated(p))?;
                    sink.raw_row((y + row) * w + x, px);
                    p += rw;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Plain 8-bit decoder for tools: index buffer + palette, rects composited in.
pub struct Indexed {
    pub pixels: Vec<u8>,
    pub palette: [[u8; 3]; 256],
    pub palette_floor: usize,
}

impl Sink for Indexed {
    fn pixels(&mut self, o: usize, px: &[u8]) {
        let end = (o + px.len()).min(self.pixels.len());
        if o < end {
            self.pixels[o..end].copy_from_slice(&px[..end - o]);
        }
    }
    fn fill(&mut self, o: usize, n: usize, v: u8) {
        let end = (o + n).min(self.pixels.len());
        if o < end {
            self.pixels[o..end].fill(v);
        }
    }
    fn raw_row(&mut self, o: usize, px: &[u8]) {
        self.pixels(o, px)
    }
    fn color(&mut self, i: usize, rgb: [u8; 3]) {
        if i >= self.palette_floor {
            self.palette[i] = rgb;
        }
    }
}

/// Records the bounding box of everything a file draws (x0, y0, x1, y1 exclusive).
#[derive(Default)]
pub struct Bounds(pub Option<(usize, usize, usize, usize)>);

impl Bounds {
    fn add(&mut self, o: usize, n: usize) {
        if n == 0 {
            return;
        }
        let (y, x0, x1) = (o / PLAYFIELD_W, o % PLAYFIELD_W, (o % PLAYFIELD_W + n).min(PLAYFIELD_W));
        self.0 = Some(match self.0 {
            None => (x0, y, x1, y + 1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y), c.max(x1), d.max(y + 1)),
        });
    }

    pub fn of(d: &[u8]) -> Self {
        let mut b = Bounds::default();
        if let Ok(h) = header(d) {
            let mut pos = h.start();
            for _ in 0..h.frames {
                if !decode_frame(d, &mut pos, false, &mut b).unwrap_or(false) {
                    break;
                }
            }
        }
        b
    }

    pub fn hits(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        self.0.is_some_and(|(a, b, c, d)| x0 < c as i32 && x1 > a as i32 && y0 < d as i32 && y1 > b as i32)
    }
}

impl Sink for Bounds {
    fn pixels(&mut self, o: usize, px: &[u8]) {
        self.add(o, px.len())
    }
    fn fill(&mut self, o: usize, n: usize, _: u8) {
        self.add(o, n)
    }
    fn raw_row(&mut self, o: usize, px: &[u8]) {
        self.add(o, px.len())
    }
    fn color(&mut self, _: usize, _: [u8; 3]) {}
}
