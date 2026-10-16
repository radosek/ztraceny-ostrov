//! String lists (Data.dat 0x20/0x21/0x22, reader 0x44e2c8): `u32 count; count × xor_str`.
//! Strings are kept as cp1250 bytes; the font maps bytes to glyphs directly.

use crate::reader::{Eof, Reader};

pub fn string_list(d: &[u8]) -> Result<Vec<Vec<u8>>, Eof> {
    let mut r = Reader::new(d);
    let n = r.i32()?;
    (0..n).map(|_| r.xor_str()).collect()
}
