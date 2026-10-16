//! TToolTip text (0x451610–0x451d7c): 10×17 glyphs from Data.dat 0x15, 9 px advance,
//! wrapped like Delphi WrapText at 40 columns, ink = colour, outline (value 2) = palette 0.

use crate::screen::{Screen, H, W};

const ACCENTS: &[u8] = b"\xc1\xc4\xc8\xcf\xc9\xcc\xcd\xbc\xc5\xd2\xd3\xd4\xd6\xc0\xd8\x8a\x8d\xda\xdc\xdd\x8e\xa3\xe1\xe4\xe8\xef\xe9\xec\xed\xbe\xe5\xf2\xf3\xf4\xf6\xe0\xf8\x9a\x9d\xfa\xfc\xfd\x9e\xb3\xdf\xd9\xf9";
const GW: usize = 10;
const GH: usize = 17;
pub const ADVANCE: i32 = 9;
pub const LINE_H: i32 = 17;

/// 0x4516b8 with the table built by 0x451610.
pub fn glyph(c: u8) -> usize {
    match c {
        0..0x20 => 0,
        0x20..0x80 => (c - 0x20) as usize,
        0x84 | 0x93 | 0x94 | 0xbd => 2,
        0x91 | 0x92 | 0xb4 => 7,
        0xa0 => 0,
        _ => ACCENTS.iter().position(|&a| a == c).map_or(0x5f, |i| 0x60 + i),
    }
}

fn blank(c: u8) -> bool {
    c <= b' '
}

/// SysUtils.WrapText(s, #13, [#0..' '], max_col) as traced in ENGINE_D §D9. Returns lines
/// (break characters stay at the end of their line).
pub fn wrap(text: &[u8], max_col: usize) -> Vec<Vec<u8>> {
    let s: Vec<u8> = text.iter().map(|&c| match c {
        b'"' => 0xff,
        b'\'' => 0xfe,
        c => c,
    }).collect();
    let len = s.len();
    let mut out: Vec<u8> = Vec::new();
    let (mut col, mut pos, mut line_pos, mut break_pos) = (1usize, 1usize, 1usize, 0usize);
    let mut existing = false;
    while pos <= len {
        let ch = s[pos - 1];
        if ch == b'\r' {
            existing = true;
            break_pos = pos;
        } else if blank(ch) {
            break_pos = pos;
        }
        pos += 1;
        col += 1;
        if existing || (col > max_col && break_pos > line_pos) {
            col = pos - break_pos;
            out.extend_from_slice(&s[line_pos - 1..break_pos]);
            while pos <= len && (blank(s[pos - 1]) || s[pos - 1] == b'\n') {
                pos += 1;
            }
            if !existing && pos < len {
                out.push(b'\r');
            }
            break_pos += 1;
            line_pos = break_pos;
            existing = false;
        }
    }
    if line_pos <= len {
        out.extend_from_slice(&s[line_pos - 1..]);
    }
    let restore = |c: u8| match c {
        0xff => b'"',
        0xfe => b'\'',
        c => c,
    };
    let mut lines: Vec<Vec<u8>> = out.split(|&c| c == b'\r').map(|l| l.iter().map(|&c| restore(c)).collect()).collect();
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

/// A laid-out tooltip (SetText 0x451834).
#[derive(Clone, Debug)]
pub struct Layout {
    pub lines: Vec<Vec<u8>>,
    pub width: i32,
    pub height: i32,
}

pub fn layout(text: &[u8], max_col: usize) -> Layout {
    let lines = wrap(text, max_col);
    let max_len = lines.iter().map(|l| l.len()).max().unwrap_or(0) as i32;
    Layout { width: 9 * max_len + 1, height: LINE_H * lines.len() as i32, lines }
}

/// Clamp 0x4517c4.
pub fn clamp(l: &Layout, x: i32, y: i32) -> (i32, i32) {
    let mut x = x.max(0);
    if x + l.width > 639 {
        x = 639 - l.width;
    }
    let mut y = y.max(0);
    if y + l.height > H as i32 {
        y = H as i32 - l.height;
    }
    (x, y)
}

/// Glyph 0x4516e8 (unclipped in the original; clipped here only to stay in bounds).
fn draw_glyph(scr: &mut Screen, font: &[u8], g: usize, x: i32, y: i32, color: u8) {
    let Some(gd) = font.get(g * GW * GH..(g + 1) * GW * GH) else { return };
    let ink = scr.lut[color as usize];
    let outline = scr.lut[0];
    for r in 0..GH {
        for c in 0..GW {
            let v = gd[r * GW + c];
            let (px, py) = (x + c as i32, y + r as i32);
            if v == 0 || px < 0 || py < 0 || px >= W as i32 || py >= H as i32 {
                continue;
            }
            scr.back[py as usize * W + px as usize] = if v == 2 { outline } else { ink };
        }
    }
}

/// Draw 0x451a74: each line centred in the box.
pub fn draw(scr: &mut Screen, font: &[u8], l: &Layout, left: i32, top: i32, color: u8) {
    for (i, line) in l.lines.iter().enumerate() {
        let x = left + (l.width - 9 * line.len() as i32) / 2;
        let y = top + LINE_H * i as i32;
        for (k, &c) in line.iter().enumerate() {
            draw_glyph(scr, font, glyph(c), x + ADVANCE * k as i32, y, color);
        }
    }
}
