//! Blocking routines: speech, dialogs, waits, scripted walks, animation players
//! (ENGINE_A_dialog.md, ENGINE_C_ui.md §C1–C4). Each runs its own 70 ms (or per-frame) loop.

use crate::assets::Arc8;
use crate::flc;
use crate::game::{CHAR2, DEFAULT_FRAME_MS, Game, HERO};
use crate::mixer::Channel;
use crate::screen::{VIEW_H, W};
use std::rc::Rc;

impl Game {
    /// The translation selected in the settings, loaded on first use.
    pub(crate) fn translation(&mut self) -> Option<Rc<crate::lang::Lang>> {
        let code = self.io.borrow().settings.language.clone();
        if code == "cs" {
            return None;
        }
        if self.lang.as_ref().is_none_or(|(c, _)| *c != code) {
            let text = self.io.borrow().files.get(&crate::lang::file(&code)).map(|b| String::from_utf8_lossy(b).into_owned());
            self.lang = text.map(|t| (code.clone(), Rc::new(crate::lang::Lang::parse(&t))));
        }
        self.lang.as_ref().map(|(_, l)| l.clone())
    }

    pub(crate) fn text(&mut self, i: i32) -> Vec<u8> {
        let i = i.max(0) as usize;
        self.translation().and_then(|l| l.texts.get(&i).cloned()).or_else(|| self.a.texts.get(i).cloned()).unwrap_or_default()
    }

    /// Port UI text in the selected language (Czech, or English for any translation).
    pub(crate) fn ui(&self, cs: &str, en: &str) -> Vec<u8> {
        crate::reader::to_cp1250(if self.io.borrow().settings.language == "cs" { cs } else { en })
    }

    pub(crate) fn item_name(&mut self, i: usize) -> Option<Vec<u8>> {
        self.translation().and_then(|l| l.items.get(&i).cloned()).or_else(|| self.a.items.get(i).cloned())
    }

    pub(crate) fn label_name(&mut self, i: usize) -> Option<Vec<u8>> {
        self.translation().and_then(|l| l.labels.get(&i).cloned()).or_else(|| self.a.labels.get(i).cloned())
    }

    pub(crate) fn char_index(&self, c: i32) -> usize {
        (c.max(0) as usize).min(39)
    }

    pub(crate) fn any_talking(&self) -> bool {
        self.chars.iter().any(|c| c.talking)
    }

    /// 0x45dd20.
    pub(crate) fn stop_all_talk(&mut self) {
        self.stop(Channel::Speech);
        for c in &mut self.chars {
            c.talking = false;
        }
    }

    /// TCharacter.Say 0x4577d0.
    fn char_say(&mut self, c: usize, text: Vec<u8>, alt: Vec<u8>, speech: i32) {
        if text.is_empty() && speech <= 0 {
            return;
        }
        let mut speech = speech;
        let mut s = text;
        {
            let ch = &mut self.chars[c];
            if !ch.is_anim_npc {
                if ch.dir == 2 {
                    ch.turn(3);
                }
                if ch.dir == 6 {
                    ch.turn(5);
                }
                ch.stop();
            }
            if ch.skin == 0x0c {
                if !s.is_empty() {
                    s = alt;
                }
                if speech != 0 {
                    speech = 0x1f9;
                }
            }
            ch.talking = true;
            ch.talk_ticks = 0;
        }
        self.tip_set(true, &s, 0, 0);
        if !s.is_empty() {
            let h = self.sprite_height(c);
            let ch = &self.chars[c];
            let l = &self.speech.layout;
            let (x, y) = (ch.x - l.width / 2, ch.y - h - l.height - 17);
            self.tip_set(true, &s, x, y);
            self.speech.color = self.chars[c].talk_color;
            self.chars[c].talk_duration = (s.len() as i32).max(15);
        }
        if speech > 0 {
            self.play(Channel::Speech, Arc8::Speech, speech);
            self.chars[c].talk_duration = 0;
        }
        if let Some(mut a) = self.npc_anims[c].clone() {
            a.pos = a.header.start();
            self.npc_anims[c] = Some(a);
        }
    }

