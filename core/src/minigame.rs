//! SpecialFX 1: the cannon "battleship" minigame 0x45ef1c (ENGINE_D §D7). Returns true on a win.

use crate::assets::Arc8;
use crate::flc;
use crate::game::{AnimSlot, Game};
use crate::gfx::Image;
use crate::mixer::Channel;
use crate::screen::{H, SavedRect, Target};
use std::rc::Rc;

const COLS: usize = 21;
const ROWS: usize = 8;
const X0: i32 = 46;
const Y0: i32 = 42;
const CELL: i32 = 26;
const FRAME_MS: u64 = 0x72;

fn cell(x: i32, y: i32) -> Option<(usize, usize)> {
    ((46..592).contains(&x) && (42..250).contains(&y)).then(|| (((x - X0) / CELL) as usize, ((y - Y0) / CELL) as usize))
}

struct Board {
    grid: [[u8; COLS]; ROWS],
    shots: i32,
    list: Rc<Vec<Image>>,
    flc: Option<AnimSlot>,
    effect: Option<(Image, i32, i32, SavedRect)>,
}

fn blit(g: &mut Game, img: &Image, x: i32, y: i32) {
    g.scr.blit(Target::Back, img, x, y, true, H);
}

impl Board {
    /// 0x45ee38.
    fn ammo(&self, g: &mut Game) {
        if let Some(img) = self.list.get(31) {
            g.scr.blit(Target::Back, img, 0, 420, false, H);
        }
        if let Some(img) = self.list.get(30) {
            for i in 0..self.shots {
                blit(g, img, 105 + 17 * i, 437);
            }
        }
    }

    /// 0x45e84c.
    fn board(&self, g: &mut Game) {
        for (r, row) in self.grid.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                if v >= 0x81
                    && let Some(img) = self.list.get((v - 0x81) as usize)
                {
                    blit(g, img, X0 + CELL * c as i32, Y0 + CELL * r as i32);
                }
            }
        }
    }

    fn flc_step(&mut self, g: &mut Game) {
        if let Some(mut s) = self.flc.take() {
            let a = g.a.clone();
            let d = a.raw(s.archive, s.index);
            let mut sink = g.sink(true, false, 0);
            let _ = flc::decode_frame(d, &mut s.pos, false, &mut sink);
            if s.pos.frame >= s.header.frames as u32 {
                s.pos = if s.header.loop_frame != 0 { flc::Pos { offset: s.header.loop_frame, frame: 1 } } else { s.header.start() };
            }
            self.flc = Some(s);
        }
    }

    fn effect_hide(&mut self, g: &mut Game) {
        if let Some((_, _, _, r)) = self.effect.take() {
            g.scr.put_rect(&r);
        }
    }

    fn effect_show(&mut self, g: &mut Game, img: Image, x: i32, y: i32) {
        let (ex, ey) = (x - img.width as i32 / 2, y - img.height as i32 / 2);
        let r = g.scr.save_rect(ex, ey, img.width as i32, img.height as i32);
        blit(g, &img, ex, ey);
        self.effect = Some((img, x, y, r));
    }

    /// Effect player 0x45ebbc: six consecutive images from the list, 114 ms each.
    async fn effect(&mut self, g: &mut Game, first: usize, x: i32, y: i32, fx: i32) {
        if fx > 0 {
            if let Some(s) = g.a.sound(Arc8::Fx, fx) {
                g.io.borrow_mut().mixer.play(Channel::Fx, s, true);
            }
        }
        g.cursor_hide();
        g.tip_hide(false);
        for f in 0..6 {
            self.effect_hide(g);
            self.flc_step(g);
            if let Some(img) = self.list.get(first + f).cloned() {
                self.effect_show(g, img, x, y);
            }
            g.present();
            g.wait_ms(FRAME_MS).await;
        }
        self.effect_hide(g);
        g.stop(Channel::Fx);
        g.clear_input();
    }

    /// In-minigame Say 0x45e930.
    async fn say(&mut self, g: &mut Game, line: i32) {
        g.cursor_hide();
        g.tip_hide(false);
        g.duck(true);
        g.say_line(line);
        let spk = g.char_index(g.a.lines.get(line as usize).map_or(0, |l| l.speaker as i32));
        loop {
            g.tip_hide(true);
            self.flc_step(g);
            g.talk_tick(spk, true);
            g.tip_show(true);
            g.present();
            if !g.any_talking() {
                break;
            }
            g.wait_ms(FRAME_MS).await;
            if g.input.skip() {
                g.stop_all_talk();
                break;
            }
        }
        g.tip_hide(true);
        g.duck(false);
    }
}

