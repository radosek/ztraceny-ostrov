//! TMainForm game loop (0x4630a4), click handling (0x45a204), hover (0x462dd8) and the
//! inventory screen (0x463d94).

use crate::assets::Arc8;
use crate::character::Character;
use crate::game::{CHAR2, CHAR_ROOM, Game, HERO};
use crate::geom::inside;
use crate::mixer::Channel;
use crate::screen::{H, Target, VIEW_H};

const INV_X: i32 = 107;
const INV_Y: i32 = 100;
const INV_CELL: i32 = 46;
const INV_COLS: i32 = 9;

fn in_inventory_button(x: i32, y: i32) -> bool {
    (80..=243).contains(&x) && (427..=474).contains(&y)
}

fn in_menu_button(x: i32, y: i32) -> bool {
    (370..=499).contains(&x) && (427..=474).contains(&y)
}

pub enum Exit {
    NewGame,
    GameOver,
    Quit,
}

impl Game {
    /// World ctor 0x45a42c + NewGame 0x45d7ec.
    pub fn new_game(&mut self) {
        self.rooms = self.a.rooms().unwrap_or_default();
        self.flags = [0; 256];
        self.skin = 3;
        self.w16 = -1;
        self.pending = -1;
        self.game_over = false;
        self.dialog = 0;
        self.default_speaker = 0;
        self.held = -1;
        self.chars = vec![Character::default(); 40];
        for i in 2..40 {
            let c = &mut self.chars[i];
            let (lo, hi) = self.a.npcs.get(i).copied().unwrap_or((0, 0));
            c.talk_color = hi as u8;
            let anim = self.a.npc_anims.get(lo as usize).map_or(2000, |p| p.0 as i32);
            c.npc_anim = (anim != 2000).then_some(anim);
            c.is_anim_npc = true;
            let (x, y) = self.a.npc_pos.get(i).copied().unwrap_or((0, 0));
            c.x = x as i32;
            c.y = y as i32;
            c.stop();
            c.room_id = -1;
        }
        self.chars[HERO].skin = 3;
        self.chars[HERO].talk_color = 5;
        self.chars[CHAR2].skin = 0x0f;
        self.chars[CHAR2].talk_color = 0x0d;
        self.room = 0;
        let start = self.room_index(0x210).unwrap_or(0);
        self.room = start;
        let r = self.rooms[start].clone();
        self.chars[HERO].room_id = 0x210;
        self.chars[HERO].set_pos(Some(400), Some(300), Some(&r));
        if let Some(ri) = self.room_index(0x646) {
            let r2 = self.rooms[ri].clone();
            self.chars[CHAR2].room_id = 0x646;
            self.chars[CHAR2].set_pos(Some(200), Some(350), Some(&r2));
        }
        if let Ok(p) = self.a.palette(Arc8::Data, 20) {
            self.scr.set_palette(p);
        }
        for c in 2..40 {
            self.npc_open(c);
        }
        self.char_saves = vec![None; 40];
        self.load_room_palette();
        self.load_anim(0);
        self.load_anim(1);
        self.redraw_room();
        self.load_mask();
        self.redraw = true;
    }

    /// Topmost enabled region of the current room containing the point.
    fn region_at(&self, x: i32, y: i32) -> Option<usize> {
        self.cur().regions.iter().rposition(|r| r.enabled && inside(&r.points, x, y))
    }

    /// 0x4570a0: scaled bounding box of the current frame, inset 16/scale × 10/scale.
    fn char_hit(&self, c: usize, px: i32, py: i32) -> bool {
        let ch = &self.chars[c];
        if ch.is_anim_npc || ch.moving {
            return false;
        }
        let Some(img) = self.a.sprite_list(ch.skin as i32).ok().and_then(|l| l.get(ch.frame().0).cloned()) else {
            return false;
        };
        let s = ch.scale;
        let w = crate::geom::dround(img.width as f64 / s);
        let h = crate::geom::dround(img.height as f64 / s);
        let (left, top) = ((ch.x - w / 2) as f64, (ch.y - h) as f64);
        let (mx, my) = (16.0 / s, 10.0 / s);
        let (px, py) = (px as f64, py as f64);
        left + mx <= px && px < left + w as f64 - mx && top + my <= py && py < top + h as f64 - my
    }

    fn char_region_at(&self, x: i32, y: i32) -> Option<i32> {
        if self.chars[CHAR2].room_id == self.cur_id() && self.char_hit(CHAR2, x, y) {
            return Some(0x801);
        }
        self.char_hit(HERO, x, y).then_some(0x800)
    }

