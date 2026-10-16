//! Run the game without a window: headless <game-dir> <out-dir> <script>
//! script: comma list of `t=ms` (dump frame), `c=ms:x:y` (left click), `r=ms:x:y` (right click),
//! `k=ms:code` (key), `m=ms:x:y` (move). Frames are written as PNG.
use ostrov_core::{app::App, assets::Assets, config::Config, input::Event, screen::rgb565_to_rgba};
use std::{collections::HashMap, rc::Rc};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1];
    let out = std::path::PathBuf::from(&a[2]);
    std::fs::create_dir_all(&out).unwrap();
    let find = |name: &str| {
        std::fs::read_dir(dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).and_then(|e| std::fs::read(e.path()).ok())
    };
    let assets = Assets::load(find).expect("assets");
    let mut app = App::new(Rc::new(assets), Config::default(), 12345, HashMap::new());
    let mut steps: Vec<(u64, String, Vec<i32>)> = a[3]
        .split(',')
        .map(|s| {
            let (k, v) = s.split_once('=').unwrap();
            let mut it = v.split(':').map(|x| x.parse::<i64>().unwrap());
            let t = it.next().unwrap() as u64;
            (t, k.to_string(), it.map(|x| x as i32).collect())
        })
        .collect();
    steps.sort_by_key(|s| s.0);
    for (t, k, v) in steps {
        app.run_until(t);
        let mut io = app.io.borrow_mut();
        match k.as_str() {
            "t" => {
                let mut rgba = vec![0u8; 640 * 480 * 4];
                rgb565_to_rgba(&io.frame, &mut rgba);
                let f = std::fs::File::create(out.join(format!("{t:08}.png"))).unwrap();
                let mut e = png::Encoder::new(f, 640, 480);
                e.set_color(png::ColorType::Rgba);
                e.write_header().unwrap().write_image_data(&rgba).unwrap();
            }
            "c" | "r" => {
                io.input.apply(Event::MouseMove(v[0], v[1]));
                io.input.apply(Event::MouseDown { x: v[0], y: v[1], right: k == "r" });
            }
            "m" => io.input.apply(Event::MouseMove(v[0], v[1])),
            "k" => io.input.apply(Event::KeyDown(v[0] as u16)),
            _ => {}
        }
        if !io.frame.is_empty() && k == "t" {
            eprintln!("t={t} frames={} room={:#x}", io.frames, io.room);
        }
    }
}
