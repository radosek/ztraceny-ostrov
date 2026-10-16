//! Replay a bot walkthrough with drawing on, dumping the frame after every step.
//!   replay <game-dir> <walkthrough.txt> <out-dir> [first] [last]
use ostrov_core::{assets::Assets, config::Config, game::{Game, Io}, sched::{Clock, Executor}, screen::rgb565_to_rgba};
use std::{cell::RefCell, rc::Rc};

fn dump(io: &Io, path: &std::path::Path) {
    let mut rgba = vec![0u8; 640 * 480 * 4];
    rgb565_to_rgba(&io.frame, &mut rgba);
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(f, 640, 480);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&rgba).unwrap();
}

async fn run(mut g: Game, steps: Vec<(i32, i32, i32)>, out: std::path::PathBuf, first: usize, last: usize) {
    g.auto = std::env::var("REPLAY_REALTIME").is_err();
    g.auto_draw = true;
    g.new_game();
    g.run_enter().await;
    for (n, &(room, r, it)) in steps.iter().enumerate().take(last + 1) {
        if g.cur_id() != room {
            eprintln!("step {n}: expected room {room:#x}, in {:#x}", g.cur_id());
        }
        let ok = if r == -10 { g.combine(it / 1000, it % 1000).await } else { g.act(r, it).await };
        // one ordinary tick so the screen settles like in the main loop
        if g.redraw {
            g.redraw_chars(true);
            g.redraw = false;
        }
        g.frame();
        g.present();
        if n >= first {
            dump(&g.io.borrow(), &out.join(format!("step{n:03}_{room:x}_{r}_{it}.png")));
        }
        if !ok {
            eprintln!("step {n}: action ignored");
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = a[1].clone();
    let steps: Vec<(i32, i32, i32)> = std::fs::read_to_string(&a[2])
        .unwrap()
        .lines()
        .filter_map(|l| {
            let mut p = l.split(':');
            let room = i32::from_str_radix(p.next()?.strip_prefix("0x")?, 16).ok()?;
            Some((room, p.next()?.parse().ok()?, p.next()?.parse().ok()?))
        })
        .collect();
    let out = std::path::PathBuf::from(&a[3]);
    std::fs::create_dir_all(&out).unwrap();
    let first = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let last = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX - 1);
    let find = |name: &str| {
        std::fs::read_dir(&dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).and_then(|e| std::fs::read(e.path()).ok())
    };
    let assets = Rc::new(Assets::load(find).expect("assets"));
    let clock = Clock::default();
    let io = Rc::new(RefCell::new(Io::default()));
    let mut cfg = Config::default();
    cfg.raw[14] = 0;
    let g = Game::new(assets, clock.clone(), io.clone(), cfg, 0);
    let mut ex = Executor::new(clock, run(g, steps, out, first, last));
    let mut t = 0u64;
    let every = std::env::var("REPLAY_FRAMES").ok().map(std::path::PathBuf::from);
    let mut last = 0;
    while ex.finished().is_none() {
        t += if every.is_some() { 35 } else { 1_000_000 };
        ex.run_until(t, |_| {});
        io.borrow_mut().audio.clear();
        if let Some(d) = &every {
            let io = io.borrow();
            if io.frames != last && io.room == 0x200 && t > 1000 {
                last = io.frames;
                dump(&io, &d.join(format!("f{t:08}.png")));
            }
        }
    }
}
