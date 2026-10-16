use ostrov_core::{archive::Archive, flc};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(format!("{}/{}", a[1], a[2])).unwrap();
    let ar = Archive::parse(&data).unwrap();
    for i in a[3..].iter().map(|s| s.parse::<usize>().unwrap()) {
        let e = ar.get(i).unwrap();
        let h = flc::header(e).unwrap();
        // list chunk types of the first 3 frames
        let mut o = h.first_frame;
        let mut kinds = vec![];
        for _ in 0..3.min(h.frames) {
            let size = u32::from_le_bytes(e[o..o + 4].try_into().unwrap()) as usize;
            let n = u16::from_le_bytes([e[o + 6], e[o + 7]]);
            let mut c = o + 16;
            let mut ks = vec![];
            for _ in 0..n {
                let cs = u32::from_le_bytes(e[c..c + 4].try_into().unwrap()) as usize;
                ks.push(u16::from_le_bytes([e[c + 4], e[c + 5]]));
                c += cs;
            }
            kinds.push(ks);
            o += size;
        }
        println!("{i}: delta_only={} frames={} speed={} first chunks {:?}", h.delta_only, h.frames, h.speed_ms, kinds);
    }
}
