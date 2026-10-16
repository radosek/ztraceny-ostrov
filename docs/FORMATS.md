# File formats

## Archives (`*.dat` except Anim2/Graph/Install)
`u32 count`, then `count × (u32 offset, u32 size)`; `0xffffffff` offset = empty slot. Little-endian.

| File | Entries used / slots | Content |
|---|---|---|
| Anim.dat | 57 / 1582 | Autodesk FLC (`0xAF12`), 640×480×8 full-screen cutscenes |
| Partial.dat | 425 / 2001 | 304 FLC + 121 entries with magic `0x00ff` at +4 (TBD) |
| Backgnd.dat | 640 / 1931 | scene images (640×420) + 1024-byte palettes (TBD) |
| Data.dat | 73 / 103 | TBD |
| Music/Speech/Fx/Fx2.dat | 25 / 385 / 94 / 17 | 4-bit ADPCM, see below |

All standard FLC entries decode cleanly (`cargo run --release -p ostrov-core --example flccheck <game-dir>`):
Anim 9412 frames, Partial 12075 frames. Chunks used: standard set only.

## Video
`Logo.avi`, `Titulky.avi`: Cinepak (`cvid`) 640×480 @ 15 fps, no audio stream.

## Not referenced by Ostrov.exe
`Anim2.dat` — never opened by the exe. Raw 4 KB-sector disk dump containing byte-identical copies of
Anim/Backgnd/Data/Fx/Fx2/Music/Partial/Speech.dat, both AVIs and Install.dat (Anim.dat at offset 1,228,800),
plus a different Ostrov.exe build at 375,353,344 (first 4 KB identical). Nothing unique; ignored.

## Sound (Music, Speech, FX, FX2)
Header (parsed at 0x45552c): `u16 sample_rate` (22050), `i32 data_size`, `i32 pcm_size`, `u8 flags` (bit0 stereo, bit1 16-bit; always 0x02), `u8 compression` = `'c'`.
Samples = `pcm_size/2 - 1`. Then blocks: `u16 nbytes (≤0x200), u16 unused, u32 0xDEAF, nbytes data`. State (pred=0, idx=0) persists across blocks.
Nibbles low then high. Per nibble (0x4557dc): `step=STEP[idx]` (IMA table, 0x468d6c), `diff = step*(n&7)/4 + step/8`,
sign from bit 3, clamp s16, `idx += {-1,-1,-1,-1,2,4,6,8}[n&7]` (0x468d4c) clamped 0..88.
