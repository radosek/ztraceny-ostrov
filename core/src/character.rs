//! TCharacter (unit 0x456494–0x458694): position, walking, path planning, sprite frame choice.
//! See docs/ENGINE_B_character.md.

use crate::geom::{angle, dround, inside, segment_inside};
use crate::world::Room;

pub const NONE_DIR: i8 = -1;

#[derive(Clone, Debug)]
pub struct Character {
    /// +4: drawn by its FLC anim instead of a sprite list; never path-finds.
    pub is_anim_npc: bool,
    /// +0x10: Data.dat index of the sprite set.
    pub skin: u8,
    pub dir: i8,
    pub target_dir: i8,
    pub final_dir: i8,
    pub x: i32,
    pub y: i32,
    pub seg_start: (i32, i32),
    pub seg_end: (i32, i32),
    pub queued: (i32, i32),
    pub dest: (i32, i32),
    pub walk_frame: i32,
    pub scale: f64,
    pub items: Vec<u8>,
    pub moving: bool,
    pub just_stopped: u8,
    pub via_waypoint: bool,
    pub visited: [bool; 20],
    pub tried: [bool; 20],
    pub busy: bool,
    pub room_id: i32,
    pub cycle: u8,
    pub talk_color: u8,
    pub talking: bool,
    pub talk_ticks: i32,
    pub talk_duration: i32,
    pub talk_toggle: bool,
    /// +0xc: Partial.dat index of the NPC anim (2000 = none).
    pub npc_anim: Option<i32>,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            is_anim_npc: false,
            skin: 0,
            dir: 0,
            target_dir: 0,
            final_dir: NONE_DIR,
            x: 0,
            y: 0,
            seg_start: (0, 0),
            seg_end: (0, 0),
            queued: (-1, -1),
            dest: (0, 0),
            walk_frame: -1,
            scale: 1.0,
            items: Vec::new(),
            moving: false,
            just_stopped: 0,
            via_waypoint: false,
            visited: [false; 20],
            tried: [false; 20],
            busy: false,
            room_id: 0,
            cycle: 0,
            talk_color: 0,
            talking: false,
            talk_ticks: 0,
            talk_duration: 0,
            talk_toggle: false,
            npc_anim: None,
        }
    }
}

const TAN20: f64 = 0.363_970_234_266_202_4;
const TAN10: f64 = 0.176_326_980_708_464_97;

/// Straight-line test of TryDirect (angle from vertical ≤ 20°, from horizontal ≤ 10°, axis-aligned,
/// or both deltas under 6 px).
fn is_straight(dx: i32, dy: i32) -> bool {
    if dx == 0 || dy == 0 || (dx.abs() < 6 && dy.abs() < 6) {
        return true;
    }
    let from_vertical = (dx.abs() as f64 / dy.abs() as f64).atan().to_degrees();
    from_vertical <= 20.0 || 90.0 - from_vertical <= 10.0
}

fn walkable(walk: &[(i32, i32)], a: (i32, i32), b: (i32, i32)) -> bool {
    inside(walk, a.0, a.1) && inside(walk, b.0, b.1) && segment_inside(walk, a.0, a.1, b.0, b.1)
}

/// The two-leg plan TryDirect uses when the line is too oblique: (corner, ok).
fn dog_leg(walk: &[(i32, i32)], s: (i32, i32), d: (i32, i32), force: bool) -> Option<(i32, i32)> {
    let (dx, dy) = (d.0 - s.0, d.1 - s.1);
    let ox = dx.signum() * dround(dy.abs() as f64 * TAN20);
    let oy = dy.signum() * dround(dx.abs() as f64 * TAN10);
    let a = (d.0 - ox, s.1 + oy);
    if walkable(walk, s, a) && walkable(walk, a, d) {
        return Some(a);
    }
    if force {
        return Some(a);
    }
    let b = (s.0 + ox, d.1 - oy);
    (walkable(walk, s, b) && walkable(walk, b, d)).then_some(b)
}

/// 0x456648: would TryDirect succeed, without side effects.
pub fn can_reach(walk: &[(i32, i32)], s: (i32, i32), d: (i32, i32)) -> bool {
    let (dx, dy) = (d.0 - s.0, d.1 - s.1);
    if is_straight(dx, dy) { walkable(walk, s, d) } else { dog_leg(walk, s, d, false).is_some() }
}

impl Character {
    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// 0x4574ac.
    pub fn rescale(&mut self, room: Option<&Room>) {
        let Some(r) = room else { return };
        let (a, b) = (r.scale_a as i32, r.scale_b as i32);
        self.scale = if self.y == a { 1.0 } else { (b - a) as f64 / (self.y - a) as f64 };
        if self.scale < 1.0 {
            self.scale = 1.0;
        }
    }

