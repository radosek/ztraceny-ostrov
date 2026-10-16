use anyhow::{Context, Result, bail};
use ostrov_core::{archive::Archive, audio, flc::Flc};
use std::{fs, path::{Path, PathBuf}};

fn write_png(path: &Path, w: u32, h: u32, pixels: &[u8], pal: &[[u8; 3]; 256]) -> Result<()> {
    let f = fs::File::create(path)?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Indexed);
    e.set_depth(png::BitDepth::Eight);
    e.set_palette(pal.concat());
    e.write_header()?.write_image_data(pixels)?;
    Ok(())
}

fn extract_flc(name: &str, src: &Path, out: &Path, max_frames: usize) -> Result<()> {
    let data = fs::read(src.join(name)).with_context(|| name.to_string())?;
    let ar = Archive::parse(&data)?;
    let dir = out.join(name.trim_end_matches(".dat"));
    fs::create_dir_all(&dir)?;
    for (i, e) in ar.iter() {
        fs::write(dir.join(format!("{i:04}.flc")), e)?;
        let flc = match Flc::parse(e) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("{name}[{i}]: {err}");
                continue;
            }
        };
        let mut d = flc.decoder();
        let mut n = 0;
        while n < max_frames && d.next_frame().with_context(|| format!("{name}[{i}] frame {n}"))? {
            write_png(&dir.join(format!("{i:04}_{n:04}.png")), flc.width as u32, flc.height as u32, &d.pixels, &d.palette)?;
            n += 1;
        }
    }
    Ok(())
}

fn extract_audio(name: &str, src: &Path, out: &Path) -> Result<()> {
    let data = fs::read(src.join(name)).with_context(|| name.to_string())?;
    let ar = Archive::parse(&data)?;
    let dir = out.join(name.trim_end_matches(".dat"));
    fs::create_dir_all(&dir)?;
    for (i, e) in ar.iter() {
        let s = audio::decode(e).with_context(|| format!("{name}[{i}]"))?;
        let spec = hound::WavSpec { channels: 1, sample_rate: s.sample_rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(dir.join(format!("{i:04}.wav")), spec)?;
        for v in s.samples {
            w.write_sample(v)?;
        }
        w.finalize()?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        bail!("usage: ostrov-extract <game-dir> <out-dir> [max-frames]");
    }
    let src = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let max = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(usize::MAX);
    for name in ["Anim.dat", "Partial.dat"] {
        extract_flc(name, &src, &out, max)?;
    }
    for name in ["Music.dat", "Speech.dat", "Fx.dat", "Fx2.dat"] {
        extract_audio(name, &src, &out)?;
    }
    Ok(())
}
