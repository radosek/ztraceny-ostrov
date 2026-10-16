# File formats

Derived from Ostrov.exe disassembly (sha256 fc1241fe…6af6, Delphi, image base 0x400000; CODE raw 0x400 -> VA 0x401000, DATA raw 0x67200 -> VA 0x468000).
Rust implementation in `core/`. Every format was decoded and checked visually.

## 0. Archive container (all *.dat except Graph/Install)
```
u32 count
count × { u32 offset, u32 size }     offset == 0xFFFFFFFF -> empty slot
```
Archive object: class created by `FUN_0044e5c8(&0x44e144, 1, name)`; opened in `0x4599bc`. Handles:
`0x46910c` Anim, `0x469114` Partial, `0x469110` Backgnd, `0x469118` Data, `0x46911c` Music, `0x469120` FX, `0x469124` FX2, `0x469128` Speech.
`0x44e6a8(arc, idx)` seeks the archive stream (`arc+4`) to the entry; loaders then read from that stream.
`0x44e4e4(obj, arc, idx)` = seek + `obj.vmt[0](stream)` (generic "load object from entry").

Entry type is never stored. The game knows the type from the caller. Entry kinds and counts:

| archive | rle | gif | pal | flc (0xAF12) | flc (0x00FF) | other | empty |
|---|---|---|---|---|---|---|---|
| Backgnd | 328 | 89 | 115 | 4 | 104 | – | 1291 |
| Partial | – | – | – | 304 | 121 | – | 1576 |
| Anim | – | – | – | 57 | – | – | 1525 |
| Data | 41 + 19 lists | – | 2 | – | 1 | 10 (see §5) | 30 |

## 1. Palette (1024 bytes)
256 × `{B, G, R, 0}`, 8-bit values that are 6-bit×4 (max 0xFC). Memory copy is at `0x46c498` (same layout).
- `0x44ed98(arc, idx)`: loads all 256 entries, then rebuilds the LUT. Used only for Data #20 (menu, `0x4648ea`).
- `0x44edb0(arc, idx)`: loads 1024 bytes but copies **only entries 56..255** (from file offset 0xE0 to `0x46c578`). Entries 0..55 are the fixed UI colours. Used for scene palettes (`0x45c274`, scene field +0x54, Backgnd) and Data #40.
- `0x44ed18`: rebuilds the 16-bit hicolor LUT `u16 0x46c898[256]`: `((B>>sB)<<pB)|((G>>sG)<<pG)|((R>>sR)<<pR)`. Shifts are at `0x46ceac..0x46ceb5` and come from the DirectDraw surface pixel format. The screen is 16 bpp. Every blitter converts 8-bit indices through this LUT.
- FLC COLOR_256 writes into `0x46c498` but skips indices `< *0x468b2c`. Before playing, `0x45b2f0` sets that value to 0 (full-screen cut-scene) or 0x38 (protect UI colours).

## 2. TRLEImage (VMT 0x44e7c8) – Backgnd, Data
Load `0x44f1d0`, unpack `0x44f1a4`, encoder `0x44f090`.
```
u32 w; u32 h; u32 packed_size; u8 transparent_index; u8 data[packed_size]
data: repeat until consumed:
   s8 c = *p++;
   c >= 0 : run   – next byte repeated c times
   c <  0 : literal – copy -c bytes
```
The sample `c6 32 c6 ff a1 02 c6 …` reads as: trans=0xC6, 0x32×C6, literal{A1}, 2×C6, …
The encoder sets trans = first pixel. Literal and run lengths are capped at 127.

**TRLEImageList** (VMT 0x44e8a4, load `0x44f358`, save `0x44f414`): `u32 count` followed by `count` TRLEImage records back to back. All images in one list have the same w/h.