    /// SetDir 0x457b58.
    pub fn set_dir(&mut self, d: i8) {
        self.dir = d;
        self.target_dir = d;
        self.stop();
    }

    /// Turn 0x457b7c.
    pub fn turn(&mut self, d: i8) {
        self.target_dir = d;
        if d != self.dir {
            self.moving = true;
        }
    }

    /// SetX/SetY 0x457bac/0x457bcc.
    pub fn set_pos(&mut self, x: Option<i32>, y: Option<i32>, room: Option<&Room>) {
        if let Some(x) = x {
            self.x = x;
            self.seg_end.0 = x;
            self.dest.0 = x;
        }
        if let Some(y) = y {
            self.y = y;
            self.seg_end.1 = y;
            self.dest.1 = y;
        }
        self.stop();
        self.rescale(room);
    }

    /// Stop 0x457c34.
    pub fn stop(&mut self) {
        self.dest = (self.x, self.y);
        self.seg_start = self.dest;
        self.seg_end = self.dest;
        self.queued = (-1, -1);
        self.moving = false;
        self.walk_frame = -1;
        self.just_stopped = 2;
    }

    /// Face along the current segment (0x457430).
    fn face_segment(&mut self) {
        let a = angle(self.x, self.y, self.seg_end.0, self.seg_end.1);
        let d = match a {
            31..=150 => 0,
            151..=210 => 6,
            211..=330 => 4,
            _ => 2,
        };
        self.turn(d);
    }

    fn start_segment(&mut self, to: (i32, i32), queued: Option<(i32, i32)>) {
        self.seg_start = (self.x, self.y);
        self.seg_end = to;
        self.queued = queued.unwrap_or((-1, -1));
        self.face_segment();
        self.moving = true;
    }

    /// TryDirect 0x457c64.
    fn try_direct(&mut self, walk: &[(i32, i32)], d: (i32, i32), force: bool) -> bool {
        let s = self.pos();
        let (dx, dy) = (d.0 - s.0, d.1 - s.1);
        if is_straight(dx, dy) {
            if force || walkable(walk, s, d) {
                self.start_segment(d, None);
                return true;
            }
            return false;
        }
        match dog_leg(walk, s, d, force) {
            Some(c) => {
                self.start_segment(c, Some(d));
                true
            }
            None => false,
        }
    }

    /// WalkTo 0x456ed4.
    pub fn walk_to(&mut self, room: &Room, x: i32, y: i32, reset: bool) {
        let n = room.waypoints.len().min(20);
        if reset {
            self.visited[..n].fill(false);
        }
        self.tried = [false; 20];
        self.dest = (x, y);
        self.via_waypoint = false;
        let walk = &room.walk.points;
        while !self.try_direct(walk, self.dest, false) {
            match self.pick_waypoint(room) {
                None => {
                    self.try_direct(walk, self.dest, true);
                    return;
                }
                Some(i) => {
                    self.tried[i] = true;
                    self.via_waypoint = true;
                    if self.try_direct(walk, room.waypoints[i], false) {
                        self.visited[i] = true;
                        return;
                    }
                }
            }
        }
    }