    /// Hover 0x45b7a0: cursor id and label.
    fn hover(&self, x: i32, y: i32) -> (i32, Option<Vec<u8>>) {
        if y >= VIEW_H as i32 {
            let id = if in_inventory_button(x, y) {
                10
            } else if in_menu_button(x, y) {
                11
            } else {
                -1
            };
            return (id, None);
        }
        if let Some(ri) = self.region_at(x, y) {
            let r = &self.cur().regions[ri];
            let label = (r.label > 0).then(|| self.a.labels.get(r.label as usize).cloned()).flatten();
            return (r.cursor as i32, label);
        }
        if inside(&self.cur().walk.points, x, y) { (2, None) } else { (5, None) }
    }

    /// 0x45a204.
    async fn click(&mut self, x: i32, y: i32, walk_only: bool, item: i32) {
        let region = self.region_at(x, y).map(|r| r as i32).or_else(|| self.char_region_at(x, y));
        let cursor = match region {
            Some(r) if r >= 0x800 => {
                let cr = self.room_index(CHAR_ROOM).unwrap_or(0);
                self.rooms[cr].regions.get((r - 0x800) as usize).map_or(0, |g| g.cursor)
            }
            Some(r) => self.cur().regions[r as usize].cursor,
            None => 0,
        };
        let Some(region) = region else {
            self.hero_walk(x, y);
            return;
        };
        if cursor == 0 || (item != -1 && cursor == 7) || (walk_only && item == -1 && cursor == 2) {
            self.hero_walk(x, y);
            return;
        }
        if !self.flag(0x200) {
            let id = self.cur_id();
            let ok = cursor == 4
                || (id == 0x200 && region == 1)
                || (id == 0x204 && (region == 1 || region == 3))
                || (id == 0x205 && region == 0);
            if !ok {
                self.say(0x20b).await;
                return;
            }
        }
        if !self.start_region(region, (x, y), item)
            && let Some(l) = self.queued_say.take()
        {
            self.say(l).await;
        }
    }

    /// Cursor of a region id as the click handler sees it.
    fn region_cursor(&self, region: i32) -> u8 {
        if region >= 0x800 {
            let cr = self.room_index(CHAR_ROOM).unwrap_or(0);
            self.rooms[cr].regions.get((region - 0x800) as usize).map_or(0, |g| g.cursor)
        } else {
            self.cur().regions.get(region as usize).map_or(0, |g| g.cursor)
        }
    }

    /// Test/bot entry: use `region` (optionally with `item`) as a click would, then wait for
    /// the walk and run the script to completion. Returns false if the click is ignored.
    pub async fn act(&mut self, region: i32, item: i32) -> bool {
        let cursor = self.region_cursor(region);
        if cursor == 0 || (item != -1 && cursor == 7) {
            return false;
        }
        if !self.flag(0x200) {
            let id = self.cur_id();
            let ok = cursor == 4
                || (id == 0x200 && region == 1)
                || (id == 0x204 && (region == 1 || region == 3))
                || (id == 0x205 && region == 0);
            if !ok {
                return false;
            }
        }
        let (x, y) = self.region_center(region);
        if !self.start_region(region, (x, y), item) {
            self.queued_say = None;
            return false;
        }
        let mut guard = 0;
        while self.pending != -1 {
            self.frame();
            self.wait_tick().await;
            self.script_progress().await;
            guard += 1;
            if guard > 2000 {
                for c in &mut self.chars {
                    c.busy = false;
                    c.stop();
                }
            }
        }
        true
    }

    fn region_center(&self, region: i32) -> (i32, i32) {
        if region >= 0x800 {
            let c = &self.chars[(region - 0x800) as usize];
            return (c.x, c.y - 20);
        }
        let pts = &self.cur().regions[region as usize].points;
        let n = pts.len().max(1) as i32;
        (pts.iter().map(|p| p.0).sum::<i32>() / n, pts.iter().map(|p| p.1).sum::<i32>() / n)
    }

    /// 0x45a8a8: once per tick, run the pending script when its walkers have arrived.
    pub async fn script_progress(&mut self) {
        if self.pending == -1 {
            return;
        }
        if self.chars.iter().any(|c| c.busy && c.moving) {
            return;
        }
        for c in &mut self.chars {
            c.busy = false;
        }
        let region = self.pending;
        self.room_changed = false;
        let n = self.exec(region).await;
        if n == 0 {
            let l = 0x206 + self.rng.below(4) as i32;
            self.say(l).await;
        }
        self.pending = -1;
        if self.room_changed || (self.cur_id() == 0x40a && !self.flag(0x40a)) {
            self.run_enter().await;
        }
    }

    /// 0x45d7ac: run the room's Enter script.
    pub async fn run_enter(&mut self) {
        loop {
            self.room_changed = false;
            if !self.start_region(-2, (0, 0), -1) {
                break;
            }
            self.exec(-2).await;
            self.pending = -1;
            if !self.room_changed || self.game_over {
                break;
            }
        }
    }

