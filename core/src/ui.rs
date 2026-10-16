//! Custom in-surface UI (unit 0x464e55–0x467900, ENGINE_E_ui.md): game menu, confirm dialogs,
//! save/load dialog. Widgets redraw themselves on input only; panels save what is under them.

use crate::assets::Arc8;
use crate::game::Game;
use crate::gfx::Image;
use crate::mixer::Channel;
use crate::save::{THUMB_H, THUMB_W};
use crate::screen::{H, SavedRect, Target};
use crate::screen::Screen;
use crate::text_render;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuResult {
    Continue,
    Load,
    Save,
    New,
    Quit,
}

const SLOTS: [(i32, i32); 8] = [(31, 106), (121, 106), (211, 106), (301, 106), (31, 178), (121, 178), (211, 178), (301, 178)];
const TRACK_RANGE: i32 = 107;

pub fn slot_file(i: usize) -> String {
    format!("SaveGame/Game{i}.sav")
}

#[derive(Clone)]
enum Kind {
    Button { normal: Image, hover: Image, enabled: bool, visible: bool, normal_on_press: bool },
    Check { normal: Image, hover: Image, overlay: Image, enabled: bool, value: bool, radio: bool },
    Track { track: Image, thumb: Image, enabled: bool, value: i32, dragging: Option<(i32, i32)> },
    Edit { bg: Image, enabled: bool, caret_visible: bool, text: Vec<u8>, caret: usize },
    Stamp { used: bool, hovered: bool, locked: bool, grey: Vec<u16>, color: Vec<u16>, name: Vec<u8> },
}

#[derive(Clone)]
struct Widget {
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    kind: Kind,
}

impl Widget {
    fn inside(&self, mx: i32, my: i32) -> bool {
        self.x <= mx && mx < self.x + self.w && self.y <= my && my < self.y + self.h
    }
}

struct Panel {
    x: i32,
    y: i32,
    image: Image,
    widgets: Vec<Widget>,
    save: Option<SavedRect>,
}

fn grey(src: &[u16]) -> Vec<u16> {
    src.iter()
        .map(|&p| {
            let r = ((p >> 11) & 31) << 3;
            let g = ((p >> 5) & 63) << 2;
            let b = (p & 31) << 3;
            let v = (r + g + b) / 3;
            ((v >> 3) << 11) | ((v >> 2) << 5) | (v >> 3)
        })
        .collect()
}

impl Game {
    fn ui_image(&self, i: i32) -> Image {
        self.a.image(Arc8::Data, i, 0).unwrap_or(Image { width: 1, height: 1, transparent: 0, pixels: vec![0] })
    }

    fn panel(&self, img: i32) -> Panel {
        let image = self.ui_image(img);
        let x = (640 - image.width as i32) / 2;
        let y = (480 - image.height as i32) / 2;
        Panel { x, y, image, widgets: Vec::new(), save: None }
    }

    fn button(&self, p: &Panel, id: i32, x: i32, y: i32, n: i32, h: i32) -> Widget {
        let normal = self.ui_image(n);
        let hover = self.ui_image(h);
        Widget {
            id,
            x: p.x + x,
            y: p.y + y,
            w: normal.width as i32,
            h: normal.height as i32,
            kind: Kind::Button { normal, hover, enabled: true, visible: true, normal_on_press: false },
        }
    }

    fn check(&self, p: &Panel, id: i32, x: i32, y: i32, imgs: (i32, i32, i32), value: bool, radio: bool) -> Widget {
        let normal = self.ui_image(imgs.0);
        Widget {
            id,
            x: p.x + x,
            y: p.y + y,
            w: normal.width as i32,
            h: normal.height as i32,
            kind: Kind::Check { hover: self.ui_image(imgs.1), overlay: self.ui_image(imgs.2), normal, enabled: true, value, radio },
        }
    }

    fn track(&self, p: &Panel, id: i32, x: i32, y: i32, track: i32, value: u8) -> Widget {
        let track = self.ui_image(track);
        Widget {
            id,
            x: p.x + x,
            y: p.y + y,
            w: track.width as i32,
            h: track.height as i32,
            kind: Kind::Track { track, thumb: self.ui_image(0x59), enabled: true, value: value as i32, dragging: None },
        }
    }