    /// 0x4579c0.
    fn pick_waypoint(&self, room: &Room) -> Option<usize> {
        let s = self.pos();
        let d = self.dest;
        let direct = angle(s.0, s.1, d.0, d.1);
        let mut best: Option<(i32, usize)> = None;
        for (i, &w) in room.waypoints.iter().enumerate().take(20) {
            if self.visited[i] || self.tried[i] || w.0 == s.0 || w.1 == s.1 || w.0 == d.0 || w.1 == d.1 {
                continue;
            }
            let dist = |a: (i32, i32), b: (i32, i32)| dround((((a.0 - b.0).pow(2) + (a.1 - b.1).pow(2)) as f64).sqrt());
            let mut diff = (angle(s.0, s.1, w.0, w.1) - direct).abs();
            if diff > 180 {
                diff = 360 - diff;
            }
            let mut cost = dist(s, w) + dist(w, d) + diff / 2;
            if can_reach(&room.walk.points, w, d) {
                cost -= 50;
            }
            if best.is_none_or(|(c, _)| cost < c) {
                best = Some((cost, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Goto op (0x457040).
    pub fn goto(&mut self, room: &Room, x: i32, y: i32, dir: i8) {
        if (self.x, self.y) == (x, y) {
            self.turn(dir);
        } else {
            self.walk_to(room, x, y, true);
            self.final_dir = dir;
        }
    }

    fn base_step(&self) -> i32 {
        let e = self.skin == 0x0e;
        match self.dir {
            0 => {
                if e && self.walk_frame == 9 { 15 } else { 5 }
            }
            4 => {
                if e && self.walk_frame == 9 { 15 } else { 4 }
            }
            _ => match self.skin {
                3 => 3,
                0x0e => {
                    if self.walk_frame == 9 { 25 } else { 5 }
                }
                0x0f => 8,
                _ => 10,
            },
        }
    }

    /// Per-tick update 0x457ff0 (talk tick is separate).
    pub fn update(&mut self, room: Option<&Room>) {
        self.cycle = (self.cycle + 1) % 6;
        self.just_stopped = self.just_stopped.saturating_sub(1);
        if self.target_dir != self.dir {
            let diff = (self.target_dir - self.dir).rem_euclid(8);
            if diff == 1 || diff == 7 {
                self.dir = self.target_dir;
            } else if diff <= 4 {
                self.dir = (self.dir + 1).rem_euclid(8);
            } else {
                self.dir = (self.dir - 1).rem_euclid(8);
            }
            self.walk_frame = -1;
            if self.dir == self.target_dir && self.pos() == self.seg_end {
                self.moving = false;
            }
            return;
        }
        if self.pos() != self.seg_end {
            let mut step = dround(self.base_step() as f64 / self.scale);
            if self.skin == 0x0e {
                step = step.max(3);
            }
            let (sx, sy) = self.seg_start;
            let (ex, ey) = self.seg_end;
            if (ex - sx).abs() > (ey - sy).abs() {
                self.x = advance(self.x, ex, step, self.via_waypoint);
                self.y = sy + (self.x - sx).abs() * (ey - sy) / (ex - sx).abs();
            } else {
                self.y = advance(self.y, ey, step, self.via_waypoint);
                let d = (ey - sy).abs();
                self.x = if d == 0 { sx } else { sx + (self.y - sy).abs() * (ex - sx) / d };
            }
            let m = if (8..=11).contains(&self.skin) { 8 } else { 10 };
            self.walk_frame = (self.walk_frame + 1).rem_euclid(m);
        }
        let reached = self.pos() == self.seg_end
            || (self.via_waypoint
                && (self.x - self.seg_end.0).abs() < dround(11.0 / self.scale)
                && (self.y - self.seg_end.1).abs() < dround(6.0 / self.scale));
        if reached && self.moving {
            if self.queued.0 != -1 {
                let q = self.queued;
                self.start_segment(q, None);
            } else if self.pos() != self.dest {
                if let Some(r) = room {
                    let (dx, dy) = self.dest;
                    self.walk_to(r, dx, dy, false);
                }
            } else {
                self.walk_frame = -1;
                self.moving = false;
                self.just_stopped = 2;
                if self.final_dir != NONE_DIR && self.final_dir != self.dir {
                    self.turn(self.final_dir);
                }
                self.final_dir = NONE_DIR;
                if !self.busy {
                    match self.dir {
                        2 => self.turn(3),
                        6 => self.turn(5),
                        _ => {}
                    }
                }
            }
        }
        self.rescale(room);
    }

    /// Sprite frame index 0x456ba4, and whether to mirror (0x456e8c).
    pub fn frame(&self) -> (usize, bool) {
        let e = self.skin == 0x0e;
        let base = match self.dir {
            0 => 22,
            1 => 34,
            2 => {
                if e { 43 } else { 0 }
            }
            3 | 5 => 33,
            4 => 11,
            6 => 0,
            _ => 34,
        };
        let mut f = base;
        if self.moving && self.walk_frame >= 0 && self.dir % 2 == 0 {
            f = base + self.walk_frame + 1;
        } else if f == 33 {
            f = if self.just_stopped != 0 { 35 } else { 36 };
        }
        if self.talking {
            let t = (self.talk_ticks / 2) % 6;
            if e {
                f = 54 + t;
            } else if f == 36 || self.skin == 3 {
                f = 43 + t;
            } else if f == 11 {
                f = 37 + t;
            }
        }
        (f as usize, self.dir < 4 && !e)
    }
}

fn advance(v: i32, to: i32, step: i32, tolerant: bool) -> i32 {
    if v < to {
        let n = v + step;
        if n > to && !tolerant { to } else if n > to { v - step.min(n - to) } else { n }
    } else {
        let n = v - step;
        if n < to && !tolerant { to } else if n < to { v + step.min(to - n) } else { n }
    }
}
