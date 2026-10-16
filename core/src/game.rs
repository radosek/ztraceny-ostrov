//! TWorld + TMainForm state: flags, rooms, audio, script interpreter.
//! Drawing lives in render.rs, blocking routines in blocking.rs, the main loop in session.rs.
//! Addresses refer to Ostrov.exe; see docs/ENGINE*.md.

use crate::assets::{Arc8, Assets};
use crate::character::{Character, NONE_DIR};
use crate::config::Config;
use crate::flc::{Header, Pos};
use crate::geom::inside;
use crate::input::Input;
use crate::mixer::{Channel, Mixer};
use crate::rng::Rng;
use crate::sched::Clock;
use crate::screen::{SavedRect, Screen, VIEW_H, W};
use crate::text_render::Layout;
use crate::world::{ARGC, Room};
use std::{cell::RefCell, rc::Rc};

pub const TICK_MS: u64 = 70;
pub const DEFAULT_FRAME_MS: u64 = 0x72;
pub const CHAR_ROOM: i32 = 0x7ff;
pub const HERO: usize = 0;
pub const CHAR2: usize = 1;

/// What the frontend exchanges with the running game.
#[derive(Default)]
pub struct Io {
    pub input: Input,
    /// Last presented frame (RGB565, 640×480) and a counter bumped on every present.
    pub frame: Vec<u16>,
    pub frames: u64,
    pub audio: Vec<i16>,
    pub mixer: Mixer,
    /// Files the game writes (saves, Main.cfg): (name, bytes). The frontend persists them.
    pub writes: Vec<(String, Vec<u8>)>,
    /// Files the game may read (saves). Filled by the frontend.
    pub files: std::collections::HashMap<String, Vec<u8>>,
    pub quit: bool,
    /// Current room id at the last present (for tools and tests).
    pub room: i32,
    /// Port extras; the game writes `settings.cfg` when they change, the frontend reads them.
    pub settings: crate::settings::Settings,
}

pub type IoRef = Rc<RefCell<Io>>;

/// An FLC player slot: background anims (world +0x120/+0x124) and NPC anims.
#[derive(Clone)]
pub struct AnimSlot {
    pub archive: Arc8,
    pub index: i32,
    pub header: Header,
    pub pos: Pos,
}

#[derive(Clone, Default)]
pub struct Cursor {
    pub visible: bool,
    pub base: i32,
    pub counter: i32,
    pub item: Option<i32>,
    pub save: Option<SavedRect>,
}

/// TToolTip instance.
#[derive(Clone)]
pub struct Tip {
    pub layout: Layout,
    pub color: u8,
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub save: Option<SavedRect>,
}

impl Tip {
    pub fn new(color: u8) -> Self {
        Self {
            layout: crate::text_render::layout(b"", 40),
            color,
            x: 0,
            y: 0,
            visible: false,
            save: None,
        }
    }
}

