//! Port extras drawn over the presented frame (never into the game's own buffers).

use crate::screen::{Screen, VIEW_H, W};
use crate::text_render;
use crate::world::Room;

const YELLOW: u16 = 0xffe0;
const DARK: u16 = 0x0000;

fn plot(f: &mut [u16], x: i32, y: i32, c: u16) {
    if (0..W as i32).contains(&x) && (0..VIEW_H as i32).contains(&y) {
        f[y as usize * W + x as usize] = c;
    }
}

fn line(f: &mut [u16], (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: u16) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=n {
        let x = x0 + (x1 - x0) * i / n;
        let y = y0 + (y1 - y0) * i / n;
        plot(f, x + 1, y + 1, DARK);
        plot(f, x, y, c);
    }
}

/// Thick outline (3 px, dark edge) for the hint marker.
pub fn draw_outline(points: &[(i32, i32)], f: &mut [u16], c: u16) {
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (0, 0)] {
            line(f, (a.0 + dx, a.1 + dy), (b.0 + dx, b.1 + dy), c);
        }
    }
}

/// Outline every enabled hotspot of the room.
pub fn draw_hotspots(room: &Room, f: &mut [u16]) {
    for r in room.regions.iter().filter(|r| r.enabled && r.cursor != 0 && r.points.len() > 2) {
        for i in 0..r.points.len() {
            line(f, r.points[i], r.points[(i + 1) % r.points.len()], YELLOW);
        }
    }
}

/// A one-line message centred near the top of the view.
pub fn draw_toast(font: &[u8], lut: &[u16; 256], text: &[u8], f: &mut [u16]) {
    let l = text_render::layout(text, 60);
    let mut s = Screen { back: f.to_vec(), ..Default::default() };
    s.lut = *lut;
    s.lut[0] = DARK;
    s.lut[1] = YELLOW;
    let (x, y) = ((640 - l.width) / 2, 12);
    text_render::draw(&mut s, font, &l, x, y, 1);
    f.copy_from_slice(&s.back);
}
