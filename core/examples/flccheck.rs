use ostrov_core::{archive::Archive, flc::Flc};

fn main() {
    let dir = std::env::args().nth(1).expect("game dir");
    for name in ["Anim.dat", "Partial.dat"] {
        let data = std::fs::read(format!("{dir}/{name}")).unwrap();
        let ar = Archive::parse(&data).unwrap();
        let (mut ok, mut frames) = (0, 0);
        for (i, e) in ar.iter() {
            let Ok(f) = Flc::parse(e) else { continue };
            let mut d = f.decoder();
            let mut n = 0;
            loop {
                match d.next_frame() {
                    Ok(true) => n += 1,
                    Ok(false) => break,
                    Err(err) => { println!("{name}[{i}] frame {n}: {err}"); break }
                }
            }
            if n != f.frames as usize { println!("{name}[{i}]: {n}/{} frames", f.frames) } else { ok += 1 }
            frames += n;
        }
        println!("{name}: {ok} ok, {frames} frames");
    }
}