pub struct Game {
    pub a: Rc<Assets>,
    pub clock: Clock,
    pub io: IoRef,
    pub scr: Screen,
    pub rooms: Vec<Room>,
    pub chars: Vec<Character>,
    /// Save-unders of drawn characters (+0x104) and whether they are shown (+0x100).
    pub char_saves: Vec<Option<SavedRect>>,
    pub npc_anims: Vec<Option<AnimSlot>>,
    pub flags: [u8; 256],
    pub room: usize,
    pub skin: i16,
    pub w16: i16,
    pub pending: i32,
    pub pc: i32,
    pub blocking: bool,
    pub room_changed: bool,
    pub game_over: bool,
    pub dialog: i32,
    pub dialog_line: i32,
    pub default_speaker: i32,
    pub music_set: i32,
    pub music_pos: i32,
    pub anims: [Option<AnimSlot>; 2],
    pub cfg: Config,
    pub rng: Rng,
    pub held: i32,
    /// Tick base 0x470304.
    pub base: u64,
    pub input: Input,
    pub cursor: Cursor,
    /// 0x468ca4 hover label (colour 1) and 0x468ca8 speech bubble.
    pub hover: Tip,
    pub speech: Tip,
    /// Tooltip wrap width 0x468ca0.
    pub wrap: usize,
    /// 0x469894 / 0x469840: full redraw requested.
    pub redraw: bool,
    /// "That doesn't work" line chosen by StartRegion, spoken by the caller.
    pub queued_say: Option<i32>,
    /// Save file chosen in the Load dialog.
    pub menu_file: Option<String>,
    /// The last menu loaded a save.
    pub menu_loaded: bool,
    /// Bot/test mode: every blocking routine is skipped as if ESC was pressed.
    pub auto: bool,
    /// Draw the hotspot overlay on the next present (main loop only).
    pub show_hotspots: bool,
    /// Bot mode but still drawing (replays that dump frames).
    pub auto_draw: bool,
    /// The click that started the current script was a double click: walks run.
    pub run_next: bool,
    /// Debug label of the routine currently presenting frames.
    pub ctx: &'static str,
    /// Loaded translation and its code.
    pub lang: Option<(String, Rc<crate::lang::Lang>)>,
    /// Waits return immediately (hint search inside a running game).
    pub instant: bool,
    /// Hotspot to outline for a hint, until the given time.
    pub hint_mark: Option<(i32, i32, u64)>,
    /// Port extra: saves taken just before the player's last actions, with what they did, for
    /// F12 bug reports.
    pub recent: std::collections::VecDeque<(String, Vec<u8>)>,
    /// Character positions at the start of the last tick (smooth motion interpolates from them).
    pub prev_pos: Vec<(i32, i32)>,
    /// Region under the mouse when the hover label was last placed.
    pub hover_region_id: i32,
    /// Short message shown on top of the game (quick save etc.) until the given time.
    pub toast: Option<(Vec<u8>, u64)>,
}

/// 0x459714 playlists: [variant][set] = tracks.
const PLAYLISTS: [[&[i32]; 4]; 2] = [
    [
        &[7, 8, 21, 24, 3, 4, 6, 25, 13, 23, 9, 5, 10],
        &[19, 10, 24, 9, 8, 20, 6, 16, 3, 11, 13],
        &[12, 2, 11, 24, 7, 14, 4, 18, 1, 15],
        &[13, 14, 10, 23, 12, 1, 25, 9, 16, 2, 22, 20, 19, 17],
    ],
    [
        &[6, 3, 4, 12, 13, 24, 7, 8, 10],
        &[4, 9, 10, 24, 13, 8, 6, 15, 3, 12],
        &[10, 12, 13, 24, 7, 2, 3, 5, 1],
        &[9, 13, 15, 2, 4, 1, 5, 6, 8, 7],
    ],
];

/// Ambient table 0x468f38: (room id, FX2.dat index).
pub(crate) const AMBIENT: [(i32, i32); 48] = [
    (0x200, 781), (0x201, 782), (0x202, 775), (0x204, 776), (0x206, 778), (0x20a, 768), (0x20b, 769), (0x20c, 783),
    (0x20e, 778), (0x20f, 768), (0x602, 772), (0x603, 774), (0x604, 774), (0x601, 772), (0x600, 772), (0x605, 772),
    (0x606, 772), (0x636, 773), (0x63e, 772), (0x63f, 773), (0x640, 772), (0x641, 772), (0x642, 773), (0x643, 773),
    (0x644, 773), (0x645, 773), (0x646, 773), (0x647, 772), (0x648, 771), (0x649, 771), (0x64a, 775), (0x64b, 778),
    (0x64c, 779), (0x64e, 771), (0x3, 779), (0x2, 778), (0x405, 781), (0x40e, 779), (0x403, 771), (0x404, 776),
    (0x406, 782), (0x407, 782), (0x408, 779), (0x40a, 770), (0x40b, 783), (0x40c, 778), (0x40d, 771), (0x40f, 782),
];

