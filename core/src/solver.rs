//! Search over game states by trying hotspots, items and item combinations. Used by the
//! autoplay test bot and by the in-game hint (port extra).

use crate::game::Game;
use std::collections::{BinaryHeap, HashSet};
use std::hash::{Hash, Hasher};

/// One player action: use `region` of `room` (with `item` or −1), or combine items (region −10,
/// item = a·1000 + b).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Action {
    pub room: i32,
    pub region: i32,
    pub item: i32,
}

pub fn key(g: &Game) -> u64 {
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

pub fn score(g: &Game) -> i64 {
    g.flags.iter().map(|b| b.count_ones() as i64).sum::<i64>() * 10 + g.chars[0].items.len() as i64
}

pub fn actions(g: &Game) -> Vec<Action> {
    let room = g.cur_id();
    let items: Vec<i32> = g.chars[0].items.iter().map(|&i| i as i32).collect();
    let mut v = Vec::new();
    for (i, &a) in items.iter().enumerate() {
        for &b in &items[i + 1..] {
            v.push(Action { room, region: -10, item: a * 1000 + b });
        }
    }
    let regions = (0..g.cur().regions.len() as i32).filter(|&r| g.cur().regions[r as usize].enabled).chain([0x800, 0x801]);
    for r in regions {
        v.push(Action { room, region: r, item: -1 });
        for &it in &items {
            v.push(Action { room, region: r, item: it });
        }
    }
    v
}

impl Game {
    pub async fn perform(&mut self, a: Action) -> bool {
        if a.region == -10 { self.combine(a.item / 1000, a.item % 1000).await } else { self.act(a.region, a.item).await }
    }

    /// Best-first search from the current state for the first action of a path that gains
    /// score (a flag or an item). The game state is restored afterwards.
    pub async fn hint(&mut self, budget: usize) -> Option<Action> {
        let thumb = vec![0u16; crate::save::THUMB_W * crate::save::THUMB_H];
        let start = self.save(b"hint", &thumb);
        let saved = (self.auto, self.auto_draw, self.instant, self.rng.clone(), self.base);
        self.auto = true;
        self.auto_draw = false;
        self.instant = true;
        let base_score = score(self);
        let mut seen = HashSet::new();
        seen.insert(key(self));
        let mut heap = BinaryHeap::new();
        heap.push((0i64, 0usize, start.clone(), None::<Action>));
        let mut found = None;
        let mut n = 0;
        'search: while let Some((_, _, save, first)) = heap.pop() {
            n += 1;
            if n > budget {
                break;
            }
            if self.load(&save).is_err() {
                continue;
            }
            for a in actions(self) {
                if self.load(&save).is_err() {
                    break;
                }
                if !self.perform(a).await || self.game_over {
                    continue;
                }
                let k = key(self);
                if !seen.insert(k) {
                    continue;
                }
                let first = first.or(Some(a));
                if score(self) > base_score {
                    found = first;
                    break 'search;
                }
                heap.push((score(self), usize::MAX - n, self.save(b"hint", &thumb), first));
            }
        }
        let _ = self.load(&start);
        (self.auto, self.auto_draw, self.instant, self.rng, self.base) = saved;
        self.redraw = true;
        found
    }
}
