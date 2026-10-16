//! All game archives, loaded once.

use crate::{
    archive::{ArchiveError, Pack},
    audio,
    gfx::{self, GfxError, Image, Palette},
    reader::Eof,
    text,
    world::{self, Line},
};
use std::{collections::HashMap, rc::Rc, sync::Arc};

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("{0}: {1}")]
    Archive(&'static str, ArchiveError),
    #[error("missing {0}")]
    Missing(&'static str),
    #[error(transparent)]
    Eof(#[from] Eof),
    #[error(transparent)]
    Gfx(#[from] GfxError),
}

pub const FILES: [&str; 8] =
    ["Anim.dat", "Backgnd.dat", "Partial.dat", "Data.dat", "Music.dat", "FX.dat", "FX2.dat", "Speech.dat"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arc8 {
    Anim,
    Backgnd,
    Partial,
    Data,
    Music,
    Fx,
    Fx2,
    Speech,
}

pub struct Assets {
    packs: [Pack; 8],
    pub items: Vec<Vec<u8>>,
    pub labels: Vec<Vec<u8>>,
    pub texts: Vec<Vec<u8>>,
    pub lines: Vec<Line>,
    pub dialogs: Vec<(i16, i16)>,
    pub npcs: Vec<(u16, u16)>,
    pub npc_anims: Vec<(u16, u16)>,
    pub npc_pos: Vec<(u16, u16)>,
    pub font: Vec<u8>,
    pub videos: HashMap<&'static str, Vec<u8>>,
    sounds: std::cell::RefCell<HashMap<(Arc8, i32), Arc<[i16]>>>,
    lists: std::cell::RefCell<HashMap<i32, Rc<Vec<Image>>>>,
}

impl Assets {
    /// `read(name)` returns a file's bytes; names are as the exe spells them (case may differ on disk).
    pub fn load(mut read: impl FnMut(&str) -> Option<Vec<u8>>) -> Result<Self, AssetError> {
        let mut packs = Vec::with_capacity(8);
        for name in FILES {
            let d = read(name).ok_or(AssetError::Missing(name))?;
            packs.push(Pack::new(d).map_err(|e| AssetError::Archive(name, e))?);
        }
        let packs: [Pack; 8] = packs.try_into().ok().unwrap();
        let videos = ["Logo.avi", "Titulky.avi"].into_iter().filter_map(|n| Some((n, read(n)?))).collect();
        let data = &packs[Arc8::Data as usize];
        Ok(Self {
            items: text::string_list(data.entry(0x20))?,
            labels: text::string_list(data.entry(0x21))?,
            texts: text::string_list(data.entry(0x22))?,
            lines: world::lines(data.entry(0x23))?,
            dialogs: world::dialogs(data.entry(0x24))?,
            npcs: world::pairs(data.entry(0x25))?,
            npc_anims: world::pairs(data.entry(0x26))?,
            npc_pos: world::pairs(data.entry(0x27))?,
            font: data.entry(0x15).to_vec(),
            videos,
            packs,
            sounds: Default::default(),
            lists: Default::default(),
        })
    }

    pub fn raw(&self, a: Arc8, i: i32) -> &[u8] {
        self.packs[a as usize].entry(i)
    }

    pub fn exists(&self, a: Arc8, i: i32) -> bool {
        usize::try_from(i).is_ok_and(|i| self.packs[a as usize].exact(i).is_some())
    }

    pub fn palette(&self, a: Arc8, i: i32) -> Result<Palette, GfxError> {
        gfx::palette(self.raw(a, i))
    }

    /// Image class factory 0x459990: type 1 → GIF, else RLE.
    pub fn image(&self, a: Arc8, i: i32, kind: u8) -> Result<Image, GfxError> {
        let d = self.raw(a, i);
        if kind == 1 { gfx::gif(d) } else { Ok(gfx::rle(d, 0)?.0) }
    }

    pub fn sprite_list(&self, i: i32) -> Result<Rc<Vec<Image>>, GfxError> {
        if let Some(l) = self.lists.borrow().get(&i) {
            return Ok(l.clone());
        }
        let l = Rc::new(gfx::rle_list(self.raw(Arc8::Data, i))?);
        self.lists.borrow_mut().insert(i, l.clone());
        Ok(l)
    }

    pub fn sound(&self, a: Arc8, i: i32) -> Option<Arc<[i16]>> {
        if let Some(s) = self.sounds.borrow().get(&(a, i)) {
            return Some(s.clone());
        }
        let s: Arc<[i16]> = audio::decode(self.raw(a, i)).ok()?.samples.into();
        if a != Arc8::Music {
            self.sounds.borrow_mut().insert((a, i), s.clone());
        }
        Some(s)
    }

    pub fn rooms(&self) -> Result<Vec<world::Room>, Eof> {
        world::rooms(self.raw(Arc8::Data, 0x30))
    }
}
