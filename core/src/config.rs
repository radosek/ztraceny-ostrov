//! Main.cfg / Backup.cfg (17 bytes, read 0x459c9c, written 0x45a14c).

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub raw: [u8; 17],
}

impl Default for Config {
    fn default() -> Self {
        let mut raw = [0u8; 17];
        raw[0] = b'.';
        raw[1..8].fill(1);
        raw[12] = 100;
        raw[13] = 100;
        raw[14] = 100;
        raw[15] = 1;
        Self { raw }
    }
}

impl Config {
    pub fn parse(b: &[u8]) -> Self {
        let mut c = Self::default();
        if b.len() >= 17 {
            c.raw.copy_from_slice(&b[..17]);
        }
        c
    }
    pub fn music_volume(&self) -> u8 {
        self.raw[12].min(100)
    }
    pub fn fx_volume(&self) -> u8 {
        self.raw[13].min(100)
    }
    pub fn speech_volume(&self) -> u8 {
        self.raw[14].min(100)
    }
    pub fn subtitles(&self) -> bool {
        self.raw[15] != 0
    }
    pub fn music_variant(&self) -> u8 {
        self.raw[16]
    }
}