## 3. TGIFImage (VMT 0x44e838) – Backgnd type-1 backgrounds
Load `0x450188`, unpack `0x450134` (LZW init `0x44ea5c`, line decode `0x44ec28`, end `0x44eb54`).
This is a normal GIF87/89 file where the 6-byte signature is replaced by `u32 total_entry_size` + 2 zero bytes. The game raises `"Niekto zasa zabudol upraviť hlavičku GIFu"` if the first 4 bytes are `GIF8`.
Requirements: a global colour table must be present (it is skipped, the game palette is used instead). The image descriptor must have no local colour table. The LSD background byte is stored as the transparent index.
Decode: replace bytes 0..5 with `GIF89a`, then decode with any GIF library and take the raw indices (`decode_gif`).
Which class is used comes from scene field +0x4e or object field +0x24 (`0x459990`: 0 -> TRLEImage, 1 -> TGIFImage).

## 4. FLC (TFlic, VMT 0x44e910) – Anim, Partial, Backgnd, Data #42
Open: `0x44f4bc(obj, arc, idx, mode)` -> `0x44f4e4`. It reads the 128-byte header to obj+5 and accepts magic **0xAF12 or 0x00FF**; anything else raises "Not a real FLC file.". Mode 0 loads the whole file into memory if size ≤ 0x80000, mode 1 always loads it, mode 2 streams it. The frame buffer is 0x4b000 ("Frame too big to fit in buffer.").
Standard header fields: size, frames (+6), w/h=640×480, depth 8, speed, oframe1 (+0x50), oframe2 (+0x54). After the last frame the player seeks back to oframe2 (`0x44f744` loop logic). `0x44fb54` = seek to frame, `0x44fc10` = rewind, `0x44f4b0` = "has loop".