    /// SayLine 0x45d908.
    pub(crate) fn say_line(&mut self, line: i32) {
        let rec = self.a.lines.get(line.max(0) as usize).copied().unwrap_or_default();
        let text = if rec.text != 0 { rec.text as i32 } else { line };
        let mut speech = if rec.speech != 0 { rec.speech as i32 } else { line };
        if !self.speech_enabled() {
            speech = 0;
        }
        let spk = self.char_index(if rec.speaker != 0 { rec.speaker as i32 } else { self.default_speaker });
        if !self.cfg.subtitles() && self.speech_enabled() && speech != 0 {
            self.char_say(spk, Vec::new(), Vec::new(), speech);
        } else {
            let (t, alt) = (self.text(text), self.text(0x1f9));
            self.char_say(spk, t, alt, speech);
        }
    }

    fn raw_speaker(&self, line: i32) -> usize {
        self.char_index(self.a.lines.get(line.max(0) as usize).map_or(0, |l| l.speaker as i32))
    }

    /// Common prologue of the blocking routines: hide cursor and hover label, redraw chars.
    fn prep(&mut self) {
        self.cursor_hide();
        self.tip_hide(false);
        self.redraw_chars(true);
        self.redraw = false;
    }

    /// One drawn tick of a speech loop.
    fn speech_frame(&mut self) {
        self.music_tick();
        self.tip_hide(true);
        self.frame();
        self.tip_show(true);
        self.present();
    }

    /// Say 0x45da88.
    pub async fn say(&mut self, line: i32) {
        self.ctx = "say";
        self.prep();
        self.duck(true);
        self.say_line(line);
        self.reset_base();
        loop {
            self.speech_frame();
            if !self.any_talking() {
                break;
            }
            self.wait_tick().await;
            if self.input.skip() {
                self.stop_all_talk();
                break;
            }
        }
        self.tip_hide(true);
        let spk = self.raw_speaker(line);
        self.npc_settle(spk);
        self.duck(false);
        self.clear_input();
        self.reset_base();
    }

    /// Advance 0x45da10.
    pub(crate) fn dialog_advance(&mut self) {
        if self.dialog > 0 {
            self.dialog_line += 1;
            let last = self.a.dialogs.get(self.dialog as usize).map_or(0, |d| d.1 as i32);
            if self.dialog_line > last {
                self.dialog = 0;
                self.stop_all_talk();
            } else {
                self.say_line(self.dialog_line);
            }
        }
    }

    /// Dialog 0x45dd90.
    pub async fn run_dialog(&mut self, n: i32) {
        self.ctx = "dialog";
        self.prep();
        self.duck(true);
        let first = self.a.dialogs.get(n.max(0) as usize).map_or(0, |d| d.0 as i32);
        self.dialog = n;
        self.dialog_line = first - 1;
        self.dialog_advance();
        self.reset_base();
        let mut done = self.dialog == 0;
        while !done {
            self.music_tick();
            self.tip_hide(true);
            if !self.any_talking() {
                let spk = self.raw_speaker(self.dialog_line);
                self.npc_settle(spk);
                self.dialog_advance();
                done = self.dialog == 0;
            }
            self.frame();
            self.tip_show(true);
            self.present();
            if done {
                break;
            }
            self.wait_tick().await;
            if self.input.skip() {
                self.tip_hide(true);
                let spk = self.raw_speaker(self.dialog_line);
                self.stop(Channel::Speech);
                self.chars[spk].talking = false;
                self.npc_settle(spk);
                self.dialog_advance();
                done = self.dialog == 0;
            }
            if self.input.esc() {
                self.dialog = 0;
                self.stop_all_talk();
                done = true;
            }
        }
        self.tip_hide(true);
        self.duck(false);
        self.clear_input();
        self.reset_base();
    }

    fn anim_cycle_done(&self, slot: usize) -> bool {
        self.anims[slot].as_ref().is_none_or(|a| a.header.frames <= 1 || a.pos.frame == 0)
    }

    /// 0x45f81c: mode 0 = wait `ticks` (skippable), 1/2 = until back anim 1/2 completes a cycle.
    pub async fn delay(&mut self, ticks: i32, mode: i32) {
        self.ctx = "delay";
        self.prep();
        if mode > 0 && self.anim_cycle_done(mode as usize - 1) {
            return;
        }
        self.reset_base();
        let mut count = 0;
        loop {
            self.music_tick();
            self.frame();
            self.present();
            self.wait_tick().await;
            if mode == 0 && self.input.skip() {
                break;
            }
            count += 1;
            if mode == 0 && count >= ticks {
                break;
            }
            if mode > 0 {
                let slot = mode as usize - 1;
                if self.anims[slot].as_ref().is_none_or(|a| a.pos.frame <= 1) {
                    break;
                }
            }
        }
        self.clear_input();
        self.reset_base();
    }

