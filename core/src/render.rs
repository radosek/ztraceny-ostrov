//! Drawing as the original does it: one persistent screen, a clean background buffer, and
//! save-under rectangles for characters, tooltips and the cursor (ENGINE_D §D2–D3).

use crate::assets::Arc8;
use crate::flc::{self, Pos, Sink};
use crate::game::{AnimSlot, CHAR2, Game, HERO};
use crate::geom::dround;
use crate::gfx::{Image, SCENE_PALETTE_FIRST};
use crate::mixer::Channel;
use crate::screen::{H, Screen, Target, VIEW_H, W};
use crate::text_render;

/// Writes FLC output like 0x44f744: 16-bit pixels via the LUT, plus the 8-bit mirror.
pub(crate) struct ScreenSink<'a> {
    pub scr: &'a mut Screen,
    pub back: bool,
    pub bg: bool,
    pub y_offset: usize,
    pub palette_dirty: bool,
}

impl ScreenSink<'_> {
    fn put(&mut self, o: usize, px: &[u8], index: bool) {
        let o = o + self.y_offset * W;
        let end = (o + px.len()).min(self.scr.back.len());
        if o >= end {
            return;
        }
        let px = &px[..end - o];
        if index {
            self.scr.index[o..end].copy_from_slice(px);
        }
        for (i, &p) in px.iter().enumerate() {
            let c = self.scr.lut[p as usize];
            if self.back {
                self.scr.back[o + i] = c;
            }
            if self.bg {
                self.scr.bg[o + i] = c;
            }
        }
    }
}

impl Sink for ScreenSink<'_> {
    fn pixels(&mut self, o: usize, px: &[u8]) {
        self.put(o, px, true)
    }
    fn fill(&mut self, o: usize, n: usize, v: u8) {
        let buf = vec![v; n];
        self.put(o, &buf, true)
    }
    fn raw_row(&mut self, o: usize, px: &[u8]) {
        self.put(o, px, false)
    }
    fn color(&mut self, i: usize, rgb: [u8; 3]) {
        self.scr.palette[i] = rgb;
        self.palette_dirty = true;
    }
    /// LUT rebuild and redraw of rows 0..419 of the target from the index mirror.
    fn palette_done(&mut self) {
        if self.palette_dirty {
            let p = self.scr.palette;
            self.scr.set_palette(p);
            let off = self.y_offset * W;
            for i in off..(off + VIEW_H * W).min(self.scr.back.len()) {
                self.scr.back[i] = self.scr.lut[self.scr.index[i] as usize];
            }
            self.palette_dirty = false;
        }
    }
}

