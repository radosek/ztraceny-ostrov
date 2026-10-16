//! Save games (writer 0x45be70, reader 0x45bc1c), ENGINE.md §11.

use crate::character::Character;
use crate::game::Game;
use crate::reader::{Eof, Reader};

pub const THUMB_W: usize = 76;
pub const THUMB_H: usize = 56;

fn w_i32(o: &mut Vec<u8>, v: i32) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn w_i16(o: &mut Vec<u8>, v: i16) {
    o.extend_from_slice(&v.to_le_bytes());
}

/// 0x46763c: 76×56 nearest sample of the 640×420 view.
pub fn thumbnail(back: &[u16]) -> Vec<u16> {
    let mut t = Vec::with_capacity(THUMB_W * THUMB_H);
    for y in 0..THUMB_H {
        for x in 0..THUMB_W {
            t.push(back[(y * 420 / THUMB_H) * 640 + x * 640 / THUMB_W]);
        }
    }
    t
}

fn write_char(o: &mut Vec<u8>, c: &Character) {
    w_i32(o, c.room_id);
    w_i32(o, c.x);
    w_i32(o, c.y);
    for v in [c.seg_start.0, c.seg_start.1, c.seg_end.0, c.seg_end.1, c.queued.0, c.queued.1, c.dest.0, c.dest.1] {
        w_i32(o, v);
    }
    o.push(c.dir as u8);
    o.push(c.target_dir as u8);
    o.push(c.final_dir as u8);
    w_i32(o, c.walk_frame);
    o.extend_from_slice(&c.scale.to_le_bytes());
    o.push(c.moving as u8);
    o.push(c.just_stopped);
    o.push(c.busy as u8);
    o.push(c.skin);
    o.push(c.via_waypoint as u8);
    o.extend(c.visited.iter().map(|&v| v as u8));
    o.push(c.is_anim_npc as u8);
    let mut items = [0u8; 128];
    items[..c.items.len().min(128)].copy_from_slice(&c.items[..c.items.len().min(128)]);
    o.extend_from_slice(&items);
    w_i32(o, c.items.len() as i32);
}

fn read_char(r: &mut Reader, c: &mut Character) -> Result<(), Eof> {
    c.room_id = r.i32()?;
    c.x = r.i32()?;
    c.y = r.i32()?;
    c.seg_start = (r.i32()?, r.i32()?);
    c.seg_end = (r.i32()?, r.i32()?);
    c.queued = (r.i32()?, r.i32()?);
    c.dest = (r.i32()?, r.i32()?);
    c.dir = r.u8()? as i8;
    c.target_dir = r.u8()? as i8;
    c.final_dir = r.u8()? as i8;
    c.walk_frame = r.i32()?;
    c.scale = r.f64()?;
    c.moving = r.u8()? != 0;
    c.just_stopped = r.u8()?;
    c.busy = r.u8()? != 0;
    c.skin = r.u8()?;
    c.via_waypoint = r.u8()? != 0;
    for v in c.visited.iter_mut() {
        *v = r.u8()? != 0;
    }
    c.is_anim_npc = r.u8()? != 0;
    let items = r.bytes(128)?.to_vec();
    let n = (r.i32()?.clamp(0, 128)) as usize;
    c.items = items[..n].to_vec();
    Ok(())
}

pub fn header(d: &[u8]) -> Result<(Vec<u8>, Vec<u16>), Eof> {
    let mut r = Reader::new(d);
    let n = r.u16()? as usize;
    let name = r.bytes(n)?.to_vec();
    let thumb = r.bytes(THUMB_W * THUMB_H * 2)?.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
    Ok((name, thumb))
}

impl Game {
    pub fn save(&self, name: &[u8], thumb: &[u16]) -> Vec<u8> {
        let mut o = Vec::with_capacity(0x10000);
        o.extend_from_slice(&(name.len() as u16).to_le_bytes());
        o.extend_from_slice(name);
        for &p in thumb {
            o.extend_from_slice(&p.to_le_bytes());
        }
        for c in &self.chars {
            write_char(&mut o, c);
        }
        for room in &self.rooms {
            w_i16(&mut o, room.bg);
            o.push(room.bg_type);
            w_i16(&mut o, room.anim1);
            w_i16(&mut o, room.anim2);
            w_i32(&mut o, room.fg);
            w_i32(&mut o, room.fg_y1);
            w_i32(&mut o, room.fg_y2);
            w_i16(&mut o, room.pal);
            for g in &room.regions {
                o.push(g.enabled as u8);
                o.push(g.cursor);
                w_i16(&mut o, g.x);
                w_i16(&mut o, g.y);
                w_i16(&mut o, g.image);
                o.push(g.image_type);
                w_i16(&mut o, g.label);
            }
        }
        o.extend_from_slice(&self.flags);
        w_i32(&mut o, self.cur_id());
        w_i16(&mut o, self.skin);
        w_i16(&mut o, self.w16);
        w_i32(&mut o, self.pending);
        w_i32(&mut o, self.pc);
        o.push(self.blocking as u8);
        o.push(self.game_over as u8);
        w_i32(&mut o, self.music_set);
        w_i32(&mut o, self.default_speaker);
        o
    }

    pub fn load(&mut self, d: &[u8]) -> Result<(), Eof> {
        self.new_game();
        let mut r = Reader::new(d);
        let n = r.u16()? as usize;
        r.bytes(n)?;
        r.bytes(THUMB_W * THUMB_H * 2)?;
        for c in self.chars.iter_mut() {
            read_char(&mut r, c)?;
        }
        for room in self.rooms.iter_mut() {
            room.bg = r.i16()?;
            room.bg_type = r.u8()?;
            room.anim1 = r.i16()?;
            room.anim2 = r.i16()?;
            room.fg = r.i32()?;
            room.fg_y1 = r.i32()?;
            room.fg_y2 = r.i32()?;
            room.pal = r.i16()?;
            for g in room.regions.iter_mut() {
                g.enabled = r.u8()? != 0;
                g.cursor = r.u8()?;
                g.x = r.i16()?;
                g.y = r.i16()?;
                g.image = r.i16()?;
                g.image_type = r.u8()?;
                g.label = r.i16()?;
            }
        }
        self.flags.copy_from_slice(r.bytes(256)?);
        let room = r.i32()?;
        self.skin = r.i16()?;
        self.w16 = r.i16()?;
        self.pending = r.i32()?;
        self.pc = r.i32()?;
        self.blocking = r.u8()? != 0;
        self.game_over = r.u8()? != 0;
        let set = r.i32()?;
        self.default_speaker = r.i32()?;
        self.room = usize::MAX;
        self.room = self.room_index(room).unwrap_or(0);
        let id = self.cur_id();
        self.anims = [None, None];
        self.force_room(id);
        self.start_music_set(set);
        Ok(())
    }
}