impl Game {
    /// `seed`: Delphi RandSeed after Randomize (UTC ms of day). The copy-protection draw that
    /// the original makes at startup (Random(1500), 0x4593fe) is replayed so sequences match.
    pub fn new(a: Rc<Assets>, clock: Clock, io: IoRef, cfg: Config, seed: u32) -> Self {
        let mut rng = Rng(seed);
        rng.below(1500);
        let mut g = Self {
            a,
            clock,
            io,
            scr: Screen::default(),
            rooms: Vec::new(),
            chars: vec![Character::default(); 40],
            char_saves: vec![None; 40],
            npc_anims: vec![None; 40],
            flags: [0; 256],
            room: 0,
            skin: 3,
            w16: -1,
            pending: -1,
            pc: 0,
            blocking: false,
            room_changed: false,
            game_over: false,
            dialog: 0,
            dialog_line: 0,
            default_speaker: 0,
            music_set: 0,
            music_pos: 0,
            anims: [None, None],
            cfg,
            rng,
            held: -1,
            base: 0,
            input: Input::default(),
            cursor: Cursor::default(),
            hover: Tip::new(1),
            speech: Tip::new(0),
            wrap: 40,
            redraw: true,
            queued_say: None,
            menu_file: None,
            menu_loaded: false,
            auto: false,
            show_hotspots: false,
            auto_draw: false,
            run_next: false,
            ctx: "",
            lang: None,
            instant: false,
            hint_mark: None,
            recent: Default::default(),
            prev_pos: vec![(0, 0); 40],
            hover_region_id: -2,
            toast: None,
        };
        g.apply_volumes();
        g
    }

    // ---- flags, rooms, characters ------------------------------------------------------

    pub fn flag(&self, f: i32) -> bool {
        let f = f as usize & 0x7ff;
        self.flags[f >> 3] & (1 << (f & 7)) != 0
    }

    pub fn set_flag(&mut self, f: i32, v: bool) {
        let f = f as usize & 0x7ff;
        if v {
            self.flags[f >> 3] |= 1 << (f & 7);
        } else {
            self.flags[f >> 3] &= !(1 << (f & 7));
        }
    }

    pub fn room_index(&self, id: i32) -> Option<usize> {
        self.rooms.iter().position(|r| r.id == id)
    }

    pub fn cur(&self) -> &Room {
        &self.rooms[self.room]
    }

    pub fn cur_id(&self) -> i32 {
        self.rooms[self.room].id
    }

    fn char_room(&self, c: usize) -> Option<&Room> {
        self.room_index(self.chars[c].room_id).map(|i| &self.rooms[i])
    }


    // ---- input / time ----------------------------------------------------------------------

    /// ProcessMessages: take the frontend's latched input.
    pub fn poll_input(&mut self) {
        let mut io = self.io.borrow_mut();
        self.input = io.input.clone();
        io.input.clear();
        io.input.now = self.clock.now();
        if io.quit || self.auto {
            self.input.key_down = true;
            self.input.key = crate::input::KEY_ESC;
        }
        drop(io);
        // F12 works everywhere, including during animations and speech.
        if self.input.key_down && self.input.key == 0x7b && !self.auto {
            self.input.key_down = false;
            self.bug_report();
            let m = self.ui("Hlášení chyby uloženo (složka Bugs)", "Bug report saved (Bugs folder)");
            self.toast(&m);
        }
    }

    /// The frontend asked to quit: every loop unwinds as if ESC was pressed.
    pub fn quitting(&self) -> bool {
        self.io.borrow().quit
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.io.borrow_mut().input.clear();
    }

    /// Flip: mix audio up to now and publish the back buffer.
    pub fn present(&mut self) {
        if self.auto && !self.auto_draw {
            return;
        }
        if std::env::var_os("OSTROV_DEBUG").is_some()
            && self.chars[HERO].room_id == self.cur_id()
            && let Some((x0, y0, x1, y1)) = self.char_box(HERO)
        {
            let mut same = true;
            let mut any = false;
            for y in y0.max(0)..y1.min(420) {
                for x in x0.max(0)..x1.min(640) {
                    let o = y as usize * 640 + x as usize;
                    any = true;
                    if self.scr.back[o] != self.scr.bg[o] {
                        same = false;
                    }
                }
            }
            if any && same {
                eprintln!("hero invisible room {:#x} t={} ctx={} talking={}", self.cur_id(), self.clock.now(), self.ctx, self.chars.iter().any(|c| c.talking));
            }
        }
        let back = self.scr.back.clone();
        self.publish(back);
    }

