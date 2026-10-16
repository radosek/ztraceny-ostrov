//! Palettes, TRLEImage / TRLEImageList (0x44f1d0 / 0x44f358) and TGIFImage (0x450188).

#[derive(Debug, thiserror::Error)]
pub enum GfxError {
    #[error("truncated")]
    Truncated,
    #[error("gif: {0}")]
    Gif(#[from] gif::DecodingError),
}

pub type Palette = [[u8; 3]; 256];

/// First palette index scene palettes overwrite (0x44edb0); 0..56 are fixed UI colours.
pub const SCENE_PALETTE_FIRST: usize = 56;

/// 256 × {B, G, R, 0} → RGB.
pub fn palette(d: &[u8]) -> Result<Palette, GfxError> {
    let d = d.get(..1024).ok_or(GfxError::Truncated)?;
    let mut p = [[0; 3]; 256];
    for (i, c) in p.iter_mut().enumerate() {
        *c = [d[i * 4 + 2], d[i * 4 + 1], d[i * 4]];
    }
    Ok(p)
}

#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub transparent: u8,
    pub pixels: Vec<u8>,
}

fn u32_at(d: &[u8], o: usize) -> Result<u32, GfxError> {
    Ok(u32::from_le_bytes(d.get(o..o + 4).ok_or(GfxError::Truncated)?.try_into().unwrap()))
}

/// Decode one TRLEImage at `pos`; returns the image and the position after it.
pub fn rle(d: &[u8], pos: usize) -> Result<(Image, usize), GfxError> {
    let width = u32_at(d, pos)?;
    let height = u32_at(d, pos + 4)?;
    let size = u32_at(d, pos + 8)? as usize;
    let transparent = *d.get(pos + 12).ok_or(GfxError::Truncated)?;
    let src = d.get(pos + 13..pos + 13 + size).ok_or(GfxError::Truncated)?;
    let n = width as usize * height as usize;
    let mut pixels = Vec::with_capacity(n);
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        i += 1;
        if c < 0x80 {
            let v = *src.get(i).ok_or(GfxError::Truncated)?;
            pixels.resize(pixels.len() + c as usize, v);
            i += 1;
        } else {
            let k = 256 - c as usize;
            pixels.extend_from_slice(src.get(i..i + k).ok_or(GfxError::Truncated)?);
            i += k;
        }
    }
    pixels.resize(n, 0);
    Ok((Image { width, height, transparent, pixels }, pos + 13 + size))
}

pub fn rle_list(d: &[u8]) -> Result<Vec<Image>, GfxError> {
    let n = u32_at(d, 0)?;
    let mut p = 4;
    (0..n)
        .map(|_| {
            let (img, next) = rle(d, p)?;
            p = next;
            Ok(img)
        })
        .collect()
}

/// GIF with its 6-byte signature replaced by `u32 size, 0, 0`; the transparent index is the
/// logical screen background byte. The embedded colour table is ignored by the game.
pub fn gif(d: &[u8]) -> Result<Image, GfxError> {
    let mut fixed = Vec::with_capacity(d.len());
    fixed.extend_from_slice(b"GIF89a");
    fixed.extend_from_slice(d.get(6..).ok_or(GfxError::Truncated)?);
    let transparent = *d.get(11).ok_or(GfxError::Truncated)?;
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::Indexed);
    let mut dec = opts.read_info(fixed.as_slice())?;
    let (width, height) = (dec.width() as u32, dec.height() as u32);
    let frame = dec.read_next_frame()?.ok_or(GfxError::Truncated)?;
    let mut pixels = vec![0; width as usize * height as usize];
    for y in 0..frame.height as usize {
        let dst = (frame.top as usize + y) * width as usize + frame.left as usize;
        let src = &frame.buffer[y * frame.width as usize..(y + 1) * frame.width as usize];
        pixels[dst..dst + src.len()].copy_from_slice(src);
    }
    Ok(Image { width, height, transparent, pixels })
}

/// What an entry looks like. Entry types are not stored; the game knows them from context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Palette,
    Rle,
    RleList,
    Gif,
    Flc,
    Other,
}

pub fn classify(d: &[u8]) -> Kind {
    let s = d.len();
    if s == 1024 && (0..256).all(|i| d[i * 4 + 3] == 0) {
        return Kind::Palette;
    }
    if s < 16 {
        return Kind::Other;
    }
    let u = |o| u32_at(d, o).unwrap();
    let magic = u16::from_le_bytes([d[4], d[5]]);
    if u(0) as usize == s && matches!(magic, 0xAF12 | 0x00FF) {
        return Kind::Flc;
    }
    if u(0) as usize == s && d[4..6] == [0, 0] && d[10] & 0x80 != 0 {
        return Kind::Gif;
    }
    if (1..=640).contains(&u(0)) && (1..=480).contains(&u(4)) && u(8) as usize + 13 == s {
        return Kind::Rle;
    }
    let n = u(0);
    if n > 0 && n < 1000 {
        let mut p = 4;
        for _ in 0..n {
            match rle(d, p) {
                Ok((_, next)) => p = next,
                Err(_) => return Kind::Other,
            }
        }
        if p == s {
            return Kind::RleList;
        }
    }
    Kind::Other
}
