//! TWorld (Data.dat 0x30): rooms, regions and their scripts, plus the dialogue and NPC tables.

use crate::reader::{Eof, Reader};

#[derive(Clone, Debug)]
pub struct Region {
    pub points: Vec<(i32, i32)>,
    pub enabled: bool,
    pub cursor: u8,
    pub script: Vec<i16>,
    pub image: i16,
    pub image_type: u8,
    pub x: i16,
    pub y: i16,
    pub label: i16,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub id: i32,
    pub name: Vec<u8>,
    pub regions: Vec<Region>,
    /// Walk polygon; its script is the room's Enter script.
    pub walk: Region,
    pub bg: i16,
    pub bg_type: u8,
    pub anim1: i16,
    pub anim2: i16,
    pub waypoints: Vec<(i32, i32)>,
    pub pal: i16,
    pub scale_a: i16,
    pub scale_b: i16,
    pub fg: i32,
    pub fg_y1: i32,
    pub fg_y2: i32,
}

fn region(r: &mut Reader) -> Result<Region, Eof> {
    let n = r.i32()?;
    let points = (0..n).map(|_| Ok((r.i32()?, r.i32()?))).collect::<Result<_, Eof>>()?;
    let enabled = r.u8()? != 0;
    let cursor = r.u8()?;
    r.i16()?;
    let nb = r.i32()? as usize;
    let script = r.bytes(nb)?.chunks_exact(2).map(|w| i16::from_le_bytes([w[0], w[1]])).collect();
    Ok(Region {
        points,
        enabled,
        cursor,
        script,
        image: r.i16()?,
        image_type: r.u8()?,
        x: r.i16()?,
        y: r.i16()?,
        label: r.i16()?,
    })
}

fn room(r: &mut Reader) -> Result<Room, Eof> {
    let id = r.i32()?;
    let name = r.xor_str()?;
    let n = r.i32()?;
    let regions = (0..n).map(|_| region(r)).collect::<Result<_, _>>()?;
    let walk = region(r)?;
    let bg = r.i16()?;
    let bg_type = r.u8()?;
    let anim1 = r.i16()?;
    let anim2 = r.i16()?;
    r.bytes(48)?;
    let nw = r.i32()?;
    let waypoints = (0..nw).map(|_| Ok((r.i32()?, r.i32()?))).collect::<Result<_, Eof>>()?;
    Ok(Room {
        id,
        name,
        regions,
        walk,
        bg,
        bg_type,
        anim1,
        anim2,
        waypoints,
        pal: r.i16()?,
        scale_a: r.i16()?,
        scale_b: r.i16()?,
        fg: r.i32()?,
        fg_y1: r.i32()?,
        fg_y2: r.i32()?,
    })
}

pub fn rooms(d: &[u8]) -> Result<Vec<Room>, Eof> {
    let mut r = Reader::new(d);
    let n = r.i32()?;
    (0..n).map(|_| room(&mut r)).collect()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Line {
    pub speaker: i16,
    pub text: i16,
    pub speech: i16,
}

/// Data.dat 0x23.
pub fn lines(d: &[u8]) -> Result<Vec<Line>, Eof> {
    let mut r = Reader::new(d);
    let n = r.i32()?;
    (0..n).map(|_| Ok(Line { speaker: r.i16()?, text: r.i16()?, speech: r.i16()? })).collect()
}

/// Data.dat 0x24: dialog → inclusive line range.
pub fn dialogs(d: &[u8]) -> Result<Vec<(i16, i16)>, Eof> {
    let mut r = Reader::new(d);
    let n = r.i32()?;
    (0..n).map(|_| Ok((r.i16()?, r.i16()?))).collect()
}

/// Data.dat 0x25/0x26/0x27: 40 × u16 pairs.
pub fn pairs(d: &[u8]) -> Result<Vec<(u16, u16)>, Eof> {
    let mut r = Reader::new(d);
    let n = r.i32()?;
    (0..n).map(|_| Ok((r.u16()?, r.u16()?))).collect()
}

/// Script opcode argument counts (table 0x468b50).
pub const ARGC: [usize; 42] = [
    0, 1, 4, 3, 3, 2, 3, 2, 3, 2, 2, 2, 0, 1, 4, 1, 1, 2, 2, 3, 1, 2, 5, 3, 3, 0, 2, 0, 1, 2, 4, 0, 4, 2, 4, 2, 1, 1, 3,
    4, 2, 2,
];