    /// Hand a finished frame (plus port overlays) to the frontend and mix audio up to now.
    pub fn publish(&mut self, buf: Vec<u16>) {
        let now = self.clock.now();
        let mut io = self.io.borrow_mut();
        let Io { mixer, audio, frame, frames, .. } = &mut *io;
        mixer.render_until(now, audio);
        *frame = buf;
        if self.show_hotspots {
            crate::extras::draw_hotspots(&self.rooms[self.room], frame);
        }
        if let Some((room, region, until)) = self.hint_mark
            && now < until
            && room == self.rooms[self.room].id
            && let Some(r) = self.rooms[self.room].regions.get(region as usize)
        {
            if (until - now) / 250 % 2 == 0 {
                crate::extras::draw_outline(&r.points, frame, 0xffe0);
            }
        }
        if let Some((text, until)) = &self.toast {
            if now < *until {
                crate::extras::draw_toast(&self.a.font, &self.scr.lut, text, frame);
            }
        }
        *frames += 1;
        io.room = self.rooms.get(self.room).map_or(-1, |r| r.id);
    }

    /// Wait for the next 70 ms tick boundary and collect input (the inner busy loop of every
    /// blocking routine).
    pub async fn wait_tick(&mut self) {
        if self.instant {
            self.poll_input();
            return;
        }
        self.base += TICK_MS;
        self.clock.until(self.base).await;
        self.poll_input();
    }

    /// Absolute pacing used by animation players: wait until base + ms.
    pub async fn wait_ms(&mut self, ms: u64) {
        if self.instant {
            self.poll_input();
            return;
        }
        self.base += ms;
        self.clock.until(self.base).await;
        self.poll_input();
    }

    pub fn reset_base(&mut self) {
        self.base = self.clock.now();
    }

    // ---- audio -------------------------------------------------------------------------------

    pub fn apply_volumes(&mut self) {
        let mut io = self.io.borrow_mut();
        io.mixer.volume = [self.cfg.music_volume(), self.cfg.fx_volume(), self.cfg.speech_volume(), self.cfg.fx_volume()];
    }

    pub fn speech_enabled(&self) -> bool {
        self.cfg.speech_volume() > 0
    }

    pub fn play(&self, ch: Channel, a: Arc8, i: i32) {
        if self.auto && !self.auto_draw {
            return;
        }
        if let Some(s) = self.a.sound(a, i) {
            self.io.borrow_mut().mixer.play(ch, s, false);
        }
    }

    pub fn stop(&self, ch: Channel) {
        self.io.borrow_mut().mixer.stop(ch);
    }

    pub fn playing(&self, ch: Channel) -> bool {
        self.io.borrow().mixer.playing(ch)
    }

    pub(crate) fn duck(&self, on: bool) {
        if self.speech_enabled() {
            self.io.borrow_mut().mixer.duck = on;
        }
    }

    /// 0x459714, once per tick in every loop.
    pub fn music_tick(&mut self) {
        if self.cfg.music_volume() == 0 || self.playing(Channel::Music) {
            return;
        }
        let list = PLAYLISTS[(self.cfg.music_variant() as usize).min(1)][(self.music_set as usize).min(3)];
        let t = list[self.music_pos as usize % list.len()];
        self.music_pos = (self.music_pos + 1) % list.len() as i32;
        self.play(Channel::Music, Arc8::Music, t);
    }

    /// 0x45f7c0: switch playlist and restart.
    pub fn start_music_set(&mut self, set: i32) {
        self.music_set = set;
        self.music_pos = 0;
        self.stop(Channel::Music);
        self.music_tick();
    }

    pub fn next_track(&mut self) {
        self.stop(Channel::Music);
        self.music_tick();
    }

    /// Ambient part of 0x45e140: one roll per matching table entry, only if none is playing.
    pub(crate) fn ambient_tick(&mut self) {
        if self.playing(Channel::Ambient) {
            return;
        }
        let id = self.cur_id();
        for &(room, fx) in AMBIENT.iter() {
            if room == id && self.rng.below(100000) < 600 {
                self.play(Channel::Ambient, Arc8::Fx2, fx);
            }
        }
    }

    // ---- room changes ----------------------------------------------------------------------

    /// ChangeRoom 0x45a9b8.
    pub fn change_room(&mut self, id: i32) {
        if id == self.cur_id() {
            return;
        }
        self.force_room(id);
    }