pub async fn cannon(g: &mut Game) -> bool {
    let a = g.a.clone();
    g.hide_overlays();
    let saved_palette = g.scr.palette;
    if let Ok(p) = a.palette(Arc8::Data, 0x28) {
        g.scr.set_palette(p);
    }
    let mut grid = [[0u8; COLS]; ROWS];
    let col = g.rng.below(16) as usize;
    let row = g.rng.below(7) as usize;
    for r in 0..2 {
        for c in 0..6 {
            grid[row + r][col + c] = (r * 6 + c + 1) as u8;
        }
    }
    let d = a.raw(Arc8::Data, 0x2a);
    let flc = flc::header(d).ok().map(|header| AnimSlot { archive: Arc8::Data, index: 0x2a, header, pos: header.start() });
    let mut b = Board { grid, shots: 25, list: a.sprite_list(0x2b).unwrap_or_default(), flc, effect: None };
    let backdrop = a.image(Arc8::Data, 0x29, 0).ok();
    let wrap = g.wrap;
    g.wrap = 70;
    let draw_all = |g: &mut Game, b: &mut Board| {
        if let Some(img) = &backdrop {
            g.scr.blit(Target::Back, img, 0, 0, false, H);
        }
        if let Some(f) = b.flc.as_mut() {
            f.pos = f.header.start();
        }
        b.board(g);
    };
    draw_all(g, &mut b);
    b.ammo(g);
    g.cursor.base = 0;
    g.cursor.item = None;
    g.cursor_show();
    g.reset_base();
    let mut label = false;
    let won = loop {
        g.cursor_hide();
        g.tip_hide(false);
        if g.redraw {
            draw_all(g, &mut b);
            g.redraw = false;
        }
        b.flc_step(g);
        if label {
            g.tip_show(false);
        }
        g.cursor_animate();
        g.cursor_show();
        g.present();
        g.wait_ms(FRAME_MS).await;
        g.music_tick();
        if g.quitting() {
            break false;
        }
        let (mx, my) = (g.input.mouse_x, g.input.mouse_y);
        let hover = cell(mx, my);
        g.cursor_hide();
        g.tip_hide(false);
        label = hover.is_some();
        if let Some((c, r)) = hover {
            g.tip_set(false, format!("[{c},{r}]").as_bytes(), mx + 10, my + 10);
        }
        if !g.input.left {
            continue;
        }
        let Some((c, r)) = hover else { continue };
        b.shots -= 1;
        b.ammo(g);
        b.effect(g, 12, 320, 305, 0x101).await;
        if b.grid[r][c] == 0 {
            b.effect(g, 24, X0 + CELL * c as i32 + 12, Y0 + CELL * r as i32 + 14, 0x100).await;
        } else {
            if b.grid[r][c] < 0x81 {
                b.grid[r][c] += 0x80;
            }
            b.board(g);
            b.effect(g, 18, X0 + CELL * c as i32 + 10, Y0 + CELL * r as i32 + 10, 0x102).await;
            b.say(g, 0x1f5).await;
        }
        if !b.grid.iter().flatten().any(|&v| (1..=0x80).contains(&v)) {
            break true;
        }
        if b.shots == 0 {
            b.say(g, 0x1f6).await;
            break false;
        }
    };
    g.cursor_hide();
    g.tip_hide(false);
    g.wrap = wrap;
    g.scr.clear(480);
    g.scr.set_palette(saved_palette);
    g.redraw = true;
    g.base = g.clock.now().saturating_sub(57);
    won
}
