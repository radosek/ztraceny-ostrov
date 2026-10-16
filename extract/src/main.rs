//! Dumps every archive entry for inspection. Palette pairing is a guess (nearest palette entry
//! in the same archive over Data.dat #20's UI colours); the real pairing comes from scene records.

use anyhow::{Context, Result, bail};
use ostrov_core::{
    archive::Archive,
    audio,
    flc::{self, PLAYFIELD_W},
    gfx::{self, Image, Kind, Palette, SCENE_PALETTE_FIRST},
    reader::cp1250,
    text,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn write_png(path: &Path, w: u32, h: u32, pixels: &[u8], pal: &Palette, transparent: Option<u8>) -> Result<()> {
    let f = fs::File::create(path)?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Indexed);
    e.set_depth(png::BitDepth::Eight);
    e.set_palette(pal.concat());
    if let Some(t) = transparent {
        let mut trns = vec![255u8; t as usize + 1];
        trns[t as usize] = 0;
        e.set_trns(trns);
    }
    e.write_header()?.write_image_data(pixels)?;
    Ok(())
}

fn write_image(path: &Path, img: &Image, pal: &Palette, transparent: bool) -> Result<()> {
    write_png(path, img.width, img.height, &img.pixels, pal, transparent.then_some(img.transparent))
}

fn scene_palette(base: &Palette, scene: &Palette) -> Palette {
    let mut p = *base;
    p[SCENE_PALETTE_FIRST..].copy_from_slice(&scene[SCENE_PALETTE_FIRST..]);
    p
}

fn dump_flc(dir: &Path, i: usize, e: &[u8], pal: &Palette, max_frames: usize) -> Result<()> {
    let h = flc::header(e)?;
    let mut sink = flc::Indexed {
        pixels: vec![0; PLAYFIELD_W * 480],
        palette: *pal,
        palette_floor: if h.delta_only { SCENE_PALETTE_FIRST } else { 0 },
    };
    let mut pos = h.start();
    let mut n = 0;
    while n < max_frames.min(h.frames as usize) && flc::decode_frame(e, &mut pos, true, &mut sink).with_context(|| format!("frame {n}"))? {
        write_png(&dir.join(format!("{i:04}_{n:04}.png")), 640, 480, &sink.pixels, &sink.palette, None)?;
        n += 1;
    }
    Ok(())
}

fn dump_archive(name: &str, src: &Path, out: &Path, ui: &Palette, max_frames: usize) -> Result<()> {
    let data = fs::read(src.join(name)).with_context(|| name.to_string())?;
    let ar = Archive::parse(&data)?;
    let dir = out.join(name.trim_end_matches(".dat"));
    fs::create_dir_all(&dir)?;
    let palettes: Vec<(usize, Palette)> = ar
        .iter()
        .filter(|(_, e)| gfx::classify(e) == Kind::Palette)
        .map(|(i, e)| Ok((i, gfx::palette(e)?)))
        .collect::<Result<_>>()?;
    let pal_for = |i: usize| {
        palettes
            .iter()
            .min_by_key(|(j, _)| (*j as isize - i as isize).unsigned_abs())
            .map_or(*ui, |(_, p)| scene_palette(ui, p))
    };
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    for (i, e) in ar.iter() {
        let kind = gfx::classify(e);
        *kinds.entry(format!("{kind:?}")).or_default() += 1;
        let pal = if name == "Data.dat" { *ui } else { pal_for(i) };
        let r = (|| -> Result<()> {
            match kind {
                Kind::Rle => write_image(&dir.join(format!("{i:04}.png")), &gfx::rle(e, 0)?.0, &pal, false),
                Kind::Gif => write_image(&dir.join(format!("{i:04}.png")), &gfx::gif(e)?, &pal, false),
                Kind::RleList => {
                    for (k, img) in gfx::rle_list(e)?.iter().enumerate() {
                        write_image(&dir.join(format!("{i:04}_{k:03}.png")), img, &pal, true)?;
                    }
                    Ok(())
                }
                Kind::Flc => dump_flc(&dir, i, e, &pal, max_frames),
                Kind::Palette => Ok(fs::write(dir.join(format!("{i:04}.pal")), e)?),
                Kind::Other => {
                    if name == "Data.dat" && (32..=34).contains(&i) {
                        let s: Vec<String> = text::string_list(e)?.iter().map(|b| cp1250(b)).collect();
                        fs::write(dir.join(format!("{i:04}.txt")), s.join("\n"))?;
                    }
                    Ok(fs::write(dir.join(format!("{i:04}.bin")), e)?)
                }
            }
        })();
        if let Err(err) = r {
            bail!("{name}[{i}] {kind:?}: {err:#}");
        }
    }
    println!("{name}: {kinds:?}");
    Ok(())
}

fn dump_audio(name: &str, src: &Path, out: &Path) -> Result<()> {
    let data = fs::read(src.join(name)).with_context(|| name.to_string())?;
    let ar = Archive::parse(&data)?;
    let dir = out.join(name.trim_end_matches(".dat"));
    fs::create_dir_all(&dir)?;
    for (i, e) in ar.iter() {
        let s = audio::decode(e).with_context(|| format!("{name}[{i}]"))?;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: s.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(dir.join(format!("{i:04}.wav")), spec)?;
        for v in s.samples {
            w.write_sample(v)?;
        }
        w.finalize()?;
    }
    println!("{name}: {} sounds", ar.iter().count());
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        bail!("usage: ostrov-extract <game-dir> <out-dir> [max-frames-per-flc]");
    }
    let src = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let max = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(usize::MAX);
    let data = fs::read(src.join("Data.dat")).context("Data.dat")?;
    let ui = gfx::palette(Archive::parse(&data)?.get(20).context("Data.dat #20")?)?;
    for name in ["Data.dat", "Backgnd.dat", "Partial.dat", "Anim.dat"] {
        dump_archive(name, &src, &out, &ui, max)?;
    }
    for name in ["Music.dat", "Speech.dat", "Fx.dat", "Fx2.dat"] {
        dump_audio(name, &src, &out)?;
    }
    Ok(())
}