    /// ChangeRoom without the same-room check (loading a save).
    pub fn force_room(&mut self, id: i32) {
        let Some(ri) = self.room_index(id) else { return };
        let old_pal = self.cur().pal;
        self.room = ri;
        self.chars[HERO].room_id = id;
        let new_pal = self.cur().pal;
        // 0x45aa4b's 0x0e/0x12 exception compares room ids already overwritten: dead code.
        if old_pal != new_pal && !(0x64f..=0x654).contains(&id)
        {
            self.scr.back[..VIEW_H * W].fill(0);
        }
        if self.flag(0x7ff) {
            let (x, y, d) = (self.chars[HERO].x, self.chars[HERO].y, self.chars[HERO].dir);
            let r = self.rooms[ri].clone();
            let c = &mut self.chars[CHAR2];
            c.room_id = id;
            c.set_pos(Some(x), Some(y), Some(&r));
            c.set_dir(d);
        }
        let r = self.rooms[ri].clone();
        for c in &mut self.chars {
            if c.room_id == id {
                c.rescale(Some(&r));
            }
        }
        self.load_room_palette();
        self.load_anim(0);
        self.load_anim(1);
        self.redraw_room();
        self.load_mask();
        // Save-unders hold the previous room; start the new one from its clean background.
        for c in &mut self.char_saves {
            *c = None;
        }
        if !self.auto || self.auto_draw {
            self.scr.bg_to_back();
        }
        self.room_changed = true;
    }

    // ---- script interpreter --------------------------------------------------------------

    fn region_script(&self, region: i32) -> Vec<i16> {
        if region == 0x800 || region == 0x801 {
            let ri = self.room_index(CHAR_ROOM);
            return ri
                .and_then(|i| self.rooms[i].regions.get((region - 0x800) as usize))
                .map(|r| r.script.clone())
                .unwrap_or_default();
        }
        if region == -2 {
            return self.cur().walk.script.clone();
        }
        self.cur().regions.get(region.max(0) as usize).map(|r| r.script.clone()).unwrap_or_default()
    }

    fn cond(&self, c: i16) -> bool {
        match c {
            0 => true,
            c if c > 0 => self.flag(c as i32),
            c => !self.flag(-(c as i32)),
        }
    }

    /// StartRegion 0x45c294: choose block, apply item, prescan walks. Returns false if aborted.
    pub fn start_region(&mut self, region: i32, click: (i32, i32), item: i32) -> bool {
        let s = self.region_script(region);
        self.pending = region;
        self.pc = 0;
        let mut i = 0;
        while i < s.len() {
            let op = (s[i] & 0x7f) as usize;
            if op == 14 && s[i + 1..i + 5].iter().all(|&c| self.cond(c)) {
                self.pc = (i + 5) as i32;
                break;
            }
            i += 1 + ARGC.get(op).copied().unwrap_or(0);
        }
        if item != -1 {
            let mut i = self.pc as usize;
            let mut found = false;
            while i < s.len() {
                let op = (s[i] & 0x7f) as usize;
                if op == 14 {
                    break;
                }
                if op == 1 && s[i + 1] as i32 == item {
                    self.pc = (i + 2) as i32;
                    found = true;
                    break;
                }
                i += 1 + ARGC.get(op).copied().unwrap_or(0);
            }
            if !found {
                self.pending = -1;
                let l = 0x200 + self.rng.below(6) as i32;
                self.queued_say = Some(l);
                return false;
            }
        }
        self.blocking = false;
        let mut i = self.pc as usize;
        while i < s.len() {
            let op = (s[i] & 0x7f) as usize;
            if op == 1 || op == 14 {
                break;
            }
            let a = &s[i + 1..i + 1 + ARGC[op]];
            match op {
                2 => {
                    let c = self.char_index(a[3] as i32);
                    if let Some(r) = self.char_room(c).cloned() {
                        self.chars[c].goto(&r, a[0] as i32, a[1] as i32, a[2] as i8);
                        self.chars[c].busy = true;
                        if c == HERO && self.run_next {
                            self.chars[c].run = true;
                        }
                    }
                }
                25 => {
                    self.hero_walk(click.0, click.1);
                    if self.run_next {
                        self.chars[HERO].run = true;
                    }
                }
                3 | 4 | 0x22 | 0x27 | 0x29 => self.blocking = true,
                _ => {}
            }
            i += 1 + ARGC[op];
        }
        true
    }

