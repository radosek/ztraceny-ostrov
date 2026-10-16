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

fn straight(dx: i32, dy: i32) -> bool {
    dx == 0
        || dy == 0
        || (dx as f64 / dy as f64).atan().abs() <= 0.3490658503988659
        || (dy as f64 / dx as f64).atan().abs() <= 0.17453292519943295
        || (dx.abs() <= 5 && dy.abs() <= 5)
}

fn corners(s: (i32, i32), d: (i32, i32)) -> ((i32, i32), (i32, i32)) {
    let (dx, dy) = (d.0 - s.0, d.1 - s.1);
    let t20 = 20f64.to_radians();
    let t10 = 10f64.to_radians();
    let ox = dx.signum() * dround((t20.sin() / t20.cos() * dy as f64).abs());
    let oy = dy.signum() * dround((t10.sin() / t10.cos() * dx as f64).abs());
    ((d.0 - ox, s.1 + oy), (s.0 + ox, d.1 - oy))
}

/// The corner TryDirect would walk via, or None when it fails.
enum Plan {
    Straight,
    Corner((i32, i32)),
    Fail,
}

fn plan(walk: &[(i32, i32)], s: (i32, i32), d: (i32, i32), force: bool) -> Plan {
    let (dx, dy) = (d.0 - s.0, d.1 - s.1);
    let both_in = inside(walk, d.0, d.1) && inside(walk, s.0, s.1);
    if straight(dx, dy) {
        if both_in && !segment_inside(walk, s.0, s.1, d.0, d.1) && !force {
            return Plan::Fail;
        }
        return Plan::Straight;
    }
    let (a, b) = corners(s, d);
    let seg = |p: (i32, i32), q: (i32, i32)| segment_inside(walk, p.0, p.1, q.0, q.1);
    if !both_in || (seg(s, a) && seg(a, d)) || force {
        Plan::Corner(a)
    } else if seg(s, b) && seg(b, d) {
        Plan::Corner(b)
    } else {
        Plan::Fail
    }
}

/// 0x456648: would TryDirect succeed, without side effects.
pub fn can_reach(walk: &[(i32, i32)], s: (i32, i32), d: (i32, i32)) -> bool {
    s == d || !matches!(plan(walk, s, d, false), Plan::Fail)
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

    /// TryDirect 0x457c64 from (x0, y0).
    fn try_direct(&mut self, walk: &[(i32, i32)], s: (i32, i32), d: (i32, i32), force: bool) -> bool {
        if d == s {
            return true;
        }
        match plan(walk, s, d, force) {
            Plan::Fail => false,
            Plan::Straight => {
                self.start_segment(d, None);
                true
            }
            Plan::Corner(c) => {
                self.start_segment(c, Some(d));
                true
            }
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
        let s = self.pos();
        while !self.try_direct(walk, s, self.dest, false) {
            match self.pick_waypoint(room, s) {
                None => {
                    self.try_direct(walk, s, self.dest, true);
                    return;
                }
                Some(i) => {
                    self.tried[i] = true;
                    self.via_waypoint = true;
                    if self.try_direct(walk, s, room.waypoints[i], false) {
                        self.visited[i] = true;
                        return;
                    }
                }
            }
        }
    }

    /// 0x4579c0.
    fn pick_waypoint(&self, room: &Room, s: (i32, i32)) -> Option<usize> {
        let d = self.dest;
        let direct = angle(s.0, s.1, d.0, d.1);
        let mut best = 1_000_000.0f64;
        let mut res = None;
        for (i, &w) in room.waypoints.iter().enumerate().take(20) {
            if self.visited[i] || self.tried[i] || w.0 == s.0 || w.1 == s.1 || w.0 == d.0 || w.1 == d.1 {
                continue;
            }
            let d1 = (((s.0 - w.0).pow(2) + (s.1 - w.1).pow(2)) as f64).sqrt();
            let d2 = (((d.0 - w.0).pow(2) + (d.1 - w.1).pow(2)) as f64).sqrt();
            let mut cost = d1 + d2;
            if can_reach(&room.walk.points, w, d) {
                cost -= 50.0;
            }
            let mut ad = (angle(s.0, s.1, w.0, w.1) - direct).abs();
            if ad > 180 {
                ad = 360 - ad;
            }
            cost += (ad / 2) as f64;
            if cost < best {
                best = cost;
                res = Some(i);
            }
        }
        res
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
            let d = self.target_dir - self.dir;
            if d.abs() == 1 {
                self.dir = self.target_dir;
            } else if (d + 8) % 8 > (self.dir - self.target_dir + 8) % 8 {
                self.dir = (self.dir + 7) % 8;
            } else {
                self.dir = (self.dir + 1) % 8;
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
                self.x = advance(self.x, ex, if ex < sx { -step } else { step }, self.via_waypoint);
                self.y = sy + (self.x - sx).abs() * (ey - sy) / (ex - sx).abs();
            } else {
                self.y = advance(self.y, ey, if sy > ey { -step } else { step }, self.via_waypoint);
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

/// Step `v` by signed `step` toward `to`; overshoot snaps to `to`, or with the waypoint tolerance
/// steps back again.
fn advance(v: i32, to: i32, step: i32, tolerant: bool) -> i32 {
    let n = v + step;
    let over = if step >= 0 { n > to } else { n < to };
    match (over, tolerant) {
        (false, _) => n,
        (true, false) => to,
        (true, true) => n - step,
    }
}
