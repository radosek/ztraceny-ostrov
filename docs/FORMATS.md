# File formats

## Archives (`*.dat` except Anim2/Graph/Install)
`u32 count`, then `count × (u32 offset, u32 size)`; `0xffffffff` offset = empty slot. Little-endian.

| File | Entries used / slots | Content |
|---|---|---|
| Anim.dat | 57 / 1582 | Autodesk FLC (`0xAF12`), 640×480×8 full-screen cutscenes |
| Partial.dat | 425 / 2001 | 304 FLC + 121 entries with magic `0x00ff` at +4 (TBD) |
| Backgnd.dat | 640 / 1931 | scene images (640×420) + 1024-byte palettes (TBD) |
| Data.dat | 73 / 103 | TBD |
| Music/Speech/Fx/Fx2.dat | | audio, header magic `0x5622` (TBD) |

All standard FLC entries decode cleanly (`cargo run --release -p ostrov-core --example flccheck <game-dir>`):
Anim 9412 frames, Partial 12075 frames. Chunks used: standard set only.

## Video
`Logo.avi`, `Titulky.avi`: Cinepak (`cvid`) 640×480 @ 15 fps, no audio stream.

## Not referenced by Ostrov.exe
`Anim2.dat` — the exe's string table has no reference to it.
