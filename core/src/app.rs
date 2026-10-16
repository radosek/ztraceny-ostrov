//! Program entry (0x467bcc → 0x4630a4): logo video, optional autosave menu, game sessions.

use crate::assets::Assets;
use crate::config::Config;
use crate::game::{Game, Io, IoRef};
use crate::sched::{Clock, Executor};
use crate::session::Exit;
use crate::ui::MenuResult;
use std::{cell::RefCell, rc::Rc};

pub const AUTOSAVE: &str = "SaveGame/auto.sav";

pub struct App {
    pub io: IoRef,
    pub exec: Executor<()>,
}

async fn main(mut g: Game) {
    g.video("Logo.avi", 0x1fa).await;
    let mut menu: Option<(bool, bool)> = g.io.borrow().files.contains_key(AUTOSAVE).then_some((false, true));
    loop {
        let exit = match menu {
            Some((in_game, from_autosave)) => {
                g.new_game();
                match g.menu(in_game, from_autosave).await {
                    MenuResult::Quit => break,
                    MenuResult::Load | MenuResult::Continue if from_autosave || g.menu_loaded => g.resume().await,
                    _ => g.run_session(true).await,
                }
            }
            None => g.run_session(true).await,
        };
        menu = match exit {
            Exit::Quit => break,
            Exit::GameOver => Some((false, false)),
            Exit::NewGame => None,
        };
    }
    let thumb = crate::save::thumbnail(&g.scr.back);
    let data = g.save(b"AUTOSAVE", &thumb);
    let cfg = g.cfg.raw.to_vec();
    let mut io = g.io.borrow_mut();
    io.writes.push((AUTOSAVE.into(), data));
    io.writes.push(("Main.cfg".into(), cfg));
    io.quit = true;
}

impl App {
    pub fn new(assets: Rc<Assets>, cfg: Config, seed: u32, files: std::collections::HashMap<String, Vec<u8>>) -> Self {
        let io: IoRef = Rc::new(RefCell::new(Io { files, ..Default::default() }));
        let clock = Clock::default();
        let g = Game::new(assets, clock.clone(), io.clone(), cfg, seed);
        Self { io, exec: Executor::new(clock, main(g)) }
    }

    /// Advance game time to `ms`.
    pub fn run_until(&mut self, ms: u64) {
        self.exec.run_until(ms, |_| {});
    }

    pub fn finished(&self) -> bool {
        self.exec.finished().is_some()
    }
}