    /// 0x45c70c: hero walks to a clicked point. Returns whether a walk started.
    pub fn hero_walk(&mut self, x: i32, y: i32) -> bool {
        let room = self.cur().clone();
        let h = &self.chars[HERO];
        let s = h.scale;
        if !((h.x - x).abs() as f64 > 14.0 / s || (h.y - y).abs() as f64 > 7.0 / s) {
            return false;
        }
        let walk = &room.walk.points;
        let y2 = (y + 2.max((10.0 / s) as i32)).min(419);
        let target = if inside(walk, x, y2) {
            (x, y2)
        } else if inside(walk, x, y + 2) && inside(walk, x, y) {
            (x, y)
        } else {
            return false;
        };
        let h = &mut self.chars[HERO];
        h.final_dir = NONE_DIR;
        h.walk_to(&room, target.0, target.1, true);
        true
    }

    /// 0x45c8c8: run ops from PC until ItemLabel/FlagLabel/end. Returns the op count.
    pub async fn exec(&mut self, region: i32) -> usize {
        let s = self.region_script(region);
        let mut n = 0;
        let mut i = self.pc as usize;
        while i < s.len() {
            let op = (s[i] & 0x7f) as usize;
            if op == 1 || op == 14 || op >= ARGC.len() {
                break;
            }
            let a: Vec<i32> = s[i + 1..i + 1 + ARGC[op]].iter().map(|&v| v as i32).collect();
            i += 1 + ARGC[op];
            self.pc = i as i32;
            n += 1;
            self.op(op, &a, region).await;
            if self.game_over {
                break;
            }
        }
        n
    }

    fn room_mut(&mut self, id: i32) -> Option<&mut Room> {
        let i = self.room_index(id)?;
        Some(&mut self.rooms[i])
    }

