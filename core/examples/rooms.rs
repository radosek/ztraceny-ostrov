use ostrov_core::{assets::Assets, reader::cp1250};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1];
    let id = i32::from_str_radix(a[2].trim_start_matches("0x"), 16).unwrap();
    let find = |name: &str| std::fs::read_dir(dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).and_then(|e| std::fs::read(e.path()).ok());
    let assets = Assets::load(find).unwrap();
    let rooms = assets.rooms().unwrap();
    let r = rooms.iter().find(|r| r.id == id).unwrap();
    println!("{} bg={} anim={},{} pal={} fg={} scale={},{}", cp1250(&r.name), r.bg, r.anim1, r.anim2, r.pal, r.fg, r.scale_a, r.scale_b);
    for (i, g) in r.regions.iter().enumerate() {
        let (x0, y0) = g.points.iter().fold((9999, 9999), |a, p| (a.0.min(p.0), a.1.min(p.1)));
        let (x1, y1) = g.points.iter().fold((-9999, -9999), |a, p| (a.0.max(p.0), a.1.max(p.1)));
        let label = assets.labels.get(g.label.max(0) as usize).map(|l| cp1250(l)).unwrap_or_default();
        println!("{i:2} en={} cur={} bbox=({x0},{y0})-({x1},{y1}) label={label:?} words={}", g.enabled as u8, g.cursor, g.script.len());
    }
}