impl Game {
    pub(crate) fn sink(&mut self, back: bool, bg: bool, y_offset: usize) -> ScreenSink<'_> {
        ScreenSink { scr: &mut self.scr, back, bg, y_offset, palette_dirty: false }
    }

    pub(crate) fn image(&self, a: Arc8, i: i32, kind: u8) -> Option<Image> {
        self.a.image(a, i, kind).ok()
    }

    // ---- room -------------------------------------------------------------------------------

    /// 0x45c274: Backgnd palette, colours 56..255.
    pub fn load_room_palette(&mut self) {
        let pal = self.cur().pal as i32;
        if let Ok(p) = self.a.palette(Arc8::Backgnd, pal) {
            self.scr.load_palette_from(&p, SCENE_PALETTE_FIRST);
        }
    }

    /// 0x45c1fc: priority mask from room +0x5c.
    pub fn load_mask(&mut self) {
        if self.auto && !self.auto_draw {
            return;
        }
        let fg = self.cur().fg;
        self.scr.mask.fill(0);
        if fg > 0
            && let Some(img) = self.image(Arc8::Backgnd, fg, 0)
        {
            for y in 0..(img.height as usize).min(VIEW_H) {
                for x in 0..(img.width as usize).min(W) {
                    self.scr.mask[y * W + x] = img.pixels[y * img.width as usize + x];
                }
            }
        }
    }

    pub(crate) fn draw_region_image(&mut self, ri: usize) {
        if self.auto && !self.auto_draw {
            return;
        }
        let r = &self.rooms[self.room].regions[ri];
        let (img, kind, x, y) = (r.image as i32, r.image_type, r.x as i32, r.y as i32);
        if img > 0 {
            if let Some(i) = self.image(Arc8::Backgnd, img, kind) {
                self.scr.blit(Target::Bg, &i, x, y, true, VIEW_H);
            }
        } else if img < 0 {
            self.erase_image(img, kind, x, y);
        }
    }

    /// Restore an image's rectangle from the clean room background (copy 0x468b34).
    pub(crate) fn erase_image(&mut self, img: i32, kind: u8, x: i32, y: i32) {
        if self.auto && !self.auto_draw {
            return;
        }
        if img == 0 {
            return;
        }
        let Some(i) = self.image(Arc8::Backgnd, img.abs(), kind) else { return };
        let r = self.cur();
        let Some(bg) = self.image(Arc8::Backgnd, r.bg as i32, r.bg_type) else { return };
        let lut = self.scr.lut;
        for yy in y.max(0)..(y + i.height as i32).min(VIEW_H as i32).min(bg.height as i32) {
            for xx in x.max(0)..(x + i.width as i32).min(W as i32).min(bg.width as i32) {
                let p = bg.pixels[yy as usize * bg.width as usize + xx as usize];
                self.scr.bg[yy as usize * W + xx as usize] = lut[p as usize];
            }
        }
    }

    pub(crate) fn region_changed(&mut self, ri: usize, enabled: bool) {
        let Some(r) = self.cur().regions.get(ri).cloned() else { return };
        if r.image == 0 {
            return;
        }
        if enabled {
            self.draw_region_image(ri);
        } else {
            self.erase_image(r.image as i32, r.image_type, r.x as i32, r.y as i32);
        }
        self.redraw = true;
    }

    /// 0x45b5b0 RedrawRoom: background, current back-anim frames, region images (highest first).
    pub fn redraw_room(&mut self) {
        if self.auto && !self.auto_draw {
            return;
        }
        let r = self.cur();
        let (bg, kind) = (r.bg as i32, r.bg_type);
        if bg > 0
            && let Some(img) = self.image(Arc8::Backgnd, bg, kind)
        {
            self.scr.blit(Target::Bg, &img, 0, 0, false, VIEW_H);
        }
        for slot in 0..2 {
            self.restamp_anim(slot);
        }
        for ri in (0..self.cur().regions.len()).rev() {
            let r = &self.cur().regions[ri];
            if r.enabled && r.image != 0 {
                self.draw_region_image(ri);
            }
        }
        self.redraw = true;
    }

    // ---- FLC slots ------------------------------------------------------------------------

    fn slot_open(&self, archive: Arc8, index: i32) -> Option<AnimSlot> {
        let d = self.a.raw(archive, index);
        let header = flc::header(d).ok()?;
        Some(AnimSlot { archive, index, header, pos: header.start() })
    }

    /// Decode the next frame of `slot` into the given buffers, wrapping like 0x44f744: after the
    /// last frame back to frame 1 if the file loops (has a ring frame), else to 0.
    pub(crate) fn slot_step(&mut self, s: &mut AnimSlot, back: bool, bg: bool) {
        let a = self.a.clone();
        let d = a.raw(s.archive, s.index);
        let mut sink = self.sink(back, bg, 0);
        let _ = flc::decode_frame(d, &mut s.pos, false, &mut sink);
        if s.pos.frame >= s.header.frames as u32 {
            s.pos = if s.header.loop_frame != 0 {
                Pos { offset: s.header.loop_frame, frame: 1 }
            } else {
                s.header.start()
            };
        }
    }

    /// Re-decode from the start up to the current frame onto the background (0x44fc10).
    fn restamp_anim(&mut self, slot: usize) {
        if self.auto && !self.auto_draw {
            return;
        }
        let Some(s) = self.anims[slot].clone() else { return };
        let mut t = AnimSlot { pos: s.header.start(), ..s.clone() };
        let n = if s.pos.frame == 0 { s.header.frames as u32 } else { s.pos.frame };
        for _ in 0..n {
            self.slot_step(&mut t, false, true);
        }
    }

    /// 0x45ba34 / 0x45ba70: open the room's back anim and stamp its first frame.
    pub fn load_anim(&mut self, slot: usize) {
        let idx = if slot == 0 { self.cur().anim1 } else { self.cur().anim2 };
        self.anims[slot] = None;
        if idx == 0 {
            return;
        }
        let Some(mut s) = self.slot_open(Arc8::Backgnd, idx.unsigned_abs() as i32) else { return };
        self.slot_step(&mut s, true, true);
        self.anims[slot] = Some(s);
    }

    /// Stamp the current frame of a back anim into the background (ChangeBackAnimX 3/4).
    pub(crate) fn stamp_anim(&mut self, slot: usize) {
        self.restamp_anim(slot);
    }

    /// 0x45e140.
    fn back_anims(&mut self) {
        for slot in 0..2 {
            let field = if slot == 0 { self.cur().anim1 } else { self.cur().anim2 };
            let Some(mut s) = self.anims[slot].clone() else { continue };
            if field > 0 || (field < 0 && (s.pos.frame != 0 || self.rng.below(100000) < 500)) {
                self.slot_step(&mut s, true, true);
            }
            if field != 0 && s.pos.frame == 1 && field == 666 {
                self.play(Channel::Fx, Arc8::Fx, 58);
            }
            self.anims[slot] = Some(s);
        }
        self.ambient_tick();
    }

    /// NPC anim 0x44fc40: if mid-cycle, play the rest so it lands on its rest frame.
    pub(crate) fn npc_settle(&mut self, c: usize) {
        if self.auto && !self.auto_draw {
            return;
        }
        let Some(mut s) = self.npc_anims[c].clone() else { return };
        if s.pos.frame != 0 && s.header.frames > 1 {
            for _ in 0..(s.header.frames as u32).saturating_sub(s.pos.frame) {
                self.slot_step(&mut s, true, true);
            }
        }
        self.npc_anims[c] = Some(s);
    }

    pub(crate) fn npc_open(&mut self, c: usize) {
        self.npc_anims[c] = self.chars[c].npc_anim.and_then(|i| self.slot_open(Arc8::Partial, i));
    }

    // ---- characters ----------------------------------------------------------------------

    pub(crate) fn skin_of(&self, c: usize) -> i32 {
        self.chars[c].skin as i32
    }

    fn char_sprite(&self, c: usize) -> Option<Image> {
        let list = self.a.sprite_list(self.skin_of(c)).ok()?;
        let (f, _) = self.chars[c].frame();
        list.get(f).cloned()
    }

    pub(crate) fn sprite_height(&self, c: usize) -> i32 {
        if self.chars[c].is_anim_npc {
            return 0;
        }
        let s = self.chars[c].scale;
        self.char_sprite(c).map_or(0, |i| dround(i.height as f64 / s))
    }

    fn depth(&self, y: i32) -> u8 {
        let r = self.cur();
        if r.fg <= 0 {
            0xff
        } else if r.fg_y1 <= 0 || y < r.fg_y1 {
            0
        } else if r.fg_y2 <= 0 || y < r.fg_y2 {
            1
        } else {
            2
        }
    }

    /// Show 0x457bec: save-under, then draw 0x456a18.
    pub(crate) fn show_char(&mut self, c: usize) {
        if self.auto && !self.auto_draw {
            return;
        }
        let ch = &self.chars[c];
        if ch.is_anim_npc || (ch.x == -255 && ch.y == -255) {
            return;
        }
        let Some(img) = self.char_sprite(c) else {
            if std::env::var_os("OSTROV_DEBUG").is_some() {
                let ch = &self.chars[c];
                eprintln!("no sprite: char {c} skin {} frame {:?} dir {} talking {} t={}", ch.skin, ch.frame(), ch.dir, ch.talking, self.clock.now());
            }
            return;
        };
        let ch = &self.chars[c];
        let s = ch.scale;
        let wr = dround(img.width as f64 / s);
        let hr2 = dround((img.height as f64 + 2.0) / s);
        self.char_saves[c] = Some(self.scr.save_rect(ch.x - wr / 2, ch.y - hr2, wr, hr2));
        let hr = dround(img.height as f64 / s);
        let skin = self.skin_of(c);
        let yb = if skin == 0x0e { ch.y - dround((ch.cycle as u32 / 3) as f64 / s) } else { ch.y };
        let (_, flip) = ch.frame();
        let depth = self.depth(ch.y);
        let (x, y) = (ch.x - wr / 2, yb - hr);
        self.scr.sprite(&img, x, y, s, flip, depth, true);
    }

    /// Screen rectangle of a character's current sprite (x0, y0, x1, y1 exclusive).
    pub(crate) fn char_box(&self, c: usize) -> Option<(i32, i32, i32, i32)> {
        let ch = &self.chars[c];
        if ch.is_anim_npc || ch.x == -255 {
            return None;
        }
        let img = self.char_sprite(c)?;
        let (w, h) = (dround(img.width as f64 / ch.scale), dround(img.height as f64 / ch.scale));
        Some((ch.x - w / 2, ch.y - h, ch.x - w / 2 + w, ch.y))
    }

    /// Share (0..1) of a character's visible pixels that an anim repaints at some point.
    pub(crate) fn char_coverage(&self, c: usize, cov: &crate::flc::Coverage) -> f64 {
        let ch = &self.chars[c];
        if ch.is_anim_npc || ch.x == -255 {
            return 0.0;
        }
        let Some(img) = self.char_sprite(c) else { return 0.0 };
        let s = ch.scale;
        let (w, h) = (dround(img.width as f64 / s), dround(img.height as f64 / s));
        let (x0, y0) = (ch.x - w / 2, ch.y - h);
        let (mut total, mut hit) = (0u32, 0u32);
        for dy in 0..h {
            for dx in 0..w {
                let sx = ((dx as f64 * s) as i32).min(img.width as i32 - 1);
                let sy = ((dy as f64 * s) as i32).min(img.height as i32 - 1);
                if img.pixels[(sy * img.width as i32 + sx) as usize] == img.transparent {
                    continue;
                }
                total += 1;
                if cov.covered(x0 + dx, y0 + dy) {
                    hit += 1;
                }
            }
        }
        if total == 0 { 0.0 } else { hit as f64 / total as f64 }
    }

    /// Hide 0x457080.
    pub(crate) fn hide_char(&mut self, c: usize) {
        if let Some(r) = self.char_saves[c].take() {
            self.scr.put_rect(&r);
        }
    }

    fn in_room(&self, c: usize) -> bool {
        self.chars[c].room_id == self.cur_id()
    }

    pub(crate) fn order(&self) -> Vec<usize> {
        if self.chars[HERO].y < self.chars[CHAR2].y { (0..40).collect() } else { (0..40).rev().collect() }
    }

    /// Erase every drawn character, including ones that left the room since they were drawn.
    pub(crate) fn hide_chars(&mut self) {
        for c in self.order().into_iter().rev() {
            self.hide_char(c);
        }
    }

    pub(crate) fn show_chars(&mut self) {
        for c in self.order() {
            if self.in_room(c) {
                self.show_char(c);
            }
        }
    }

    /// 0x45c154 RedrawChars.
    pub fn redraw_chars(&mut self, include_hero: bool) {
        if self.auto && !self.auto_draw {
            return;
        }
        self.scr.bg_to_back();
        for c in &mut self.char_saves {
            *c = None;
        }
        for c in self.order() {
            if self.in_room(c) && (include_hero || c != HERO) {
                self.show_char(c);
            }
        }
    }

    /// Talk tick 0x4585d4.
    pub(crate) fn talk_tick(&mut self, c: usize, no_anim: bool) {
        let speech = self.playing(Channel::Speech);
        let enabled = self.speech_enabled();
        let ch = &mut self.chars[c];
        if !ch.talking {
            return;
        }
        ch.talk_ticks += 1;
        ch.talk_toggle = !ch.talk_toggle;
        let step = !no_anim && ch.is_anim_npc && ch.talk_toggle;
        let stop = (enabled && !speech && ch.talk_duration == 0) || (ch.talk_duration != 0 && ch.talk_ticks > ch.talk_duration);
        if stop {
            ch.talking = false;
        }
        if step && let Some(mut s) = self.npc_anims[c].clone() {
            self.slot_step(&mut s, true, true);
            self.npc_anims[c] = Some(s);
        }
    }

    /// 0x45c010 Frame: erase characters, update them, back anims, draw characters.
    pub fn frame(&mut self) {
        self.hide_chars();
        for c in 0..40 {
            let r = self.room_index(self.chars[c].room_id).map(|i| self.rooms[i].clone());
            self.chars[c].update(r.as_ref());
            self.talk_tick(c, false);
        }
        self.back_anims();
        self.show_chars();
    }

    // ---- tooltips, cursor, panel --------------------------------------------------------

    pub(crate) fn tip_show(&mut self, speech: bool) {
        if self.auto && !self.auto_draw {
            return;
        }
        let t = if speech { &mut self.speech } else { &mut self.hover };
        if t.visible || t.layout.lines.iter().all(|l| l.is_empty()) {
            return;
        }
        t.save = Some(self.scr.save_rect(t.x, t.y, t.layout.width, t.layout.height));
        t.visible = true;
        let (l, x, y, c) = (t.layout.clone(), t.x, t.y, t.color);
        text_render::draw(&mut self.scr, &self.a.font, &l, x, y, c);
    }

    pub(crate) fn tip_hide(&mut self, speech: bool) {
        let t = if speech { &mut self.speech } else { &mut self.hover };
        t.visible = false;
        if let Some(r) = t.save.take() {
            self.scr.put_rect(&r);
        }
    }

    /// SetText + SetPos (clamped). The tip must be hidden.
    pub(crate) fn tip_set(&mut self, speech: bool, text: &[u8], x: i32, y: i32) {
        let wrap = self.wrap;
        let t = if speech { &mut self.speech } else { &mut self.hover };
        t.layout = text_render::layout(text, wrap);
        let (cx, cy) = text_render::clamp(&t.layout, x, y);
        t.x = cx;
        t.y = cy;
    }

    fn cursor_image(&self) -> Option<Image> {
        match self.cursor.item {
            Some(i) => self.a.sprite_list(0x11).ok()?.get(i as usize).cloned(),
            None => self.a.sprite_list(0x12).ok()?.get((self.cursor.base + 8 * ((2 * self.cursor.counter) / 3)) as usize).cloned(),
        }
    }

    pub(crate) fn cursor_show(&mut self) {
        if self.auto && !self.auto_draw {
            return;
        }
        if self.cursor.visible {
            return;
        }
        let Some(img) = self.cursor_image() else { return };
        let (x, y) = (self.input.mouse_x - img.width as i32 / 2, self.input.mouse_y - img.height as i32 / 2);
        self.cursor.save = Some(self.scr.save_rect(x, y, img.width as i32, img.height as i32));
        self.scr.blit(Target::Back, &img, x, y, true, H);
        self.cursor.visible = true;
    }

    pub(crate) fn cursor_hide(&mut self) {
        self.cursor.visible = false;
        if let Some(r) = self.cursor.save.take() {
            self.scr.put_rect(&r);
        }
    }

    /// Cursor animate 0x4503d4.
    pub(crate) fn cursor_animate(&mut self) {
        if self.cursor.item.is_none() {
            self.cursor.counter = (self.cursor.counter + 1) % 9;
        }
    }

    /// Bottom panel 0x463b80.
    pub fn draw_panel(&mut self, hover: i32) {
        let Ok(list) = self.a.sprite_list(0x10) else { return };
        for (k, x) in [(0usize, 0), (1, 320)] {
            let h = hover == 10 + k as i32;
            if let Some(img) = list.get(k + if h { 2 } else { 0 }) {
                self.scr.blit(Target::Back, img, x, VIEW_H as i32, false, H);
            }
        }
    }

    /// Hide everything with a save-under (cursor first, LIFO).
    pub(crate) fn hide_overlays(&mut self) {
        self.cursor_hide();
        self.tip_hide(false);
        self.tip_hide(true);
    }
}
