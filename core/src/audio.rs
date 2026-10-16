//! The game's 4-bit ADPCM sound format (Music/Speech/FX/FX2.dat). Decoder at 0x4557dc.

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("truncated")]
    Truncated,
    #[error("unsupported compression {0:#04x}")]
    Compression(u8),
    #[error("unsupported format flags {0:#04x}")]
    Flags(u8),
    #[error("bad block at {0}")]
    Block(usize),
}

pub struct Sound {
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

const STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
    9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];
const INDEX: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

pub fn decode(d: &[u8]) -> Result<Sound, AudioError> {
    let h = d.get(..12).ok_or(AudioError::Truncated)?;
    let sample_rate = u16::from_le_bytes([h[0], h[1]]) as u32;
    let pcm_size = i32::from_le_bytes(h[6..10].try_into().unwrap());
    let flags = h[10];
    if h[11] != b'c' {
        return Err(AudioError::Compression(h[11]));
    }
    if flags != 0x02 {
        return Err(AudioError::Flags(flags));
    }
    let frames = (pcm_size / 2 - 1).max(0) as usize;
    let mut samples = Vec::with_capacity(frames);
    let (mut pred, mut idx) = (0i32, 0i32);
    let mut p = 12;
    'outer: while samples.len() < frames {
        let b = d.get(p..p + 8).ok_or(AudioError::Truncated)?;
        let n = u16::from_le_bytes([b[0], b[1]]) as usize;
        if n > 0x200 || u32::from_le_bytes(b[4..8].try_into().unwrap()) != 0xDEAF {
            return Err(AudioError::Block(p));
        }
        let data = d.get(p + 8..p + 8 + n).ok_or(AudioError::Truncated)?;
        p += 8 + n;
        for &byte in data {
            for nib in [byte & 15, byte >> 4] {
                let step = STEP[idx as usize];
                let mag = (nib & 7) as i32;
                let diff = step * mag / 4 + step / 8;
                pred = (if nib & 8 != 0 { pred - diff } else { pred + diff }).clamp(-32768, 32767);
                samples.push(pred as i16);
                idx = (idx + INDEX[mag as usize]).clamp(0, 88);
                if samples.len() == frames {
                    break 'outer;
                }
            }
        }
    }
    Ok(Sound { sample_rate, samples })
}
