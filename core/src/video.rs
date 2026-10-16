//! Logo.avi / Titulky.avi: Cinepak 640×480 @ 15 fps, no audio (it comes from FX.dat).

use std::io::Cursor;

pub const FPS: u32 = 15;

pub struct Video {
    chunks: Vec<Vec<u8>>,
    dec: oxideav_cinepak::CinepakDecoder,
    next: usize,
    /// Last decoded frame, RGB24 640×480.
    pub rgb: Vec<u8>,
}

fn collect(c: &riff::Chunk, f: &mut Cursor<&[u8]>, out: &mut Vec<Vec<u8>>) -> std::io::Result<()> {
    for ch in c.iter(f).collect::<Vec<_>>() {
        let ch = ch?;
        let id = ch.id();
        let id = id.as_str();
        if id == "LIST" {
            collect(&ch, f, out)?;
        } else if id.len() == 4 && (id.ends_with("dc") || id.ends_with("db")) {
            out.push(ch.read_contents(f)?);
        }
    }
    Ok(())
}

impl Video {
    pub fn open(avi: &[u8]) -> std::io::Result<Self> {
        let mut f = Cursor::new(avi);
        let root = riff::Chunk::read(&mut f, 0)?;
        let mut chunks = Vec::new();
        collect(&root, &mut f, &mut chunks)?;
        Ok(Self { chunks, dec: oxideav_cinepak::CinepakDecoder::new(), next: 0, rgb: vec![0; 640 * 480 * 3] })
    }

    pub fn frames(&self) -> usize {
        self.chunks.len()
    }

    /// Decode the next frame; false at the end. Empty or undecodable chunks repeat the
    /// previous frame (Titulky.avi ends with a zero-filled one).
    pub fn advance(&mut self) -> bool {
        let Some(c) = self.chunks.get(self.next) else { return false };
        self.next += 1;
        if !c.is_empty()
            && let Ok(img) = self.dec.decode_frame(c, None)
            && img.pixels().len() == self.rgb.len()
        {
            self.rgb.copy_from_slice(img.pixels());
        }
        true
    }
}