    /// Inventory screen 0x463d94. Returns the held item or −1.
    pub async fn inventory(&mut self) -> i32 {
        self.hide_overlays();
        let saved_back = self.scr.back.clone();
        let mut selected: i32 = -1;
        let a = self.a.clone();
        let icons = a.sprite_list(0x11).ok();
        let draw = |g: &mut Self, selected: i32| {
            g.cursor_hide();
            g.tip_hide(false);
            if let Ok(img) = a.image(Arc8::Data, 0x13, 0) {
                g.scr.blit(Target::Back, &img, 0, 0, true, H);
            }
            if let Some(icons) = &icons {
                for (i, &it) in g.chars[HERO].items.clone().iter().enumerate() {
                    if it as i32 == selected {
                        continue;
                    }
                    if let Some(img) = icons.get(it as usize) {
                        let (cx, cy) = (INV_X + (i as i32 % INV_COLS) * INV_CELL, INV_Y + (i as i32 / INV_COLS) * INV_CELL);
                        g.scr.blit(Target::Back, img, cx, cy, true, H);
                    }
                }
            }
        };
        self.cursor.item = None;
        self.cursor.base = 0;
        draw(self, selected);
        self.clear_input();
        self.reset_base();
        let mut label: Option<i32> = None;
        let result = loop {
            self.music_tick();
            self.cursor_hide();
            self.tip_hide(false);
            if label.is_some() {
                self.tip_show(false);
            }
            self.cursor_animate();
            self.cursor_show();
            self.present();
            self.wait_tick().await;
            let (mx, my) = (self.input.mouse_x, self.input.mouse_y);
            let slot = {
                let (dx, dy) = (mx - INV_X, my - INV_Y);
                (dx >= 0 && dy >= 0 && dx / INV_CELL < INV_COLS).then(|| (dy / INV_CELL) * INV_COLS + dx / INV_CELL)
            };
            let items = self.chars[HERO].items.clone();
            let under = slot.and_then(|s| items.get(s as usize).copied()).map(|i| i as i32);
            self.cursor_hide();
            self.tip_hide(false);
            label = under.filter(|&i| i != selected);
            if let Some(t) = label.and_then(|i| a.items.get(i as usize)) {
                let t = t.clone();
                self.tip_set(false, &t, mx + 10, my + 10);
            }
            if self.input.esc() {
                break -1;
            }
            if self.input.right {
                break selected;
            }
            if self.input.left {
                match (under, selected) {
                    (Some(i), -1) => selected = i,
                    (Some(i), s) if i == s => selected = -1,
                    (Some(i), s) => {
                        if self.combine(s, i).await {
                            break -1;
                        }
                        draw(self, selected);
                    }
                    (None, _) => selected = -1,
                }
                self.cursor.item = (selected >= 0).then_some(selected);
                draw(self, selected);
            }
        };
        self.cursor_hide();
        self.tip_hide(false);
        self.scr.back = saved_back;
        self.cursor.item = (result >= 0).then_some(result);
        self.redraw = true;
        self.clear_input();
        self.base = self.clock.now().saturating_sub(35);
        result
    }

    async fn combine(&mut self, a: i32, b: i32) -> bool {
        const COMBOS: [(i32, i32, u8, bool, Option<i32>); 6] = [
            (0x18, 0x0b, 0x0c, true, None),
            (0x0f, 0x0d, 0x0e, false, None),
            (0x50, 0x51, 0x52, true, None),
            (0x5d, 0x5c, 0x5f, true, None),
            (0x3d, 0x36, 0x3e, true, None),
            (0x42, 0x47, 0x43, true, Some(0x44d)),
        ];
        for (x, y, r, both, flag) in COMBOS {
            if (a, b) == (x, y) || (a, b) == (y, x) {
                if let Some(f) = flag
                    && !self.flag(f)
                {
                    self.say(0x412).await;
                    return false;
                }
                let items = &mut self.chars[HERO].items;
                let remove = |items: &mut Vec<u8>, v: i32| {
                    if let Some(p) = items.iter().position(|&i| i as i32 == v) {
                        items.remove(p);
                    }
                };
                remove(items, if both { x } else { y });
                if both {
                    remove(items, y);
                }
                items.push(r);
                return true;
            }
        }
        let l = 0x200 + self.rng.below(6) as i32;
        self.say(l).await;
        false
    }

    /// Hover 0x462dd8: cursor shape, label text and position, panel hover id.
    fn update_hover(&mut self) -> i32 {
        let (mx, my) = (self.input.mouse_x, self.input.mouse_y);
        let (cursor, label) = self.hover(mx, my);
        self.cursor.base = if (1..=7).contains(&cursor) { cursor } else { 0 };
        self.cursor.item = (self.held >= 0).then_some(self.held);
        match label {
            Some(t) => self.tip_set(false, &t, mx + 10, my + 10),
            None => self.tip_set(false, b"", 0, 0),
        }
        if (10..=11).contains(&cursor) { cursor } else { -1 }
    }