    /// StaticWalk 0x45e2cc.
    pub async fn static_walk(&mut self, x: i32, y: i32, dir: i32, c: i32) {
        self.ctx = "static_walk";
        let c = self.char_index(c);
        self.prep();
        if let Some(r) = self.room_index(self.chars[c].room_id).map(|i| self.rooms[i].clone()) {
            self.chars[c].goto(&r, x, y, dir as i8);
        }
        self.reset_base();
        while self.chars[c].moving {
            self.music_tick();
            self.frame();
            self.present_and_wait().await;
            if self.input.skip() {
                self.hide_chars();
                let r = self.room_index(self.chars[c].room_id).map(|i| self.rooms[i].clone());
                self.chars[c].set_pos(Some(x), Some(y), r.as_ref());
                self.chars[c].set_dir(dir as i8);
                self.show_chars();
                break;
            }
        }
        self.clear_input();
        self.reset_base();
    }

    /// StaticTurn 0x45e510.
    pub async fn static_turn(&mut self, dir: i32, c: i32) {
        self.ctx = "static_turn";
        let c = self.char_index(c);
        self.prep();
        self.chars[c].turn(dir as i8);
        self.reset_base();
        while self.chars[c].moving && self.chars[c].dir != self.chars[c].target_dir {
            self.music_tick();
            self.frame();
            self.present();
            self.wait_tick().await;
            if self.input.skip() {
                self.hide_chars();
                self.chars[c].set_dir(dir as i8);
                self.show_chars();
                break;
            }
        }
        self.clear_input();
        self.reset_base();
    }

    /// PlayPartial 0x45ac50. `mode` decides what happens to the characters (0x45ad47..0x45ae93):
    ///
    /// | mode | hero    | companion | typical use                                         |
    /// |------|---------|-----------|-----------------------------------------------------|
    /// | 0–2  | hidden  | visible   | the anim draws the hero itself (stairs, tying vine) |
    /// | 3    | visible | visible   | someone else acts, both stay (dialogue anims)       |
    /// | 4    | visible | hidden    |                                                     |
    /// | 5–9  | hidden  | hidden    | the anim draws both                                 |
    /// | 10   | visible | visible   | like 3, and the anim is also painted into the room  |
    ///
    /// Visible characters are erased and redrawn around every frame. Mode 1 first stamps the
    /// current frame of back anim 1 into the room; mode 2 waits for back anim 1 to finish its
    /// cycle. Script ops: `PlayPartial anim delay fx` (mode 0) and
    /// `PlayPartialX anim delay fx mode`.
    pub async fn play_partial(&mut self, idx: i32, delay: i32, mode: i32, fx: i32) {
        self.ctx = "partial";
        if std::env::var_os("OSTROV_DEBUG").is_some() {
            eprintln!("partial {idx} mode {mode} fx {fx} room {:#x} t={}", self.cur_id(), self.clock.now());
        }
        let delay = if delay == 0 { DEFAULT_FRAME_MS } else { delay as u64 };
        if mode == 2 {
            self.delay(0, 1).await;
        }
        self.cursor_hide();
        self.tip_hide(false);
        let a = self.a.clone();
        let d = a.raw(Arc8::Partial, idx);
        let Ok(h) = flc::header(d) else { return };
        if fx > 0 {
            self.play(Channel::Fx, Arc8::Fx, fx);
        }
        // 0x45ad47..0x45ae93: hide hero and companion, then show the hero again only for
        // modes 3/4/10 and the companion only for modes <= 3 or 10. In other modes the anim
        // itself draws them.
        let keep = |g: &Game, c: usize| {
            g.chars[c].room_id == g.cur_id()
                && match c {
                    HERO => matches!(mode, 3 | 4 | 10),
                    CHAR2 => mode <= 3 || mode == 10,
                    _ => true,
                }
        };
        self.hide_chars();
        for c in self.order() {
            if keep(self, c) {
                self.show_char(c);
            }
        }
        if mode == 1 {
            self.stamp_anim(0);
        }
        let mut pos = h.start();
        self.reset_base();
        let mut skipping = false;
        let kept: Vec<usize> = self.order().into_iter().filter(|&c| keep(self, c)).collect();
        for _ in 0..h.frames {
            self.music_tick();
            for &c in kept.iter().rev() {
                self.hide_char(c);
            }
            let mut sink = self.sink(true, mode == 10, 0);
            let _ = flc::decode_frame(d, &mut pos, false, &mut sink);
            for &c in &kept {
                self.show_char(c);
            }
        if !skipping {
                self.present();
                self.wait_ms(delay).await;
                if self.input.esc() {
                    skipping = true;
                }
            }
        }
        if skipping {
            self.present();
        }
        self.stop(Channel::Fx);
        // The anim's last frame (often the hero mid-action) is only on the screen, not in the
        // background: rebuild from the background on the next tick.
        self.redraw = true;
        self.clear_input();
        self.reset_base();
    }

