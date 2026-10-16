use ostrov_core::{archive::Archive, reader::Reader, world};

fn main() {
    let dir = std::env::args().nth(1).expect("game dir");
    let data = std::fs::read(format!("{dir}/Data.dat")).unwrap();
    let ar = Archive::parse(&data).unwrap();
    let e = ar.get(0x30).unwrap();
    let rooms = world::rooms(e).unwrap();
    let mut r = Reader::new(e);
    r.i32().unwrap();
    let regions: usize = rooms.iter().map(|r| r.regions.len()).sum();
    let words: usize = rooms.iter().flat_map(|r| r.regions.iter().chain([&r.walk])).map(|g| g.script.len()).sum();
    println!("rooms {} regions {regions} words {words}", rooms.len());
    let mut bad = 0;
    for rm in &rooms {
        for g in rm.regions.iter().chain([&rm.walk]) {
            let mut i = 0;
            while i < g.script.len() {
                let op = (g.script[i] & 0x7f) as usize;
                if op >= world::ARGC.len() { bad += 1; break }
                i += 1 + world::ARGC[op];
            }
            if i != g.script.len() { bad += 1 }
        }
    }
    println!("scripts with bad framing: {bad}");
    println!("lines {} dialogs {}", world::lines(ar.get(0x23).unwrap()).unwrap().len(), world::dialogs(ar.get(0x24).unwrap()).unwrap().len());
}