    fn talking_now(&self) -> bool {
        self.dialog > 0 || self.any_talking()
    }

    /// One game session (outer loop body of 0x4630a4).
    pub async fn run_session(&mut self, intro: bool) -> Exit {
        self.io.borrow_mut().mixer.stop_all();
        self.new_game();
        if intro {
            self.play_full(0x200, 0, true, 0, 0x300).await;
            self.video("Titulky.avi", 0x1f9).await;
        }
        self.start_music_set(0);
        self.redraw_chars(true);
        self.run_enter().await;
        self.resume().await
    }

    /// The per-tick main loop, entered after a new game or a load.
    pub async fn resume(&mut self) -> Exit {
        self.redraw = true;
        self.reset_base();
        let mut was_talking = !self.talking_now();
        let mut panel_hover = -2;
        let mut hover_at = (-1, -1);
        loop {
            self.music_tick();
            let talk = self.talking_now();
            self.hide_overlays();
            let at = (self.input.mouse_x, self.input.mouse_y);
            if talk != was_talking || at != hover_at || panel_hover == -2 {
                hover_at = at;
                let h = self.update_hover();
                if h != panel_hover {
                    panel_hover = h;
                    self.draw_panel(h);
                }
            }
            was_talking = talk;
            if self.redraw {
                self.redraw_chars(true);
                self.draw_panel(panel_hover);
                self.redraw = false;
            }
            self.frame();
            if !talk {
                self.tip_show(false);
                self.cursor_animate();
                self.cursor_show();
            } else {
                self.tip_show(true);
            }
            self.present();
            self.wait_tick().await;
            if self.held >= 0 && !self.chars[HERO].items.contains(&(self.held as u8)) {
                self.held = -1;
            }
            if self.quitting() {
                return Exit::Quit;
            }
            let (mx, my) = (self.input.mouse_x, self.input.mouse_y);
            if talk {
                if self.dialog > 0 {
                    if self.input.esc() {
                        self.dialog = 0;
                        self.stop_all_talk();
                    } else if self.input.left || self.input.right {
                        self.tip_hide(true);
                        self.dialog_advance();
                    }
                } else if self.input.skip() {
                    self.stop_all_talk();
                }
            } else if self.input.left {
                if my < VIEW_H as i32 {
                    let walk_only = self.hover(mx, my).0 == 2;
                    let held = self.held;
                    self.hide_overlays();
                    self.click(mx, my, walk_only, held).await;
                } else if in_inventory_button(mx, my) {
                    self.held = self.inventory().await;
                } else if in_menu_button(mx, my) {
                    self.hide_overlays();
                    match self.menu(true, false).await {
                        crate::ui::MenuResult::New => return Exit::NewGame,
                        crate::ui::MenuResult::Quit => return Exit::Quit,
                        _ => {}
                    }
                }
                panel_hover = -2;
            } else if self.input.right {
                if self.held >= 0 {
                    self.held = -1;
                } else {
                    self.held = self.inventory().await;
                }
                panel_hover = -2;
            } else if self.input.key_down && self.input.key == b'I' as u16 {
                self.held = self.inventory().await;
                panel_hover = -2;
            }
            self.script_progress().await;
            if self.game_over {
                self.video("Titulky.avi", 0x1f9).await;
                return Exit::GameOver;
            }
            if self.io.borrow().quit {
                return Exit::Quit;
            }
        }
    }

    /// 0x464c0c: AVI with its FX.dat soundtrack; click or ESC skips.
    pub async fn video(&mut self, name: &str, fx: i32) {
        let Some(mut v) = self.a.videos.get(name).and_then(|d| crate::video::Video::open(d).ok()) else { return };
        self.io.borrow_mut().mixer.set_paused(Channel::Music, true);
        self.play(Channel::Fx, Arc8::Fx, fx);
        self.hide_overlays();
        let saved = self.scr.back.clone();
        self.clear_input();
        let t0 = self.clock.now();
        let mut i = 0u64;
        while v.advance() {
            for (d, s) in self.scr.back.iter_mut().zip(v.rgb.chunks_exact(3)) {
                *d = ((s[0] as u16 >> 3) << 11) | ((s[1] as u16 >> 2) << 5) | (s[2] as u16 >> 3);
            }
            self.present();
            i += 1;
            self.clock.until(t0 + i * 1000 / crate::video::FPS as u64).await;
            self.poll_input();
            if self.input.left || self.input.esc() || self.io.borrow().quit {
                break;
            }
        }
        self.stop(Channel::Fx);
        self.io.borrow_mut().mixer.set_paused(Channel::Music, false);
        self.scr.back = saved;
        self.redraw = true;
        self.clear_input();
        self.reset_base();
    }
}