    fn draw_widget(&mut self, w: &Widget, hover: bool) {
        let font = self.a.font.clone();
        match &w.kind {
            Kind::Button { normal, hover: hi, visible, .. } => {
                if *visible {
                    self.scr.blit(Target::Back, if hover { hi } else { normal }, w.x, w.y, false, H);
                }
            }
            Kind::Check { normal, hover: hi, overlay, enabled, value, .. } => {
                self.scr.blit(Target::Back, if hover { hi } else { normal }, w.x, w.y, false, H);
                if !*value && (*enabled || hover) {
                    self.scr.blit(Target::Back, overlay, w.x, w.y, true, H);
                }
            }
            Kind::Track { track, thumb, enabled, value, .. } => {
                self.scr.blit(Target::Back, track, w.x, w.y, false, H);
                if *enabled {
                    let px = TRACK_RANGE * value / 100;
                    self.scr.blit(Target::Back, thumb, w.x + px, w.y, true, H);
                }
            }
            Kind::Edit { bg, enabled, caret_visible, text, caret } => {
                self.scr.blit(Target::Back, bg, w.x, w.y, false, H);
                let l = text_render::Layout { lines: vec![text.clone()], width: 9 * text.len() as i32 + 1, height: 17 };
                if *enabled {
                    text_render::draw(&mut self.scr, &font, &l, w.x + 6, w.y + 4, 2);
                    if *caret_visible {
                        let c = text_render::Layout { lines: vec![b"_".to_vec()], width: 10, height: 17 };
                        text_render::draw(&mut self.scr, &font, &c, w.x + 4 + 9 * *caret as i32, w.y + 4, 1);
                    }
                } else {
                    text_render::draw(&mut self.scr, &font, &l, w.x + (w.w - 9 * text.len() as i32) / 2 - 1, w.y + 4, 2);
                }
            }
            Kind::Stamp { used, hovered, locked, grey, color, .. } => {
                if *used {
                    let src = if *hovered && !*locked { color } else { grey };
                    for r in 0..THUMB_H as i32 {
                        let o = ((w.y + r) as usize) * 640 + w.x as usize;
                        let s = r as usize * THUMB_W;
                        self.scr.back[o..o + THUMB_W].copy_from_slice(&src[s..s + THUMB_W]);
                    }
                }
            }
        }
    }

    fn open_panel(&mut self, p: &mut Panel) {
        self.cursor_hide();
        p.save = Some(self.scr.save_rect(p.x, p.y, p.image.width as i32, p.image.height as i32));
        self.scr.blit(Target::Back, &p.image, p.x, p.y, true, H);
        for w in p.widgets.clone() {
            self.draw_widget(&w, false);
        }
        self.cursor_show();
        self.present();
    }

    fn close_panel(&mut self, p: &mut Panel) {
        self.cursor_hide();
        if let Some(r) = p.save.take() {
            self.scr.put_rect(&r);
        }
        self.cursor_show();
        self.present();
    }

