//! Native runner: winit window, pixels (wgpu) presentation, rodio audio, gilrs controllers.

use anyhow::{Context, Result, bail};
use ostrov_core::{
    app::App,
    assets::Assets,
    config::Config,
    input::{Event, KEY_BACK, KEY_ENTER, KEY_ESC},
    mixer::RATE,
    screen::rgb565_to_rgba,
};
use pixels::{Pixels, SurfaceTexture};

mod filter;
use std::{
    collections::{HashMap, VecDeque},
    num::NonZero,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Fullscreen, Window, WindowId},
};

/// Case-insensitive lookup of the game's files (the exe spells them "FX.dat", disks say "Fx.dat").
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name)))
}

fn game_dir() -> Result<PathBuf> {
    let looks_ok = |d: &Path| find(d, "Data.dat").is_some();
    if let Some(a) = std::env::args().nth(1) {
        let p = PathBuf::from(a);
        let p = if p.is_file() { p.parent().map(Path::to_path_buf).unwrap_or(p) } else { p };
        if looks_ok(&p) {
            return Ok(p);
        }
        bail!("{} has no Data.dat", p.display());
    }
    let remembered = state_dir().join("game-dir.txt");
    if let Ok(s) = std::fs::read_to_string(&remembered) {
        let p = PathBuf::from(s.trim());
        if looks_ok(&p) {
            return Ok(p);
        }
    }
    if let Some(p) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf))
        && looks_ok(&p)
    {
        return Ok(p);
    }
    let p = rfd::FileDialog::new().set_title("Složka se hrou Ztracený Ostrov").pick_folder().context("no game folder")?;
    if !looks_ok(&p) {
        bail!("{} has no Data.dat", p.display());
    }
    let _ = std::fs::create_dir_all(state_dir());
    let _ = std::fs::write(remembered, p.to_string_lossy().as_bytes());
    Ok(p)
}

fn state_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("ztraceny-ostrov")
}

fn load_files() -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    let save = state_dir().join("SaveGame");
    if let Ok(rd) = std::fs::read_dir(&save) {
        for e in rd.flatten() {
            if let (Some(n), Ok(d)) = (e.file_name().to_str().map(str::to_owned), std::fs::read(e.path())) {
                m.insert(format!("SaveGame/{n}"), d);
            }
        }
    }
    if let Ok(d) = std::fs::read(state_dir().join(ostrov_core::settings::FILE)) {
        m.insert(ostrov_core::settings::FILE.to_string(), d);
    }
    if let Ok(rd) = std::fs::read_dir(state_dir()) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("lang-") && n.ends_with(".txt")
                && let Ok(d) = std::fs::read(e.path())
            {
                m.insert(n, d);
            }
        }
    }
    m
}

/// Samples produced by the game, consumed by the audio thread.
#[derive(Clone, Default)]
struct Queue(Arc<Mutex<VecDeque<i16>>>);

/// Output rate; the game's 22050 Hz mix is converted with rubato's FFT resampler.
const OUT_RATE: u32 = 48000;

struct Stream {
    queue: Queue,
    resampler: rubato::Fft<f32>,
    out: VecDeque<f32>,
}

impl Stream {
    fn new(queue: Queue) -> Self {
        let resampler = rubato::Fft::<f32>::new(RATE as usize, OUT_RATE as usize, 512, 1, rubato::FixedSync::Input)
            .expect("resampler");
        Self { queue, resampler, out: VecDeque::new() }
    }
}

impl Iterator for Stream {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        use rubato::Resampler;
        if self.out.is_empty() {
            let n = self.resampler.input_frames_next();
            let input: Vec<f32> = {
                let mut q = self.queue.0.lock().unwrap();
                (0..n).map(|_| q.pop_front().map_or(0.0, |s| s as f32 / 32768.0)).collect()
            };
            let adapter = audioadapter_buffers::direct::InterleavedSlice::new(&input, 1, n).expect("buffer");
            match self.resampler.process(&adapter, None) {
                Ok(o) => self.out.extend(o.take_data()),
                Err(_) => self.out.extend(std::iter::repeat_n(0.0, n * 2)),
            }
        }
        Some(self.out.pop_front().unwrap_or(0.0))
    }
}

