//! wasm-bindgen wrapper around `ostrov_core::app::App` for the browser player.

use js_sys::{Array, Float32Array, Object, Reflect, Uint8Array};
use ostrov_core::{app::App, assets::Assets, config::Config, input::Event, screen::rgb565_to_rgba};
use std::{collections::HashMap, rc::Rc};
use wasm_bindgen::prelude::*;

fn entries(obj: &Object) -> Vec<(String, Vec<u8>)> {
    Object::entries(obj)
        .iter()
        .filter_map(|e| {
            let e = Array::from(&e);
            Some((e.get(0).as_string()?, e.get(1).dyn_into::<Uint8Array>().ok()?.to_vec()))
        })
        .collect()
}

#[wasm_bindgen]
pub struct Game {
    app: App,
    last_frame: u64,
}

#[wasm_bindgen]
impl Game {
    /// `files`: game files by name (any case). `saves`: "SaveGame/…" paths.
    #[wasm_bindgen(constructor)]
    pub fn new(files: Object, cfg: Option<Uint8Array>, saves: Object, seed: u32) -> Result<Game, JsError> {
        let mut game: HashMap<String, Vec<u8>> = HashMap::new();
        for name in Object::keys(&files).iter().filter_map(|k| k.as_string()) {
            let v = Reflect::get(&files, &JsValue::from_str(&name)).ok();
            if let Some(a) = v.and_then(|v| v.dyn_into::<Uint8Array>().ok()) {
                game.insert(name.to_ascii_lowercase(), a.to_vec());
                // Free the JS copy as soon as it is in wasm memory.
                let _ = Reflect::delete_property(&files, &JsValue::from_str(&name));
            }
        }
        let assets = Assets::load(|n| game.remove(&n.to_ascii_lowercase())).map_err(|e| JsError::new(&e.to_string()))?;
        drop(game);
        let cfg = cfg.map(|c| Config::parse(&c.to_vec())).unwrap_or_default();
        let app = App::new(Rc::new(assets), cfg, seed, entries(&saves).into_iter().collect());
        Ok(Game { app, last_frame: 0 })
    }

    pub fn tick(&mut self, now_ms: f64) {
        self.app.run_until(now_ms.max(0.0) as u64);
    }

    /// Copies the latest frame (RGBA 640×480) into `dst`; false if unchanged since the last call.
    pub fn frame(&mut self, dst: &mut [u8]) -> bool {
        let io = self.app.io.borrow();
        if io.frames == self.last_frame {
            return false;
        }
        self.last_frame = io.frames;
        rgb565_to_rgba(&io.frame, dst);
        true
    }

    #[wasm_bindgen(js_name = takeAudio)]
    pub fn take_audio(&mut self) -> Float32Array {
        let s = std::mem::take(&mut self.app.io.borrow_mut().audio);
        let f: Vec<f32> = s.iter().map(|&v| v as f32 / 32768.0).collect();
        Float32Array::from(&f[..])
    }

    /// Files the game wrote since the last call, as `[path, bytes]` pairs.
    #[wasm_bindgen(js_name = takeWrites)]
    pub fn take_writes(&mut self) -> Array {
        let mut io = self.app.io.borrow_mut();
        let writes = std::mem::take(&mut io.writes);
        let out = Array::new();
        for (name, data) in writes {
            out.push(&Array::of2(&JsValue::from_str(&name), &Uint8Array::from(&data[..])));
            io.files.insert(name, data);
        }
        out
    }

    fn send(&self, e: Event) {
        self.app.io.borrow_mut().input.apply(e);
    }

    #[wasm_bindgen(js_name = mouseMove)]
    pub fn mouse_move(&self, x: i32, y: i32) {
        self.send(Event::MouseMove(x.clamp(0, 639), y.clamp(0, 479)));
    }

    #[wasm_bindgen(js_name = mouseDown)]
    pub fn mouse_down(&self, x: i32, y: i32, right: bool) {
        self.send(Event::MouseDown { x: x.clamp(0, 639), y: y.clamp(0, 479), right });
    }

    #[wasm_bindgen(js_name = mouseUp)]
    pub fn mouse_up(&self, right: bool) {
        self.send(Event::MouseUp { right });
    }

    #[wasm_bindgen(js_name = keyDown)]
    pub fn key_down(&self, code: u16) {
        self.send(Event::KeyDown(code));
    }

    pub fn text(&self, s: &str) {
        for c in ostrov_core::reader::to_cp1250(s) {
            if c >= 0x20 {
                self.send(Event::Char(c));
            }
        }
    }

    /// Hotspot highlight key held (Space).
    pub fn highlight(&self, on: bool) {
        self.send(Event::Highlight(on));
    }

    /// Picture setting from the F1 panel: 0 sharp integer, 1 sharp fill, 2 smooth, 3 CRT.
    pub fn scaling(&self) -> u8 {
        use ostrov_core::settings::Scaling;
        match self.app.io.borrow().settings.scaling {
            Scaling::Sharp => 0,
            Scaling::Fill => 1,
            Scaling::Smooth => 2,
            Scaling::Crt => 3,
        }
    }

    /// Asks the game to quit; it writes the autosave and Main.cfg on its next ticks.
    pub fn quit(&self) {
        self.app.io.borrow_mut().quit = true;
    }

    pub fn finished(&self) -> bool {
        self.app.finished()
    }
}