Frame decoder `0x44f744(flic, dst16, pitch)`. The frame type 0xF1FA is not checked. Chunks:
| type | meaning | notes |
|---|---|---|
| 4 COLOR_256 | standard, 8-bit RGB | only applied if flic+0x94 (flag) != 0. Writes B,G,R into 0x46c498 (skips idx < *0x468b2c), then rebuilds the LUT and redraws the whole frame from the index buffer |
| 7 DELTA_FLC (SS2) | standard | 0xC000 words = line skip; **0x8000 "last byte" words are read and ignored** |
| 15 BYTE_RUN | standard, but **hard-coded 640 wide × 420 rows**, skips the packet-count byte | |
| 0x80 (custom) | `u16 count; count × { u16 x, u16 y, u16 w, u16 h, u8 pix[w*h] }`, opaque raw rectangles | starts most Partial AF12 anims (e.g. Partial #1: one 80×100 rect at 487,147). Writes **only** the 16-bit screen, not the index mirror |
| 18 PSTAMP, others | skipped | |

All drawing happens on a 640×420 playfield (the bottom 60 lines are UI). The header says 480, but the decoder never uses those extra lines.
Every chunk except 0x80 also writes the 8-bit index mirror `*0x468b3c` (640×480).

Chunk usage across all files: Anim = 15, 7, 4, 18 (normal cut-scenes, BRUN first frame). Partial AF12 = mostly 0x80 + 7, a few 15/4/18. **0x00FF variant (Partial 121, Backgnd 104, Data #42) = only 7, with an occasional 4.**
The 0x00FF variant has no key frame. It is a pure delta stream meant to run on top of the current scene image. Apart from the magic it is treated exactly like 0xAF12.

**Compositing:** there is no transparency key and no position header. Deltas use absolute 640×420 screen coordinates, and pixels a delta doesn't touch stay as they are, so placement comes from the authoring coordinates. The 0x80 rects carry their own x/y.
- Scene ambient loops (Backgnd, scene fields +0x50 / +0x52, objects at scene+0x120 / +0x124): `0x45ba34` / `0x45ba70` load them. `0x44fc40(flic, screen, pitch, 0x500, bgbuf)` steps a frame onto the screen and, when bgbuf != 0, steps the same frame onto the clean background buffer `*0x468b30` (pitch 0x500). The result is permanent, so sprites restore over it correctly.
- One-shot Partial anims (script ops 3/0x22 -> `0x45ac50(scene, partialIdx, …, mode, fx)`): each frame is drawn to the screen (`0x44f744`), and in some modes again into `*0x468b30`. Character sprites are erased and redrawn around each frame (`0x457080` / `0x457bec`).
- Full-screen cut-scenes (Anim, script ops 4/0x27 -> `0x45b2f0(…, animIdx, frameDelayMs (default 0x72), …, fxIdx)`): palette chunks are enabled (+0x94 = 1), the palette is saved and restored, and ESC skips.
- Note: Backgnd #5 (00FF) decoded over Backgnd #3 draws a column of index 5 (UI yellow) at x=254. Neighbouring entries are therefore not always a pair; the real pairing comes from scene records (+0x4c/+0x50/+0x54).

## 5. Scenes, sprites and the priority mask
Scene record fields (scene object = `*(game+0x118)`):
`+0x4c i16` background image (Backgnd); `+0x4e u8` image class (0 RLE / 1 GIF); `+0x50 i16` ambient FLC A (Backgnd); `+0x52 i16` ambient FLC B; `+0x54 i16` palette (Backgnd, colours 56..255); `+0x5c i32` **priority mask** (Backgnd RLE, 640×420).
Static scene objects (list via `0x4510d8` count / `0x4510e0` get): `+0x14 u8` visible, `+0x22 i16` image idx (Backgnd; negative = transparent blit), `+0x24 u8` class, `+0x26 i16` x, `+0x28 i16` y.

Draw scene `0x45b5b0`:
1. Decode the bg into the screen (`0x44fec4`, opaque).
2. `Move(*0x468b30 -> *0x468b34)`, 0x96000 bytes.
3. Draw the visible objects: opaque via `0x44fec4(img,x,y,420,0x500)`, transparent via `0x44ffa0` (skips pixels == trans).

`0x45baac` erases or redraws a single object (`0x44eea4` = rect copy from the background buffer).
Mask `0x45c1fc`: TRLEImage.unpack straight into `*0x468b38` (8-bit, 640 wide). Values are small depth layers 0..n (see `mask.png`).
Sprite blit `0x44fe5c` -> `0x44fcac(img, depth, h, w, useTrans, flipX, y, x)`: writes a pixel only if `mask[y][x] <= depth` and `pix != trans` (trans forced to 0xFF when useTrans=0). Optional horizontal mirror. Clipping is 0..639 and `< *0x46cea0`.

Global buffers (allocated in `0x450624`):
- `0x468b4c`: 0x4b000 scratch for decoded 8-bit images and FLC frames.
- `0x468b30`: 0x96000, 16-bit clean background plus objects.
- `0x468b34`: 0x96000, 16-bit copy.
- `0x468b38`: 0x4b000 priority mask.
- `0x468b3c`: 0x4b000 8-bit index mirror.

Screen pointer is `0x46ce98`, pitch `0x46ce9c`, height `0x46cea0`. Surface lock/unlock appear to be `0x44edf4` / `0x44ee44`.
Font glyph blitter: `0x4516e8` (17×10 glyphs, 0xAA bytes each, at `*0x468cac`; value 1 = colour, 2 = palette[0]).

## 6. Data.dat classification (103 slots)
| # | content | user |
|---|---|---|
| 1,2,4,5 | RLE list 49× 56×151: hero costume sprite sets (walk/turn frames) | script op 0x14 sets game+0x14 = list idx; `0x45d7ec` default = 3 |
| 3 | RLE list 49× 46×146: default hero | `0x45d7ec`, `0x45a42c` |
| 6,7 | RLE list 49× 56×155: hero variants | op 0x14 |
| 8–11 | RLE list 49× 143×74 (lying/crawling hero?) | op 0x14 |
| 12 | RLE list 49× 138×104 | op 0x14 |
| 13 | RLE list 49× 165×141 | op 0x14 |
| 14 | RLE list 60× 142×70 | op 0x14 |
| 15 | RLE list 49× 69×153: second character | `0x45a42c` (idx 0xf) |
| 16 | RLE list 4× 320×60: menu UI strip | `0x464a04` |
| 17 | RLE list 97× 44×44: **inventory icons** (97 = number of item names in #32) | `0x4649bb` |
| 18 | RLE list 48× 28×28: cursors/small icons | `0x464959` |
| 19 | RLE 640×480: menu/inventory screen background | `0x464999` |
| 20 | palette (full 256, menu) | `0x4648ea` |
| 21 | 24310 raw bytes, values 0..2 (looks like a small grid map) | no immediate reference found |
| 32 | string list, 97 inventory item names ("Liána", "Pohrabáč", …) | `0x464a36`, game+0x320 |
| 33 | string list, 1605 hotspot/object names ("Lano", "Kapitán", …) | `0x464a66`, game+0x324 |
| 34 | string list, 1633 dialogue/subtitle lines | `0x459bb4` -> `*0x469134` |
| 35 | `u32 n(=0x800)` + n×{i16 speaker (0=current char), i16 textIdx (0=same as line id), i16 speechIdx in Speech.dat (0=same)}: dialogue line table | `*0x469138`, used by `0x45d908` (say line) |
| 36 | `u32 n(=1544)` + n×{i16 firstLine, i16 lastLine}: dialogue sequences | `*0x46913c`, `0x45da10` (advance) |
| 37 | `u32 40` + 40×{u16 animListIdx, u8 attr, u8 pad}: NPC definitions (entries 0,1 = heroes, skipped) | `0x45a42c` |
| 38 | `u32 40` + 40×u32 Partial.dat index: NPC idle FLCs, loaded into list game+0xc | `0x45a42c` |
| 39 | `u32 40` + 40×{u16 x, u16 y}: NPC positions | `0x45a42c` |
| 40 | palette (colours 56..255) for the #41 screen | `0x45f17d` |
| 41 | RLE 640×420, special screen (map/ending) | `0x45f10f` |
| 42 | FLC 0x00FF delta over #41 | `0x45f139` |
| 43 | RLE list 32× 26×26 icons for #41 | `0x45f16e` |
| 48 | 87374 bytes: initial world/game state (new-game "savegame", same stream format as SaveGame) | `0x4631ae` -> world.vmt[0], then `0x45d7ec` init |
| 64 | RLE 300×350 menu panel | |
| 65–102 | RLE menu/button graphics, mostly normal/highlight pairs (65/66 = "Pokračovat") | `0x465760`, `0x465aa6`/`ac9`, `0x466786`…, index passed in by caller |
| 0, 22–31, 44–47, 49–63 | empty | |

**String list format** (`0x44e2c8`, item reader `0x44e428`):
`u32 count; count × { u16 len; u8 c[len] }`. Decrypt with `k=0x55; for each byte: p = c^k; k = c` (XOR with the previous *cipher* byte). The text is CP1250. Reader `0x44e254` is the plaintext variant and is not used by the game.

## 7. Graph.dat / Install.dat
Neither file is referenced anywhere in Ostrov.exe (no strings, no loads). They are left over from the original **EPA Prot** CD copy protection. This Win10 repack's exe no longer calls them.
- **Graph.dat**: PE32 GUI exe, MSVC CRT, built from the "T32xWin" wizard skeleton, timestamp 2000-01-17. It has a 125195-byte encrypted overlay at file offset 0x10000.
  - Strings: `EpaProtLib`, `EpaProt.DLL`, `Prot16.DLL`, `ProtLib16`, `CorrectCDRom`.
  - Imports: GetLogicalDrives, GetDriveTypeA, GetTempPathA, CreateFileA, WriteFile, DeleteFileA, LoadLibraryA, GetProcAddress, RegCreateKeyExA, RegSetValueExA, AdjustTokenPrivileges, ExitWindowsEx.
  - Reading: it unpacks the protection DLLs from the overlay into the temp dir, loads them, checks the CD and writes the result to the registry.
- **Install.dat**: PE32 GUI exe ("SharpTiny" skeleton, same date, no overlay).
  - Strings: `EpaProtLib`, `C:\EPAPROT\TRCHANGER.HLP`, `windir=C:\WINDOWS`.
  - Imports: Reg{Open,Enum,Query,Set,Create,Delete}Key, CreateProcessA, ExitWindowsEx.
  - Reading: the protection install/uninstall helper (registry setup, spawn process, reboot).
- These are not resources or graphics. The name "Graph.dat" is a disguise.

## 8. Key addresses (for game-logic RE)
| addr | what |
|---|---|
| 0x4599bc | open all archives |
| 0x44e5c8 / 0x44e6a8 / 0x44e4e4 / 0x44e67c | archive create / seek entry / load object from entry / read N bytes from entry |
| 0x44f1d0 / 0x44f1a4 / 0x44f090 | RLE load / unpack / pack |
| 0x44f358 / 0x44f414 | RLE list load / save |
| 0x450188 / 0x450134 | GIF load / unpack |
| 0x459990 | image class factory (0 RLE, 1 GIF) |
| 0x44f4bc / 0x44f4e4 / 0x44f744 / 0x44fb54 / 0x44fc10 / 0x44fc40 | FLC open / open-impl / decode frame / seek frame / rewind / step frame (screen + bg) |
| 0x44ed18 / 0x44ed98 / 0x44edb0 | LUT rebuild / full palette load / scene palette load (56..255) |
| 0x44fec4 (row copy 0x44fe9c) | opaque image blit |
| 0x44ffa0 | transparent image blit |
| 0x44fe5c -> 0x44fcac | masked/depth sprite blit (+flip) |
| 0x44eea4 | rect copy (restore from background) |
| 0x4516e8 | font glyph blit |
| 0x450624 / 0x4505ac | gfx buffer alloc / free |
| 0x45b5b0 | draw scene background + objects |
| 0x45ba34 / 0x45ba70 | load scene ambient FLC A / B |
| 0x45c1fc / 0x45c274 | load priority mask / scene palette |
| 0x45baac | redraw/erase a scene object |
| 0x45ac50 | play Partial anim (script) |
| 0x45b2f0 | play Anim.dat cut-scene |
| 0x45c8c8 | **script interpreter** (opcode switch; ops 3,4,8,0xa,0xb,0x14,0x15,0x1a,0x1d,0x22,0x26,0x27 touch graphics) |
| 0x45a42c | world/NPC setup (Data 0x25–0x27, Partial NPC anims) |
| 0x459bb4 | load dialogue tables (Data 0x22–0x24) |
| 0x45d908 / 0x45da10 | say dialogue line / advance sequence |
| 0x45d7ec | new-game init |
| 0x4648b0.. | menu/inventory UI setup (Data 16–20, 32, 33) |
| 0x45f0f8.. | special screen (Data 40–43) |

## 9. Sound (Music, Speech, FX, FX2)
Header (parsed at 0x45552c): `u16 sample_rate` (22050), `i32 data_size`, `i32 pcm_size`, `u8 flags` (bit0 stereo, bit1 16-bit; always 0x02), `u8 compression` = `'c'`.
Samples = `pcm_size/2 - 1`. Then blocks: `u16 nbytes (≤0x200), u16 unused, u32 0xDEAF, nbytes data`. State (pred=0, idx=0) persists across blocks.
Nibbles low then high. Per nibble (0x4557dc): `step=STEP[idx]` (IMA table, 0x468d6c), `diff = step*(n&7)/4 + step/8`,
sign from bit 3, clamp s16, `idx += {-1,-1,-1,-1,2,4,6,8}[n&7]` (0x468d4c) clamped 0..88.

## 10. Other files
`Logo.avi`, `Titulky.avi`: Cinepak (`cvid`) 640×480 @ 15 fps, no audio stream.

## Not referenced by Ostrov.exe
`Anim2.dat` — never opened by the exe. Raw 4 KB-sector disk dump containing byte-identical copies of
Anim/Backgnd/Data/Fx/Fx2/Music/Partial/Speech.dat, both AVIs and Install.dat (Anim.dat at offset 1,228,800),
plus a different Ostrov.exe build at 375,353,344 (first 4 KB identical). Nothing unique; ignored.


`Anim2.dat` — never opened by the exe. Raw 4 KB-sector disk dump containing byte-identical copies of
Anim/Backgnd/Data/Fx/Fx2/Music/Partial/Speech.dat, both AVIs and Install.dat (Anim.dat at offset 1,228,800),
plus a different Ostrov.exe build at 375,353,344 (first 4 KB identical). Nothing unique; ignored.
