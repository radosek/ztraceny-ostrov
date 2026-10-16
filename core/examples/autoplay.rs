//! Bot that explores the game: best-first search over (flags, room, inventory, region state),
//! trying every enabled hotspot with no item and with each inventory item. Reports progress,
//! panics, stuck scripts, and whether the ending (GameOver) is reached.
//!
//!   cargo run --release -p ostrov-core --example autoplay -- <game-dir> [max-states]

use ostrov_core::{assets::Assets, config::Config, game::Game, game::Io, sched::{Clock, Executor}};
use std::{
    cell::RefCell,
    collections::{BinaryHeap, HashSet},
    hash::{Hash, Hasher},
    rc::Rc,
};

fn key(g: &Game) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.flags.hash(&mut h);
    g.cur_id().hash(&mut h);
    let mut items = g.chars[0].items.clone();
    items.sort();
    items.hash(&mut h);
    g.skin.hash(&mut h);
    for r in &g.rooms {
        for reg in &r.regions {
            (reg.enabled, reg.cursor, reg.image).hash(&mut h);
        }
    }
    h.finish()
}

fn score(g: &Game) -> i64 {
    g.flags.iter().map(|b| b.count_ones() as i64).sum::<i64>() * 10 + g.chars[0].items.len() as i64
}

async fn explore(mut g: Game, max: usize) -> String {
    g.auto = true;
    g.new_game();
    g.run_enter().await;
    let mut seen = HashSet::new();
    let mut heap = BinaryHeap::new();
    seen.insert(key(&g));
    let thumb = vec![0u16; 76 * 56];
    heap.push((score(&g), 0usize, g.save(b"bot", &thumb), Vec::<String>::new()));
    let mut states = 0;
    let mut best = (0i64, Vec::new());
    let mut rooms_seen = HashSet::new();
    let mut dead = HashSet::new();
    while let Some((sc, _, save, path)) = heap.pop() {
        states += 1;
        if states > max {
            break;
        }
        if g.load(&save).is_err() {
            continue;
        }
        rooms_seen.insert(g.cur_id());
        if sc > best.0 {
            best = (sc, path.clone());
            eprintln!("[{states}] score {sc} room {:#x} rooms {} items {:?} depth {}", g.cur_id(), rooms_seen.len(), g.chars[0].items, path.len());
        }
        let mut actions: Vec<(i32, i32)> = Vec::new();
        let items: Vec<i32> = g.chars[0].items.iter().map(|&i| i as i32).collect();
        let regions: Vec<i32> = (0..g.cur().regions.len() as i32)
            .filter(|&r| g.cur().regions[r as usize].enabled)
            .chain([0x800, 0x801])
            .collect();
        for (i, &a) in items.iter().enumerate() {
            for &b in &items[i + 1..] {
                actions.push((-10, a * 1000 + b));
            }
        }
        for &r in &regions {
            actions.push((r, -1));
            for &it in &items {
                actions.push((r, it));
            }
        }
        let fk = key(&g);
        for (r, it) in actions {
            let room = g.cur_id();
            let key_before = key(&g);
            if dead.contains(&(fk, room, r, it)) {
                continue;
            }
            if g.load(&save).is_err() {
                break;
            }
            let acted = if r == -10 { g.combine(it / 1000, it % 1000).await } else { g.act(r, it).await };
            if !acted {
                continue;
            }
            if g.game_over {
                let mut p = path.clone();
                p.push(format!("{room:#x}:{r}:{it}"));
                return format!("GAME OVER reached after {states} states, path length {}\n{}", p.len(), p.join("\n"));
            }
            let k = key(&g);
            if k == key_before {
                dead.insert((fk, room, r, it));
            }
            if seen.insert(k) {
                let mut p = path.clone();
                p.push(format!("{room:#x}:{r}:{it}"));
                let s = score(&g);
                heap.push((s, usize::MAX - states, g.save(b"bot", &thumb), p));
            }
        }
    }
    let mut r: Vec<_> = rooms_seen.into_iter().collect();
    r.sort();
    format!(
        "no ending within {states} states; best score {} at depth {}\nrooms visited ({}): {:x?}\nbest path:\n{}",
        best.0,
        best.1.len(),
        r.len(),
        r,
        best.1.join("\n")
    )
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = a[1].clone();
    let max: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(20000);
    let find = |name: &str| {
        std::fs::read_dir(&dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).and_then(|e| std::fs::read(e.path()).ok())
    };
    let assets = Rc::new(Assets::load(find).expect("assets"));
    let clock = Clock::default();
    let io = Rc::new(RefCell::new(Io::default()));
    let mut cfg = Config::default();
    cfg.raw[14] = 0;
    let g = Game::new(assets, clock.clone(), io.clone(), cfg, 0);
    let mut ex = Executor::new(clock, explore(g, max));
    let mut t = 0u64;
    while ex.finished().is_none() {
        t += 1_000_000;
        ex.run_until(t, |_| {});
        io.borrow_mut().audio.clear();
    }
    println!("{}", ex.finished().unwrap());
}
