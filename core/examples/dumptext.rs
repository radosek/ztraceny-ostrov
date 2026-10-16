//! Dump the game's text lists as `T<idx>\t<text>` (dialogue), `I<idx>` (items), `L<idx>` (hotspot
//! labels), UTF-8 with CR as \r. Used to make translation files; prints nothing from the repo.
use ostrov_core::{assets::Assets, reader::cp1250};
fn main() {
    let dir = std::env::args().nth(1).expect("game dir");
    let find = |name: &str| std::fs::read_dir(&dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).and_then(|e| std::fs::read(e.path()).ok());
    let a = Assets::load(find).unwrap();
    for (tag, list) in [("T", &a.texts), ("I", &a.items), ("L", &a.labels)] {
        for (i, t) in list.iter().enumerate() {
            println!("{tag}{i}\t{}", cp1250(t).replace('\r', "\\r").replace('\n', "\\n"));
        }
    }
}
