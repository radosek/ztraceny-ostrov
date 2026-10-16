use ostrov_core::{archive::Archive, flc};

fn main() {
    let dir = std::env::args().nth(1).expect("game dir");
    for name in ["Anim.dat", "Partial.dat", "Backgnd.dat"] {
        let data = std::fs::read(format!("{dir}/{name}")).unwrap();
        let ar = Archive::parse(&data).unwrap();
        let (mut ok, mut frames) = (0, 0);
        for (i, e) in ar.iter() {
            let Ok(h) = flc::header(e) else { continue };
            let mut sink = flc::Indexed { pixels: vec![0; 640 * 480], palette: [[0; 3]; 256], palette_floor: 0 };
            let mut pos = h.start();
            let mut n = 0;
            while n < h.frames {
                match flc::decode_frame(e, &mut pos, true, &mut sink) {
                    Ok(true) => n += 1,
                    Ok(false) => break,
                    Err(err) => { println!("{name}[{i}] frame {n}: {err}"); break }
                }
            }
            if n != h.frames { println!("{name}[{i}]: {n}/{} frames", h.frames) } else { ok += 1 }
            frames += n as usize;
        }
        println!("{name}: {ok} ok, {frames} frames");
    }
}