    /// Widget Update for every widget; returns the id of a widget that fired.
    fn update_widgets(&mut self, p: &mut Panel) -> i32 {
        let (mx, my) = (self.input.mouse_x, self.input.mouse_y);
        let (down, up) = (self.input.left, self.input.left_up);
        let mut result = 0;
        for i in 0..p.widgets.len() {
            let mut w = p.widgets[i].clone();
            let inside = w.inside(mx, my);
            let mut hover = false;
            match &mut w.kind {
                Kind::Button { enabled, visible, normal_on_press, .. } => {
                    if inside && *enabled && *visible {
                        hover = true;
                        if down {
                            result = w.id;
                            hover = !*normal_on_press;
                        }
                    }
                }
                Kind::Check { enabled, value, radio, .. } => {
                    if inside && *enabled {
                        hover = true;
                        if down {
                            if *radio {
                                if !*value {
                                    *value = true;
                                    result = w.id;
                                }
                            } else {
                                *value = !*value;
                                result = w.id;
                            }
                        }
                    }
                }
                Kind::Track { enabled, value, dragging, thumb, .. } => {
                    if *enabled && (inside || dragging.is_some()) {
                        let tw = thumb.width as i32;
                        let px = TRACK_RANGE * *value / 100;
                        if let Some((smx, spx)) = *dragging {
                            if up {
                                *dragging = None;
                            } else {
                                *value = (mx + spx - smx).clamp(0, TRACK_RANGE) * 100 / TRACK_RANGE;
                                result = w.id;
                            }
                        } else if down {
                            if (w.x + px..w.x + px + tw).contains(&mx) {
                                *dragging = Some((mx, px));
                            } else {
                                *value = (mx - w.x - tw / 2).clamp(0, TRACK_RANGE) * 100 / TRACK_RANGE;
                                result = w.id;
                            }
                        }
                    }
                }
                Kind::Edit { enabled, text, caret, .. } => {
                    if *enabled {
                        for &c in &self.input.chars {
                            if c >= 0x20 && text.len() < 16 {
                                text.insert(*caret, c);
                                *caret += 1;
                            }
                        }
                        if self.input.key_down {
                            match self.input.key {
                                0x25 => *caret = caret.saturating_sub(1),
                                0x27 => *caret = (*caret + 1).min(text.len()),
                                0x24 => *caret = 0,
                                0x23 => *caret = text.len(),
                                0x2e => {
                                    if *caret < text.len() {
                                        text.remove(*caret);
                                    }
                                }
                                0x08 => {
                                    if *caret > 0 {
                                        *caret -= 1;
                                        text.remove(*caret);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Kind::Stamp { used, hovered, locked, .. } => {
                    if !*locked {
                        if inside {
                            *hovered = true;
                            hover = *used;
                            if down {
                                result = w.id;
                            }
                        } else {
                            *hovered = false;
                        }
                    }
                }
            }
            self.draw_widget(&w, hover);
            p.widgets[i] = w;
        }
        result
    }

    /// Generic dialog loop 0x465820: input-driven, returns the widget id that ended it
    /// (handlers in `on` may also end it by returning Some).
    async fn dialog_loop(
        &mut self,
        p: &mut Panel,
        caret_blink: bool,
        mut on: impl FnMut(&mut Self, &mut Panel, i32) -> Option<i32>,
    ) -> i32 {
        self.open_panel(p);
        self.clear_input();
        self.reset_base();
        let mut blink_base = self.clock.now();
        loop {
            self.music_tick();
            self.wait_tick().await;
            if self.quitting() {
                self.close_panel(p);
                return 2;
            }
            let i = &self.input;
            let any = i.key_down || i.left || i.left_up || i.moved || i.right || !i.chars.is_empty();
            let mut blink = false;
            if caret_blink && self.clock.now() >= blink_base + 150 {
                blink_base += 150 * ((self.clock.now() - blink_base) / 150);
                blink = true;
            }
            if !any && !blink {
                continue;
            }
            self.cursor_hide();
            if blink {
                for w in p.widgets.iter_mut() {
                    if let Kind::Edit { enabled: true, caret_visible, .. } = &mut w.kind {
                        *caret_visible = !*caret_visible;
                    }
                }
            }
            let fired = self.update_widgets(p);
            let key = if self.input.key_down { self.input.key } else { 0 };
            let r = on(self, p, if fired != 0 { fired } else { -(key as i32) });
            self.cursor_animate();
            self.cursor_show();
            self.present();
            if let Some(r) = r {
                self.close_panel(p);
                self.clear_input();
                return r;
            }
        }
    }

    /// Confirm dialog 0x466cb0 (0x5a new game, 0x5b quit).
    async fn confirm(&mut self, img: i32) -> bool {
        let mut p = self.panel(img);
        p.widgets.push(self.button(&p, 1, 17, 62, 0x5c, 0x5d));
        p.widgets.push(self.button(&p, 2, 117, 62, 0x5e, 0x5f));
        let r = self
            .dialog_loop(&mut p, false, |_, _, ev| match ev {
                1 | 2 => Some(ev),
                e if e == -0x1b => Some(2),
                e if e == -0x0d => Some(1),
                _ => None,
            })
            .await;
        r == 1
    }

    /// Save/Load dialog 0x466d8c. Returns (slot file, name) on OK.
    async fn save_load(&mut self, save: bool, snapshot: &[u16]) -> Option<(String, Vec<u8>)> {
        let mut p = self.panel(if save { 0x61 } else { 0x60 });
        let mut ok = self.button(&p, 1, 245, 261, 0x62, 0x63);
        if let Kind::Button { enabled, visible, .. } = &mut ok.kind {
            *visible = save;
            *enabled = false;
        }
        p.widgets.push(ok);
        p.widgets.push(self.button(&p, 2, 304, 261, 0x64, 0x65));
        let bg = self.ui_image(0x66);
        p.widgets.push(Widget {
            id: 3,
            x: p.x + 31,
            y: p.y + 263,
            w: bg.width as i32,
            h: bg.height as i32,
            kind: Kind::Edit { bg, enabled: false, caret_visible: false, text: Vec::new(), caret: 0 },
        });
        for (i, &(x, y)) in SLOTS.iter().enumerate() {
            let hdr = self.io.borrow().files.get(&slot_file(i)).and_then(|d| crate::save::header(d).ok());
            let (used, name, color) = match hdr {
                Some((n, t)) => (true, n, t),
                None => (false, Vec::new(), vec![0; THUMB_W * THUMB_H]),
            };
            p.widgets.push(Widget {
                id: 0x100 + i as i32,
                x: p.x + x,
                y: p.y + y,
                w: THUMB_W as i32,
                h: THUMB_H as i32,
                kind: Kind::Stamp { used, hovered: false, locked: false, grey: grey(&color), color, name },
            });
        }
        let mut chosen: Option<usize> = None;
        let snap = snapshot.to_vec();
        let r = self
            .dialog_loop(&mut p, true, |g, p, ev| {
                let hovered_name = p.widgets.iter().find_map(|w| match &w.kind {
                    Kind::Stamp { used: true, hovered: true, name, .. } => Some(name.clone()),
                    _ => None,
                });
                if ev >= 0x100 {
                    let slot = (ev - 0x100) as usize;
                    let used = matches!(p.widgets[3 + slot].kind, Kind::Stamp { used: true, .. });
                    if save {
                        for w in p.widgets.iter_mut() {
                            if let Kind::Stamp { locked, .. } = &mut w.kind {
                                *locked = true;
                            }
                        }
                        if let Kind::Stamp { grey, used, .. } = &mut p.widgets[3 + slot].kind {
                            *grey = snap.clone();
                            *used = true;
                        }
                        let name = hovered_name.unwrap_or_default();
                        if let Kind::Edit { enabled, text, caret, .. } = &mut p.widgets[2].kind {
                            *enabled = true;
                            *caret = name.len();
                            *text = name;
                        }
                        if let Kind::Button { enabled, .. } = &mut p.widgets[0].kind {
                            *enabled = true;
                        }
                        chosen = Some(slot);
                        let ws = p.widgets.clone();
                        for w in &ws {
                            g.draw_widget(w, false);
                        }
                    } else if used {
                        chosen = Some(slot);
                        return Some(1);
                    }
                    return None;
                }
                match ev {
                    1 | 2 => return Some(ev),
                    e if e == -0x1b => return Some(2),
                    e if e == -0x0d => {
                        if let Kind::Button { enabled: true, visible: true, .. } = p.widgets[0].kind {
                            return Some(1);
                        }
                    }
                    _ => {}
                }
                if let Kind::Edit { enabled: false, text, .. } = &mut p.widgets[2].kind {
                    *text = hovered_name.unwrap_or_default();
                    let w = p.widgets[2].clone();
                    g.draw_widget(&w, false);
                }
                None
            })
            .await;
        if r != 1 {
            return None;
        }
        let name = match &p.widgets[2].kind {
            Kind::Edit { text, .. } => text.clone(),
            _ => Vec::new(),
        };
        chosen.map(|s| (slot_file(s), name))
    }

    /// Game menu 0x464538 (TOptions 0x465f50).
    pub async fn menu(&mut self, in_game: bool, from_autosave: bool) -> MenuResult {
        self.cursor_hide();
        self.cursor.base = 0;
        self.cursor.item = None;
        self.tip_hide(false);
        let snapshot = crate::save::thumbnail(&self.scr.back);
        if !in_game {
            self.scr.clear(480);
        }
        let mut p = self.panel(0x40);
        let cfg = self.cfg.raw;
        let mut buttons = vec![
            self.button(&p, 10, 10, 9, 0x41, 0x42),
            self.button(&p, 13, 10, 34, 0x43, 0x44),
            self.button(&p, 14, 10, 64, 0x45, 0x46),
            self.button(&p, 12, 10, 93, 0x47, 0x48),
            self.button(&p, 11, 10, 118, 0x49, 0x4a),
            self.button(&p, 15, 92, 257, 0x4b, 0x4c),
        ];
        for b in buttons.iter_mut() {
            if let Kind::Button { enabled, normal_on_press, .. } = &mut b.kind {
                match b.id {
                    10 => *enabled = in_game || from_autosave,
                    14 => *enabled = in_game,
                    _ => {}
                }
                *normal_on_press = matches!(b.id, 13 | 14);
            }
        }
        p.widgets.extend(buttons);
        p.widgets.push(self.check(&p, 19, 10, 312, (0x4d, 0x4e, 0x4f), cfg[15] != 0, false));
        p.widgets.push(self.check(&p, 21, 120, 312, (0x50, 0x51, 0x52), cfg[16] == 0, true));
        p.widgets.push(self.check(&p, 20, 180, 312, (0x53, 0x54, 0x55), cfg[16] != 0, true));
        p.widgets.push(self.track(&p, 17, 10, 178, 0x56, cfg[13]));
        p.widgets.push(self.track(&p, 18, 10, 228, 0x57, cfg[14]));
        p.widgets.push(self.track(&p, 16, 10, 278, 0x58, cfg[12]));
        self.menu_loaded = false;
        let out;
        loop {
            let ev = self
                .dialog_loop(&mut p, false, |g, p, ev| {
                    let value = |p: &Panel, id: i32| {
                        p.widgets.iter().find(|w| w.id == id).map(|w| match &w.kind {
                            Kind::Track { value, .. } => *value,
                            Kind::Check { value, .. } => *value as i32,
                            _ => 0,
                        })
                    };
                    match ev {
                        10 | 11 | 12 | 13 | 14 => return Some(ev),
                        15 => g.next_track(),
                        16 => {
                            g.cfg.raw[12] = value(p, 16).unwrap_or(0) as u8;
                            g.apply_volumes();
                        }
                        17 => {
                            g.cfg.raw[13] = value(p, 17).unwrap_or(0) as u8;
                            g.apply_volumes();
                        }
                        18 => {
                            g.cfg.raw[14] = value(p, 18).unwrap_or(0) as u8;
                            g.apply_volumes();
                            if g.cfg.speech_volume() == 0 {
                                g.stop(Channel::Speech);
                            }
                        }
                        19 => g.cfg.raw[15] = value(p, 19).unwrap_or(0) as u8,
                        20 | 21 => {
                            let other = if ev == 20 { 21 } else { 20 };
                            if let Some(w) = p.widgets.iter_mut().find(|w| w.id == other) {
                                if let Kind::Check { value, .. } = &mut w.kind {
                                    *value = false;
                                }
                                let w = w.clone();
                                g.draw_widget(&w, false);
                            }
                            g.cfg.raw[16] = (ev == 20) as u8;
                            g.music_pos = -1;
                            g.next_track();
                        }
                        e if e == -0x1b => return Some(if in_game { 10 } else { 11 }),
                        _ => {}
                    }
                    if g.input.right && in_game {
                        return Some(10);
                    }
                    None
                })
                .await;
            if self.quitting() {
                out = MenuResult::Quit;
                break;
            }
            match ev {
                10 => {
                    out = MenuResult::Continue;
                    break;
                }
                11 => {
                    if self.confirm(0x5b).await {
                        out = MenuResult::Quit;
                        break;
                    }
                }
                12 => {
                    if self.confirm(0x5a).await {
                        out = MenuResult::New;
                        break;
                    }
                }
                13 => {
                    if let Some((file, _)) = self.save_load(false, &snapshot).await {
                        out = MenuResult::Load;
                        self.menu_file = Some(file);
                        break;
                    }
                }
                14 => {
                    if let Some((file, name)) = self.save_load(true, &snapshot).await {
                        let data = self.save(&name, &snapshot);
                        let mut io = self.io.borrow_mut();
                        io.files.insert(file.clone(), data.clone());
                        io.writes.push((file, data));
                        out = MenuResult::Save;
                        break;
                    }
                }
                _ => {}
            }
        }
        if matches!(out, MenuResult::Load) || (out == MenuResult::Continue && from_autosave && !in_game) {
            self.scr.clear(480);
            let file = self.menu_file.clone().unwrap_or_else(|| crate::app::AUTOSAVE.to_string());
            let data = self.io.borrow().files.get(&file).cloned();
            if let Some(d) = data {
                self.menu_loaded = self.load(&d).is_ok();
            }
        }
        if out != MenuResult::Quit {
            self.clear_input();
            self.redraw = true;
            self.base = self.clock.now().saturating_sub(35);
        }
        self.menu_file = None;
        out
    }
}

/// F1 settings panel (port extra), drawn with the game's font over a dark box.
impl Game {
    pub async fn settings_panel(&mut self) {
        use crate::settings::{Hotspots, Scaling};
        let saved = self.scr.back.clone();
        let font = self.a.font.clone();
        let (x0, y0, w, row) = (120, 70, 400, 26);
        let labels = |g: &Game| -> Vec<Vec<u8>> {
            let s = &g.io.borrow().settings;
            let c = &g.cfg;
            let on = |b: bool| if b { "zap." } else { "vyp." };
            [
                format!("Hlasitost hudby: {}", c.music_volume()),
                format!("Hlasitost zvuku: {}", c.fx_volume()),
                format!("Hlasitost řeči: {}", c.speech_volume()),
                format!("Titulky: {}", on(c.subtitles())),
                format!(
                    "Aktivní místa: {}",
                    match s.hotspots {
                        Hotspots::HoldSpace => "podržet mezerník",
                        Hotspots::Always => "vždy",
                        Hotspots::Off => "vyp.",
                    }
                ),
                format!("Celá obrazovka: {}", on(s.fullscreen)),
                format!("Obraz: {}", if s.scaling == Scaling::Sharp { "ostré pixely" } else { "na celou plochu" }),
                format!("Animace na střed: {}", on(s.center_cutscenes)),
                format!("Ukládat při změně místnosti: {}", on(s.autosave_rooms)),
                "Zavřít (Esc)".to_string(),
            ]
            .into_iter()
            .map(|t| crate::reader::to_cp1250(&t))
            .collect()
        };
        let draw = |g: &mut Game, hover: Option<usize>| {
            let rows = labels(g);
            let h = 50 + rows.len() as i32 * row;
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    let p = &mut g.scr.back[y as usize * 640 + x as usize];
                    *p = (*p >> 2) & 0x39e7;
                }
            }
            let mut s = Screen { back: std::mem::take(&mut g.scr.back), ..Default::default() };
            s.lut = g.scr.lut;
            s.lut[0] = 0;
            s.lut[1] = 0xffe0;
            s.lut[2] = 0xffff;
            let title = text_render::layout(&crate::reader::to_cp1250("Nastavení (F1)"), 60);
            text_render::draw(&mut s, &font, &title, x0 + 16, y0 + 12, 1);
            for (i, r) in rows.iter().enumerate() {
                let l = text_render::layout(r, 60);
                text_render::draw(&mut s, &font, &l, x0 + 16, y0 + 44 + i as i32 * row, if hover == Some(i) { 1 } else { 2 });
            }
            g.scr.back = s.back;
        };
        self.cursor.base = 0;
        self.cursor.item = None;
        draw(self, None);
        self.clear_input();
        self.reset_base();
        loop {
            self.cursor_show();
            self.present();
            self.wait_tick().await;
            self.cursor_hide();
            if self.input.esc() || (self.input.key_down && self.input.key == 0x70) || self.quitting() || self.input.right {
                break;
            }
            let (mx, my) = (self.input.mouse_x, self.input.mouse_y);
            let n = labels(self).len();
            let hover = (mx >= x0 && mx < x0 + w && my >= y0 + 44)
                .then(|| ((my - y0 - 44) / row) as usize)
                .filter(|&i| i < n);
            if self.input.left
                && let Some(i) = hover
            {
                let step = |v: u8| if v >= 100 { 0 } else { (v / 25 + 1) * 25 };
                {
                    let mut io = self.io.borrow_mut();
                    let s = &mut io.settings;
                    match i {
                        0 => self.cfg.raw[12] = step(self.cfg.raw[12]),
                        1 => self.cfg.raw[13] = step(self.cfg.raw[13]),
                        2 => self.cfg.raw[14] = step(self.cfg.raw[14]),
                        3 => self.cfg.raw[15] = (self.cfg.raw[15] == 0) as u8,
                        4 => {
                            s.hotspots = match s.hotspots {
                                Hotspots::HoldSpace => Hotspots::Always,
                                Hotspots::Always => Hotspots::Off,
                                Hotspots::Off => Hotspots::HoldSpace,
                            }
                        }
                        5 => s.fullscreen = !s.fullscreen,
                        6 => s.scaling = if s.scaling == Scaling::Sharp { Scaling::Fill } else { Scaling::Sharp },
                        7 => s.center_cutscenes = !s.center_cutscenes,
                        8 => s.autosave_rooms = !s.autosave_rooms,
                        _ => {}
                    }
                    let text = s.to_text().into_bytes();
                    io.writes.push((crate::settings::FILE.into(), text));
                    let cfg = self.cfg.raw.to_vec();
                    io.writes.push(("Main.cfg".into(), cfg));
                }
                self.apply_volumes();
                if i == 9 {
                    break;
                }
            }
            self.scr.back.copy_from_slice(&saved);
            draw(self, hover);
        }
        self.scr.back = saved;
        self.redraw = true;
        self.clear_input();
        self.reset_base();
    }
}
