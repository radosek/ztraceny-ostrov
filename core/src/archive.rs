use std::ops::Range;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("archive truncated")]
    Truncated,
    #[error("entry {0} out of bounds")]
    OutOfBounds(usize),
}

/// `u32 count` followed by `count × (u32 offset, u32 size)`; `0xffffffff` marks an empty slot.
pub struct Archive<'a> {
    data: &'a [u8],
    entries: Vec<Option<Range<usize>>>,
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().unwrap()))
}

impl<'a> Archive<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, ArchiveError> {
        let count = u32_at(data, 0).ok_or(ArchiveError::Truncated)? as usize;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let off = u32_at(data, 4 + i * 8).ok_or(ArchiveError::Truncated)?;
            let size = u32_at(data, 8 + i * 8).ok_or(ArchiveError::Truncated)?;
            if off == u32::MAX {
                entries.push(None);
                continue;
            }
            let r = off as usize..off as usize + size as usize;
            if r.end > data.len() {
                return Err(ArchiveError::OutOfBounds(i));
            }
            entries.push(Some(r));
        }
        Ok(Self { data, entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, i: usize) -> Option<&'a [u8]> {
        let r = self.entries.get(i)?.clone()?;
        Some(&self.data[r])
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &'a [u8])> + '_ {
        (0..self.len()).filter_map(|i| Some((i, self.get(i)?)))
    }
}