impl rodio::Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(OUT_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

struct Runner {
    app: App,
    start: Instant,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    audio: Queue,
    _sink: Option<rodio::MixerDeviceSink>,
    gilrs: Option<gilrs::Gilrs>,
    mouse: (i32, i32),
    pad_pos: (f32, f32),
    last_frame: u64,
    mods: winit::keyboard::ModifiersState,
    scaling: ostrov_core::settings::Scaling,
    filter: Option<filter::Filter>,
}

impl Runner {
    fn send(&self, e: Event) {
        self.app.io.borrow_mut().input.apply(e);
    }

    fn persist(&self) {
        let writes = std::mem::take(&mut self.app.io.borrow_mut().writes);
        for (name, data) in writes {
            let path = state_dir().join(&name);
            if let Some(p) = path.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            if let Err(e) = std::fs::write(&path, &data) {
                eprintln!("write {}: {e}", path.display());
            }
            self.app.io.borrow_mut().files.insert(name, data);
        }
    }

    fn pump_gamepad(&mut self) {
        let Some(g) = self.gilrs.as_mut() else { return };
        let mut events = Vec::new();
        while let Some(ev) = g.next_event() {
            use gilrs::{Button, EventType};
            let (x, y) = self.mouse;
            match ev.event {
                EventType::ButtonPressed(Button::South | Button::RightTrigger2, _) => {
                    events.push(Event::MouseDown { x, y, right: false })
                }
                EventType::ButtonPressed(Button::East | Button::LeftTrigger2, _) => events.push(Event::MouseDown { x, y, right: true }),
                EventType::ButtonPressed(Button::Start, _) => events.push(Event::KeyDown(KEY_ESC)),
                EventType::ButtonPressed(Button::North, _) => events.push(Event::KeyDown(b'I' as u16)),
                EventType::AxisChanged(gilrs::Axis::LeftStickX, v, _) => self.pad_pos.0 = v,
                EventType::AxisChanged(gilrs::Axis::LeftStickY, v, _) => self.pad_pos.1 = v,
                _ => {}
            }
        }
        let (dx, dy) = self.pad_pos;
        if dx.abs() > 0.15 || dy.abs() > 0.15 {
            self.mouse.0 = (self.mouse.0 + (dx * 12.0) as i32).clamp(0, 639);
            self.mouse.1 = (self.mouse.1 - (dy * 12.0) as i32).clamp(0, 479);
            events.push(Event::MouseMove(self.mouse.0, self.mouse.1));
        }
        for e in events {
            self.send(e);
        }
    }

    fn step(&mut self) {
        self.pump_gamepad();
        let now = self.start.elapsed().as_millis() as u64;
        self.app.run_until(now);
        self.persist();
        let samples = std::mem::take(&mut self.app.io.borrow_mut().audio);
        {
            let mut q = self.audio.0.lock().unwrap();
            q.extend(samples);
            let max = RATE as usize / 4;
            if q.len() > max {
                let n = q.len() - max;
                q.drain(..n);
            }
        }
        self.apply_settings();
        let io = self.app.io.borrow();
        if io.frames != self.last_frame
            && let Some(px) = self.pixels.as_mut()
        {
            self.last_frame = io.frames;
            rgb565_to_rgba(&io.frame, px.frame_mut());
        }
    }
}

impl Runner {
    /// Fullscreen and scaling come from the game's settings (F1 panel or shortcuts).
    fn apply_settings(&mut self) {
        let s = self.app.io.borrow().settings.clone();
        if let Some(w) = &self.window {
            if s.fullscreen != w.fullscreen().is_some() {
                w.set_fullscreen(s.fullscreen.then_some(Fullscreen::Borderless(None)));
            }
        }
        if s.scaling != self.scaling {
            self.scaling = s.scaling;
            if let Some(p) = self.pixels.as_mut() {
                p.set_scaling_mode(match s.scaling {
                    ostrov_core::settings::Scaling::Sharp => pixels::ScalingMode::PixelPerfect,
                    _ => pixels::ScalingMode::Fill,
                });
            }
        }
    }

    fn toggle_fullscreen(&mut self) {
        let mut io = self.app.io.borrow_mut();
        io.settings.fullscreen = !io.settings.fullscreen;
        let text = io.settings.to_text().into_bytes();
        io.writes.push((ostrov_core::settings::FILE.into(), text));
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (w, h) = std::fs::read_to_string(state_dir().join("window.txt"))
            .ok()
            .and_then(|t| {
                let (a, b) = t.trim().split_once('x')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .unwrap_or((1280u32, 960u32));
        let attrs = Window::default_attributes().with_title("Ztracený Ostrov").with_inner_size(LogicalSize::new(w, h));
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels = Pixels::new(640, 480, surface).expect("pixels");
        self.filter = Some(filter::Filter::new(pixels.context(), pixels.surface_texture_format()));
        window.set_cursor_visible(false);
        self.window = Some(window);
        self.pixels = Some(pixels);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.app.io.borrow_mut().quit = true;
                let t = self.start.elapsed().as_millis() as u64 + 1000;
                self.app.run_until(t);
                self.persist();
                el.exit();
            }
            WindowEvent::Resized(s) => {
                if let Some(p) = self.pixels.as_mut() {
                    let _ = p.resize_surface(s.width, s.height);
                }
                if let Some(w) = &self.window
                    && w.fullscreen().is_none()
                {
                    let l = s.to_logical::<u32>(w.scale_factor());
                    let _ = std::fs::create_dir_all(state_dir());
                    let _ = std::fs::write(state_dir().join("window.txt"), format!("{}x{}", l.width, l.height));
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(p) = &self.pixels {
                    let (x, y) = match p.window_pos_to_pixel((position.x as f32, position.y as f32)) {
                        Ok(v) => v,
                        Err(e) => p.clamp_pixel_pos(e),
                    };
                    self.mouse = (x as i32, y as i32);
                    self.send(Event::MouseMove(self.mouse.0, self.mouse.1));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let right = button == MouseButton::Right;
                if matches!(button, MouseButton::Left | MouseButton::Right) {
                    let (x, y) = self.mouse;
                    self.send(match state {
                        ElementState::Pressed => Event::MouseDown { x, y, right },
                        ElementState::Released => Event::MouseUp { right },
                    });
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: Key::Named(NamedKey::Space), state: ElementState::Released, .. },
                ..
            } => self.send(Event::Highlight(false)),
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, text, .. }, .. } => {
                let cmd = self.mods.super_key() || self.mods.alt_key();
                let fullscreen = match &logical_key {
                    Key::Named(NamedKey::F11) => true,
                    Key::Named(NamedKey::Enter) => cmd,
                    Key::Character(c) => self.mods.super_key() && c.eq_ignore_ascii_case("f"),
                    _ => false,
                };
                if fullscreen {
                    self.toggle_fullscreen();
                    return;
                }
                if self.mods.super_key() {
                    if let Key::Character(c) = &logical_key
                        && c.eq_ignore_ascii_case("q")
                    {
                        self.app.io.borrow_mut().quit = true;
                    }
                    return;
                }
                match &logical_key {
                    Key::Named(NamedKey::Escape) => self.send(Event::KeyDown(KEY_ESC)),
                    Key::Named(NamedKey::F1) => self.send(Event::KeyDown(0x70)),
                    Key::Named(NamedKey::F5) => self.send(Event::KeyDown(0x74)),
                    Key::Named(NamedKey::F9) => self.send(Event::KeyDown(0x78)),
                    Key::Named(NamedKey::F12) => self.send(Event::KeyDown(0x7b)),
                    Key::Named(NamedKey::Space) => self.send(Event::Highlight(true)),
                    Key::Named(NamedKey::Enter) => self.send(Event::KeyDown(KEY_ENTER)),
                    Key::Named(NamedKey::Backspace) => self.send(Event::KeyDown(KEY_BACK)),
                    Key::Named(NamedKey::ArrowLeft) => self.send(Event::KeyDown(0x25)),
                    Key::Named(NamedKey::ArrowRight) => self.send(Event::KeyDown(0x27)),
                    Key::Named(NamedKey::Home) => self.send(Event::KeyDown(0x24)),
                    Key::Named(NamedKey::End) => self.send(Event::KeyDown(0x23)),
                    Key::Named(NamedKey::Delete) => self.send(Event::KeyDown(0x2e)),
                    Key::Character(c) => {
                        if let Some(ch) = c.chars().next() {
                            self.send(Event::KeyDown(ch.to_ascii_uppercase() as u16));
                        }
                    }
                    _ => {}
                }
                if let Some(t) = text {
                    for c in ostrov_core::reader::to_cp1250(t.as_str()) {
                        if c >= 0x20 {
                            self.send(Event::Char(c));
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.step();
                if let (Some(p), Some(w)) = (&self.pixels, &self.window) {
                    use ostrov_core::settings::Scaling;
                    let mode = match self.scaling {
                        Scaling::Smooth => 1,
                        Scaling::Crt => 2,
                        _ => 0,
                    };
                    let size = w.inner_size();
                    let backdrop = self.app.io.borrow().settings.widescreen && size.width * 3 > size.height * 4 + 8;
                    match &self.filter {
                        Some(f) => {
                            let _ = p.render_with(|enc, target, ctx| {
                                f.render(enc, target, ctx, ctx.scaling_renderer.clip_rect(), (size.width, size.height), mode, backdrop);
                                Ok(())
                            });
                        }
                        None => {
                            let _ = p.render();
                        }
                    }
                }
                if self.app.finished() {
                    el.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn main() -> Result<()> {
    let dir = game_dir()?;
    let assets = Assets::load(|name| find(&dir, name).and_then(|p| std::fs::read(p).ok()))?;
    let cfg = std::fs::read(state_dir().join("Main.cfg"))
        .or_else(|_| find(&dir, "Main.cfg").map_or(Err(std::io::ErrorKind::NotFound.into()), std::fs::read))
        .map(|b| Config::parse(&b))
        .unwrap_or_default();
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let seed = (secs.as_millis() % 86_400_000) as u32;
    let app = App::new(Rc::new(assets), cfg, seed, load_files());
    let audio = Queue::default();
    let sink = rodio::DeviceSinkBuilder::open_default_sink().ok();
    if let Some(s) = &sink {
        s.mixer().add(Stream::new(audio.clone()));
    }
    let el = EventLoop::new()?;
    el.set_control_flow(ControlFlow::Poll);
    let mut r = Runner {
        app,
        start: Instant::now(),
        window: None,
        pixels: None,
        audio,
        _sink: sink,
        gilrs: gilrs::Gilrs::new().ok(),
        mouse: (320, 240),
        pad_pos: (0.0, 0.0),
        last_frame: 0,
        mods: Default::default(),
        scaling: ostrov_core::settings::Scaling::Sharp,
        filter: None,
    };
    el.run_app(&mut r)?;
    Ok(())
}