    /// PlayFull 0x45b2f0.
    pub async fn play_full(&mut self, idx: i32, delay: i32, stop_music: bool, mode: i32, fx: i32) {
        self.ctx = "full";
        let delay = if delay == 0 { DEFAULT_FRAME_MS } else { delay as u64 };
        self.cursor_hide();
        self.tip_hide(false);
        let a = self.a.clone();
        let d = a.raw(Arc8::Anim, idx);
        let Ok(h) = flc::header(d) else { return };
        let saved_palette = self.scr.palette;
        let panel: Vec<u16> = self.scr.back[VIEW_H * W..].to_vec();
        if mode != 1 {
            self.scr.back[VIEW_H * W..].fill(0);
        }
        if mode == 0 {
            self.scr.clear(480);
            self.present();
        }
        let y_off = if mode == 1 {
            0x38
        } else if self.io.borrow().settings.center_cutscenes {
            30
        } else {
            0
        };
        if stop_music {
            self.io.borrow_mut().mixer.set_paused(Channel::Music, true);
        }
        if fx > 0 {
            self.play(Channel::Fx, Arc8::Fx, fx);
        }
        let mut pos = h.start();
        self.reset_base();
        for _ in 0..h.frames {
            let mut sink = self.sink(true, false, y_off);
            let _ = flc::decode_frame(d, &mut pos, true, &mut sink);
            self.present();
            self.wait_ms(delay).await;
            if self.input.esc() {
                break;
            }
        }
        self.stop(Channel::Fx);
        self.io.borrow_mut().mixer.set_paused(Channel::Music, false);
        if mode == 0 {
            self.scr.clear(VIEW_H);
        }
        if mode != 1 {
            self.scr.back[VIEW_H * W..].copy_from_slice(&panel);
        }
        self.scr.set_palette(saved_palette);
        // The film covered the view; put the room back now, before the script continues
        // (a following delta-only Partial anim is authored against the room, not the film).
        self.redraw_chars(true);
        self.redraw = true;
        self.clear_input();
        self.reset_base();
    }

    /// Meanwhile 0x45fbe0.
    pub async fn meanwhile(&mut self, t: i32) {
        self.cursor_hide();
        self.tip_hide(false);
        let panel: Vec<u16> = self.scr.back[VIEW_H * W..].to_vec();
        self.scr.back[VIEW_H * W..].fill(0);
        self.scr.clear(480);
        let text = self.text(t);
        let l = crate::text_render::layout(&text, self.wrap);
        let (x, y) = ((640 - l.width) / 2, (420 - l.height) / 2);
        self.tip_set(false, &text, x, y);
        self.tip_show(false);
        self.present();
        let ticks = (2 * text.len() as i32).max(30);
        self.reset_base();
        for _ in 0..ticks {
            self.music_tick();
            self.wait_tick().await;
            if self.input.skip() {
                break;
            }
            self.present();
        }
        self.tip_hide(false);
        self.scr.back[VIEW_H * W..].copy_from_slice(&panel);
        self.redraw_chars(true);
        self.redraw = true;
        self.clear_input();
        self.reset_base();
    }

    pub(crate) async fn special_fx(&mut self, n: i32) {
        match n {
            1 => {
                let won = Box::pin(crate::minigame::cannon(self)).await;
                let region = if won { 10 } else { 11 };
                if self.start_region(region, (0, 0), -1) {
                    Box::pin(self.exec(region)).await;
                }
            }
            2..=4 => self.start_music_set(n - 1),
            _ => {}
        }
    }
}
