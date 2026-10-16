//! Custom in-surface UI (unit 0x464e55–0x467900): game menu, confirm and save/load dialogs.

use crate::assets::Arc8;
use crate::game::Game;
use crate::screen::{H, Target};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuResult {
    Continue,
    Load,
    Save,
    New,
    Quit,
}

const MENU_BUTTONS: [(i32, i32, i32); 6] = [(10, 9, 10), (10, 34, 13), (10, 64, 14), (10, 93, 12), (10, 118, 11), (92, 257, 15)];

impl Game {
    /// Game menu 0x464538.
    pub async fn menu(&mut self, in_game: bool, from_autosave: bool) -> MenuResult {
        let saved_back = self.scr.back.clone();
        let a = self.a.clone();
        let Ok(panel) = a.image(Arc8::Data, 0x40, 0) else { return MenuResult::Continue };
        let (px, py) = ((640 - panel.width as i32) / 2, (480 - panel.height as i32) / 2);
        self.scr.blit(Target::Back, &panel, px, py, false, H);
        self.hide_overlays();
        self.cursor.item = None;
        self.clear_input();
        self.reset_base();
        let r = loop {
            self.music_tick();
            self.present();
            self.wait_tick().await;
            if self.input.esc() {
                break if in_game { MenuResult::Continue } else { MenuResult::Quit };
            }
            if self.input.right && in_game {
                break MenuResult::Continue;
            }
            if !self.input.left {
                continue;
            }
            let (mx, my) = (self.input.mouse_x - px, self.input.mouse_y - py);
            let hit = MENU_BUTTONS.iter().find(|&&(x, y, _)| (x..x + 140).contains(&mx) && (y..y + 22).contains(&my));
            match hit.map(|b| b.2) {
                Some(10) if in_game || from_autosave => break MenuResult::Continue,
                Some(12) => break MenuResult::New,
                Some(11) => break MenuResult::Quit,
                Some(15) => self.next_track(),
                _ => {}
            }
        };
        self.scr.back = saved_back;
        self.clear_input();
        self.reset_base();
        r
    }
}
