//! Little-endian cursor over Delphi stream dumps.

#[derive(Debug, thiserror::Error)]
#[error("unexpected end of data at {0}")]
pub struct Eof(pub usize);

pub struct Reader<'a> {
    d: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(d: &'a [u8]) -> Self {
        Self { d, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], Eof> {
        let s = self.d.get(self.pos..self.pos + n).ok_or(Eof(self.pos))?;
        self.pos += n;
        Ok(s)
    }

    pub fn u8(&mut self) -> Result<u8, Eof> {
        Ok(self.bytes(1)?[0])
    }

    pub fn i16(&mut self) -> Result<i16, Eof> {
        Ok(i16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn u16(&mut self) -> Result<u16, Eof> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn i32(&mut self) -> Result<i32, Eof> {
        Ok(i32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn f64(&mut self) -> Result<f64, Eof> {
        Ok(f64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    /// XOR-chained string (0x44e428), returned as raw cp1250 bytes.
    pub fn xor_str(&mut self) -> Result<Vec<u8>, Eof> {
        let n = self.u16()? as usize;
        let mut k = 0x55;
        Ok(self
            .bytes(n)?
            .iter()
            .map(|&c| {
                let v = c ^ k;
                k = c;
                v
            })
            .collect())
    }

    pub fn remaining(&self) -> usize {
        self.d.len() - self.pos
    }
}

pub fn cp1250(b: &[u8]) -> String {
    encoding_rs::WINDOWS_1250.decode_without_bom_handling(b).0.into_owned()
}
