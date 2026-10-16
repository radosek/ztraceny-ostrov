//! Input latches as the original form handlers fill them (0x462da4, 0x463b4c, ...).

pub const KEY_ESC: u16 = 0x1b;
pub const KEY_ENTER: u16 = 0x0d;
pub const KEY_BACK: u16 = 0x08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    MouseMove(i32, i32),
    MouseDown { x: i32, y: i32, right: bool },
    MouseUp { right: bool },
    /// Virtual key code (Windows VK values for the keys the game reads).
    KeyDown(u16),
    /// Typed character, cp1250.
    Char(u8),
    /// Hotspot highlight key held (port extra, Space by default).
    Highlight(bool),
}

#[derive(Clone, Default, Debug)]
pub struct Input {
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub moved: bool,
    pub left: bool,
    pub right: bool,
    pub left_up: bool,
    pub right_up: bool,
    pub key_down: bool,
    pub key: u16,
    pub chars: Vec<u8>,
    /// Port extras: highlight key held, and a double click on the last left press.
    pub highlight: bool,
    pub double: bool,
    last_click: Option<(u64, i32, i32)>,
    pub now: u64,
}

impl Input {
    pub fn apply(&mut self, e: Event) {
        match e {
            Event::MouseMove(x, y) => {
                self.mouse_x = x;
                self.mouse_y = y;
                self.moved = true;
            }
            Event::MouseDown { x, y, right } => {
                self.mouse_x = x;
                self.mouse_y = y;
                if right {
                    self.right = true
                } else {
                    self.left = true;
                    self.double = self
                        .last_click
                        .is_some_and(|(t, px, py)| self.now.saturating_sub(t) < 500 && (px - x).abs() < 20 && (py - y).abs() < 20);
                    self.last_click = Some((self.now, x, y));
                }
            }
            Event::MouseUp { right } => {
                if right {
                    self.right_up = true
                } else {
                    self.left_up = true
                }
            }
            Event::KeyDown(k) => {
                self.key_down = true;
                self.key = k;
            }
            Event::Char(c) => self.chars.push(c),
            Event::Highlight(on) => self.highlight = on,
        }
    }

    /// 0x45f7a0 / 0x464880.
    pub fn clear(&mut self) {
        self.moved = false;
        self.left = false;
        self.right = false;
        self.left_up = false;
        self.right_up = false;
        self.key_down = false;
        self.chars.clear();
        self.double = false;
    }

    pub fn esc(&self) -> bool {
        self.key_down && self.key == KEY_ESC
    }

    /// "SKIP" in the blocking loops: click or ESC.
    pub fn skip(&self) -> bool {
        self.left || self.right || self.esc()
    }
}