    async fn op(&mut self, op: usize, a: &[i32], region: i32) {
        let is_cur = |g: &Self, id: i32| g.cur_id() == id;
        match op {
            0 | 2 | 24 | 25 => {}
            3 => self.play_partial(a[0], a[1], 0, a[2]).await,
            4 => self.play_full(a[0], a[1], false, 1, a[2]).await,
            5 => {
                if a[1] == 0 {
                    self.change_room(a[0]);
                } else {
                    let c = self.char_index(a[1]);
                    self.chars[c].room_id = a[0];
                    let r = self.char_room(c).cloned();
                    self.chars[c].rescale(r.as_ref());
                }
            }
            6 => {
                let c = self.char_index(a[2]);
                let r = self.char_room(c).cloned();
                self.chars[c].set_pos(Some(a[0]), Some(a[1]), r.as_ref());
            }
            7 => {
                let c = self.char_index(a[1]);
                self.chars[c].set_dir(a[0] as i8);
            }
            8 => {
                if let Some(r) = self.room_mut(a[2]) {
                    r.bg = a[0] as i16;
                    r.bg_type = (a[1] != 0) as u8;
                }
                if is_cur(self, a[2]) {
                    self.redraw_room();
                }
            }
            9 => {
                let c = self.char_index(a[1]);
                let it = &mut self.chars[c].items;
                if it.len() < 128 {
                    it.push(a[0] as u8);
                }
            }
            10 | 11 => {
                let en = op == 10;
                let cur = is_cur(self, a[1]);
                // 0x45cec5 / 0x45cf4b: nothing happens unless the state changes.
                let changed = match self.room_mut(a[1]).and_then(|r| r.regions.get_mut(a[0] as usize)) {
                    Some(r) if r.enabled != en => {
                        r.enabled = en;
                        true
                    }
                    _ => false,
                };
                if cur && changed {
                    self.region_changed(a[0] as usize, en);
                }
            }
            12 => {
                if region >= 0 && region < 0x800 {
                    let ri = region as usize;
                    let was = self.rooms[self.room].regions.get(ri).is_some_and(|r| r.enabled);
                    if was {
                        self.rooms[self.room].regions[ri].enabled = false;
                        self.region_changed(ri, false);
                    }
                }
            }
            13 => self.say(a[0]).await,
            15 => self.set_flag(a[0], true),
            16 => self.set_flag(a[0], false),
            17 => {
                let c = self.char_index(a[1]);
                self.chars[c].turn(a[0] as i8);
            }
            18 => {
                let c = self.char_index(a[1]);
                let it = &mut self.chars[c].items;
                if let Some(p) = it.iter().position(|&i| i as i32 == a[0]) {
                    it.remove(p);
                }
            }
            19 => {
                if let Some(r) = self.room_mut(a[2]).and_then(|r| r.regions.get_mut(a[1] as usize)) {
                    r.cursor = a[0] as u8;
                }
            }
            20 => {
                self.skin = a[0] as i16;
                self.chars[HERO].skin = a[0] as u8;
            }
            21 => {
                let cur = is_cur(self, a[1]);
                if cur {
                    self.delay(0, 1).await;
                }
                if let Some(r) = self.room_mut(a[1]) {
                    r.anim1 = a[0] as i16;
                }
                if cur {
                    // 0x45d1a7: RedrawRoom, then load anim 1.
                    self.redraw_room();
                    self.load_anim(0);
                }
            }
            22 => {
                let cur = is_cur(self, a[4]);
                let ri = a[3] as usize;
                let old = self
                    .room_index(a[4])
                    .and_then(|i| self.rooms[i].regions.get(ri))
                    .map(|r| (r.image, r.image_type, r.x, r.y, r.enabled));
                // 0x45d1ff: unchanged image and position → nothing.
                if old.is_some_and(|(oi, _, ox, oy, _)| (oi as i32, ox as i32, oy as i32) == (a[0], a[1], a[2])) {
                    return;
                }
                if let Some(r) = self.room_mut(a[4]).and_then(|r| r.regions.get_mut(ri)) {
                    r.image = a[0] as i16;
                    r.image_type = 0;
                    r.x = a[1] as i16;
                    r.y = a[2] as i16;
                }
                if let (true, Some((oi, ot, ox, oy, true))) = (cur, old) {
                    self.erase_image(oi as i32, ot, ox as i32, oy as i32);
                    self.draw_region_image(ri);
                }
            }
            23 => {
                if let Some(r) = self.room_mut(a[2]).and_then(|r| r.regions.get_mut(a[1] as usize)) {
                    r.label = a[0] as i16;
                }
            }
            26 => {
                if let Some(r) = self.room_mut(a[1]) {
                    r.fg = a[0];
                }
                if is_cur(self, a[1]) {
                    self.load_mask();
                }
            }
            27 => self.game_over = true,
            28 => self.run_dialog(a[0]).await,
            29 => {
                if let Some(r) = self.room_mut(a[1]) {
                    r.pal = a[0] as i16;
                }
                if is_cur(self, a[1]) {
                    self.load_room_palette();
                    self.redraw_room();
                }
            }
            30 => {
                let c = self.char_index(a[3]);
                let r = self.char_room(c).cloned();
                self.chars[c].set_pos(Some(a[0]), Some(a[1]), r.as_ref());
                self.chars[c].set_dir(a[2] as i8);
            }
            31 => self.chars[HERO].items.clear(),
            32 => self.static_walk(a[0], a[1], a[2], a[3]).await,
            33 => self.static_turn(a[0], a[1]).await,
            // PlayPartialX anim delay fx mode (the data reads e.g. `580 0 0 3`: mode 3).
            34 => self.play_partial(a[0], a[1], a[3], a[2]).await,
            35 => self.default_speaker = a[0],
            36 => self.special_fx(a[0]).await,
            37 => self.delay(a[0], 0).await,
            38 => {
                let cur = is_cur(self, a[1]);
                if cur {
                    // 0x45d502: signed `<= 2` delays, 3 stamps anim 1, anything else anim 2.
                    match a[2] {
                        i32::MIN..=2 => self.delay(0, a[2].max(0)).await,
                        3 => self.stamp_anim(0),
                        _ => self.stamp_anim(1),
                    }
                }
                let second = a[2] == 2 || a[2] > 3;
                if let Some(r) = self.room_mut(a[1]) {
                    if second {
                        r.anim2 = a[0] as i16;
                    } else {
                        r.anim1 = a[0] as i16;
                    }
                }
                if cur {
                    self.load_anim(second as usize);
                    self.redraw_room();
                }
            }
            39 => self.play_full(a[0], a[1], a[3] >= 3, a[3] % 3, a[2]).await,
            40 => {
                let (t, c) = (self.char_index(a[0]), self.char_index(a[1]));
                let ang = crate::geom::angle(self.chars[c].x, self.chars[c].y, self.chars[t].x, self.chars[t].y);
                let d = match ang {
                    151..=210 => 5,
                    211..=330 => 4,
                    331.. | 0..=30 => 3,
                    _ => 0,
                };
                self.static_turn(d, a[1]).await;
            }
            41 => self.meanwhile(a[0]).await,
            _ => {}
        }
    }



}
