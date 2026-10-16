# Ztracený Ostrov (2000) — engine spec for a 1:1 Rust port

Binary: `Ostrov.exe` (analysed copy `OstrovLogic.exe`, sha256 fc1241fe…6af6), Delphi 4/5 + DelphiX, image base 0x400000.
Sections: CODE 0x401000–0x467cac, DATA 0x468000, BSS 0x46a000 (size 0x18f5d), .rsrc 0x48f000.
All addresses below are VAs in this exe. Ghidra truncates many Delphi try/finally bodies; where noted "asm", read the
disassembly (tools: `scratchpad/logic/dis.py <start> <end>` – capstone with call annotation, `xr.py <addr>…` – call/ref scanner,
`scan.py <operand-substring>`).

Companion files (all in `scratchpad/logic/`):
- `rooms.json` – full parse of the world (Data.dat entry 0x30): 124 rooms, 482 regions, 14 586 script words.
- `scripts.txt` – human-readable disassembly of every region script, with Say text inlined.
- `strings_0x22.txt` (1633 dialogue lines), `strings_0x20.txt` (97 item names), `strings_0x21.txt` (1605 hotspot labels).
- Tools: `scratchpad/tools/arc.py` (archive + XOR string), `rooms.py` (world parser + script disassembler), `dump.py`.

## 0. Headline answer: the game is DATA-DRIVEN

Scene logic is **not** hardcoded per room. Every room/hotspot and its behaviour is a compiled bytecode script stored in
`Data.dat` entry **0x30**, executed by one interpreter (`TWorld.ExecRegion` 0x45c8c8). The exe only hardcodes:
start state, inventory combinations, a handful of room-id special cases, music playlists, ambient-sound tables,
the cannon minigame (SpecialFX 1), CD check. A Rust port = archive readers + interpreter + character/walk system + UI.
Nothing has to be transcribed per scene; the port loads entry 0x30 at runtime.

## 1. Delphi classes (RTTI names, VMT = address of first virtual slot)

| class | VMT | Load(stream) | Save(stream) | notes |
|---|---|---|---|---|
| TStored (base, unit at 0x44e1xx) | 0x44e144 | slot0 | slot1 | slot2 0x44e500 = LoadFromFile(name) |
| TRLEImage | 0x44e7c8 | 0x44f1d0 | 0x44f23c | graphics agent |
| TGIFImage | 0x44e838 | 0x450188 | 0x4502c0 | `0x459990(type)`: type 1→GIF, 0→RLE |
| TRLEImageList | 0x44e8a4 | 0x44f358 | 0x44f414 | sprite sheets (characters, cursors, panel, icons) |
| TFLC-like anim ("Partial", 0x44e910) | 0x44e910 | load = `0x44f4bc(obj, archive, index, mode)` | – | Partial.dat / Anim.dat players; FLC strings at 0x44f704 |
| TRegion | 0x450730 | 0x450cf4 | 0x450f40 | hotspot / walk polygon |
| TRoom | 0x450794 | 0x4510f4 | 0x451414 | |
| TToolTip | 0x451607 | – | – | hover label + speech bubble text renderer (font 10×17) |
| TCharacter | 0x456534 (no virtuals) | load 0x457188 | save 0x457638 | |
| TWorld | 0x459630 | 0x45a760 | 0x45a818 | ctor 0x45a42c |
| TMainForm | DFM in .rsrc @file 0x78874 | | | handlers below |
| UI: TControl 0x464e55, TGroup, TDialog, TButton, TCheckBox, TTrackBar, TOptions, TConfirmDialog, TStamp, TButton2, TSaveLoadDialog (unit 0x4652cc–0x467900) | | | | custom in-surface GUI, not VCL |
| TRegistry, TMediaPlayer (AVI), DelphiX TDXDraw/TDXSound | | | | library |

Common helpers: 0x40fcf0 Stream.ReadBuffer(stream,buf,n); 0x40fd28 WriteBuffer; 0x40fec0 TFileStream.Create(name,mode);
0x402ae0 Random(n); 0x40db7c TList.Get; 0x40da20 TList.Add; 0x402d9c Create; 0x402dcc Free; 0x402700 ReallocMem;
0x44d3b0 DXDraw.CanDraw; 0x44edf4 surface Lock; 0x44ee44 Unlock+Flip; 0x455cf8 PlaySample(archive, idx, ?).

## 2. Archives (`*.dat`)

`TDataFile` ctor 0x44e5c8: `i32 count; count × {i32 offset; i32 size}`; offset −1 = missing entry. Seek 0x44e6a8: an
out-of-range or missing index silently becomes index **1**. Object load = `0x44e4e4(obj, archive, idx)` → seek, call obj VMT slot0.
Global archive objects (created 0x4599bc): Anim 0x46910c, Backgnd 0x469110, Partial 0x469114, Data 0x469118, Music 0x46911c,
FX 0x469120, FX2 0x469124, Speech 0x469128. File name strings: 0x459650… (see task list).
Each archive is opened from the game dir (0x470308) if it exists, else from CD path (0x47030c = Main.cfg[0] + ":\").
`Data.dat` must exist in the game dir (0x4599bc raises otherwise, 0x459b54 "Chybí CD.").

Strings: `u16 len; bytes`, two flavours: plain (0x44e254 read / 0x44e1e4 write) and XOR-chained (0x44e428 read / 0x44e364 write):
`key=0x55; for each byte c: out=c^key; key=c` (key = previous *ciphertext* byte). Text encoding cp1250.
String list (0x44e2c8): `i32 n; n × xorstring`.

### Data.dat entries used by the engine (others: graphics agent)
| idx | content | loader |
|---|---|---|
| 0x03 | hero sprite list (TRLEImageList) | world ctor 0x45a42c |
| 0x0f | 2nd playable char sprite list | world ctor |
| 0x10 | bottom-panel button images (list) → form+0x318 | 0x4648b4 |
| 0x11 | inventory icon list → form+0x31c | 0x4648b4 |
| 0x12, 0x13 | cursor list (→ cursor object 0x46972c+0x10) / misc image | 0x4648b4 |
| 0x15 | font: 0x5ef6 bytes = glyphs 10×17 px, 0xaa bytes each, 1=ink 2=outline | 0x4516d4 |
| 0x20 | item names (97) – string list (form+0x320) | 0x4648b4 |
| 0x21 | hotspot labels (1605) – string list (form+0x324) | 0x4648b4 |
| 0x22 | dialogue text (1633) – string list (0x469134) | 0x459bb4 |
| 0x23 | line table: i32 n(=2048); n × {i16 speaker, i16 textIdx, i16 speechIdx} (0x469138) | 0x459bb4 |
| 0x24 | dialog table: i32 n(=1544); n × {i16 firstLine, i16 lastLine} (0x46913c) | 0x459bb4 |
| 0x25 | NPC table 40 × {u16 animListIdx, u16 talkColor} | world ctor |
| 0x26 | NPC anim list 40 × {u16 Partial.dat idx, u16 0} (2000 = none) | world ctor |
| 0x27 | NPC positions 40 × {u16 x, u16 y} | world ctor |
| 0x30 | **TWorld** (rooms + scripts) | main loop 0x4630a4 (reloaded on every new game) |
| 0x40 | "Paused"/misc? (0x30–0x3f options/menu images: 0x40=game menu, 0x41..0x59 buttons) | UI ctors |
| 0x5a,0x5b | confirm dialogs; 0x60,0x61 save/load dialogs; 0x5c–0x66 their buttons | 0x4648b4 |

## 3. World / room / region model

### TWorld fields (object at form+0x30c; ctor 0x45a42c)
| off | type | meaning |
|---|---|---|
| +0x04 | TList<TRoom> | rooms |
| +0x08 | TList | skin image (index 0 = inventory panel skin loaded from Data.dat[+0x14]) |
| +0x0c | TList<anim> | NPC anims (from 0x26) |
| +0x10 | TList<TCharacter> | [0]=hero, [1]=2nd playable char (0x45fb30), [2..39]=NPCs |
| +0x14 | i16 | current skin (Data.dat index; default 3) — ChangeSkin |
| +0x16 | i16 | (0xffff init) |
| +0x18 | u8[256] | **2048 game flags** (bit test 0x45b9b0, set/clear 0x45b9e4) |
| +0x118 | TRoom* | current room |
| +0x11c | i32 | current room id |
| +0x120/+0x124 | anim* | background animation players 1/2 (room.anim1/anim2) |
| +0x128 | i32 | pending region index (−1 none; 0x800/0x801 = click on hero/2nd char, uses room 0x7ff regions 0/1) |
| +0x12c | i32 | script PC (word index) |
| +0x130 | u8 | "script contains blocking anim" (set by prescan for ops 3,4,0x22,0x27,0x29) |
| +0x131 | u8 | room changed during script |
| +0x132 | u8 | GameOver flag → main loop ends |
| +0x134/+0x138 | i32 | active Dialog id / current line |

### TRoom (load 0x4510f4) – on-disk order
```
i32 id; xorstr name; i32 nRegions; TRegion regions[n]; TRegion walkArea;
i16 bg(+0x4c, Backgnd idx); u8 bgType(+0x4e, 1=GIF 0=RLE); i16 anim1(+0x50); i16 anim2(+0x52);
u8[16] (+0x1c, always 0); u8[32] (+0x2c, always 0); i32 nWaypoints; {i32 x,i32 y}[n] (+0x14, pathfinding nodes);
i16 pal(+0x54, palette Backgnd idx); i16 scaleA(+0x56); i16 scaleB(+0x58) (char scale = max(1,(B−A)/(y−A))); i32 foreground(+0x5c, Backgnd idx);
i32 fgY1(+0x60); i32 fgY2(+0x64)
```
Negative anim index = random-trigger anim (0x45e140: plays only when Random(100000)<500 per frame, else idle).
Room 0x7ff is a pseudo-room holding the hotspots for clicking characters (region 0 = hero, 1 = 2nd char).
Saved per room (0x451368): bg, bgType, anim1, anim2, fg, fgY1, fgY2, pal, then per region the save record.

### TRegion (load 0x450cf4, save-state 0x450ecc / restore 0x450de4)
```
i32 nPoints; {i32 x, i32 y}[n] (+8); u8 enabled(+0x14); u8 cursor(+0x15); i16 w20(+0x20, loaded but never read by the exe – editor metadata);
i32 scriptBytes(+0x1c); i16 script[scriptBytes/2](+0x18); i16 image(+0x22, Backgnd idx, <0 = erase); u8 imageType(+0x24);
i16 x(+0x26); i16 y(+0x28); i16 label(+0x2a → strings 0x21)
```
Hit test 0x450ba4: bbox (0x450c08) then Win32 `CreatePolygonRgn(ALTERNATE=… mode 2 = WINDING)` + `PtInRegion`.
Segment test 0x450c48: 10 samples along the segment all inside.
Regions are hit-tested **last to first** (topmost = highest index). Cursor code: 0=no action (just walk), 2=walk (set by
caller when point is in walk area), 4 = use/exit, 5 = default, 7 = walk-only for item use; others = cursor image index.

## 4. Script bytecode

Script = array of i16 words. `opcode = word & 0x7f`, followed by `argc` words (table at 0x468b50: 42 × {name, argc}).
Opcode-name strings and argc (address of `PTR_s_Nothing_00468b50`):

| # | name | argc | handler / semantics (args a1..an) |
|---|---|---|---|
| 0 | Nothing | 0 | – |
| 1 | ItemLabel | 1 | block terminator / entry: "use item a1 here" |
| 2 | Goto | 4 | prescan: char a4 walks to (a1,a2), final dir a3 (0x457040) |
| 3 | PlayPartial | 3 | 0x45ac50(Partial a1, a2, mode 0, fx a3) |
| 4 | PlayFull | 3 | 0x45b2f0(Anim a1, frame delay a2 ms (0→114), stopMusic=0, mode 1, fx a3) |
| 5 | ChangeRoom | 2 | a2==0: hero → room a1 (0x45a9b8), set +0x131; else char a2 placed in room a1 |
| 6 | ChangePos | 3 | char a3 .x=a1 (0x457bac) .y=a2 (0x457bcc) |
| 7 | ChangeDir | 2 | char a2 dir=a1 (0x457b58) |
| 8 | ChangeGround | 3 | room a3: bg=a1, bgType=(a2!=0); redraw if current (asm 0x45cdf0) |
| 9 | PickUp | 2 | char a2 inventory add item a1 (0x457348) |
| 10 | EnableReg | 2 | room a2 region a1 enabled=1; draws its image if current |
| 11 | DisableReg | 2 | room a2 region a1 enabled=0; erases its image |
| 12 | DisableSelf | 0 | current region (+0x128) disabled + erased |
| 13 | Say | 1 | 0x45da88(line a1) |
| 14 | FlagLabel | 4 | block header: conditions a1..a4 (>0 flag set, <0 flag clear, 0 ignore) |
| 15 | SetFlag | 1 | flag a1 = 1 |
| 16 | ClearFlag | 1 | flag a1 = 0 |
| 17 | Turn | 2 | char a2 turn to dir a1 (0x457b7c, animated) |
| 18 | Drop | 2 | char a2 inventory remove item a1 (0x456b58) |
| 19 | ChangeIcon | 3 | room a3 region a2 cursor=a1 |
| 20 | ChangeSkin | 1 | world+0x14=a1, reload skin from Data.dat[a1], hero+0x10=a1 (hero sprite set/clothes) |
| 21 | ChangeBackAnim | 2 | room a2 anim1=a1 (stops/starts if current) |
| 22 | SetRegionImage | 5 | room a5 region a4: image=a1, x=a2, y=a3 (redraw diff if current & enabled) |
| 23 | SetText | 3 | room a3 region a2 label=a1 |
| 24 | Enter | 3 | no-op in interpreter (jump table default) – data only |
| 25 | Walk | 0 | prescan: hero walks to the clicked point (0x45c70c) |
| 26 | ChangeForeground | 2 | room a2 foreground=a1 |
| 27 | GameOver | 0 | world+0x132=1 |
| 28 | Dialog | 1 | 0x45dd90(dialog a1) – asm |
| 29 | ChangePal | 2 | room a2 pal=a1; reload palette if current (0x45c274 → 0x44edb0) |
| 30 | ChangePosX | 4 | char a4: x=a1,y=a2,dir=a3 |
| 31 | CleanUpInv | 0 | hero inventory count=0 |
| 32 | StaticWalk | 4 | blocking straight walk of char a4 to (a1,a2), final dir a3 (0x45e2cc); ESC/click snaps |
| 33 | StaticTurn | 2 | blocking turn of char a2 to dir a1 (0x45e510) |
| 34 | PlayPartialX | 4 | 0x45ac50(Partial a1, a2, mode a3, fx a4) |
| 35 | ChangeStatic | 2 | default speaker 0x469144 = a1 |
| 36 | SpecialFX | 1 | 0x45fe30: 1 = cannon minigame (0x45ef1c; battleship: 21×8 grid of 26 px at (46,42), one random 2×6 ship, 25 shots, Data.dat 0x28–0x2b) → run region 11 (fail) / 10 (success) of current room; 2..4 → music set a1−1 |
| 37 | Delay | 1 | 0x45f81c: wait a1 ticks (70 ms), skippable |
| 38 | ChangeBackAnimX | 3 | room a2; a3∈{2,4} → anim2=a1 else anim1=a1; a3>2 = wait for anim end |
| 39 | PlayFullX | 4 | PlayFull(a1, delay a2, stopMusic=(a4>=3), mode a4%3, fx a3) |
| 40 | TurnTowards | 2 | char a2 turns to face char a1 (0x45fb44 → StaticTurn, dir ∈ {0,3,4,5}) |
| 41 | Meanwhile | 2 | caption strings0x22[a1] centred in view for max(2·len,30) ticks (0x45fbe0); a2 unused |

Bit 7 of the opcode word is never set in shipped data (masked anyway).

### Block structure & dispatch (0x45c294 `TWorld.StartRegion(region, clickX, item, clickY)`)
1. PC=0. Scan FlagLabel headers in order; the **first FlagLabel whose 4 conditions all hold** wins → PC = after it.
   If none matches, PC stays 0 (the default block = words before the first FlagLabel).
2. If an item is being used (item ≠ −1): from PC, scan until next FlagLabel for `ItemLabel item`; found → PC after it;
   not found → `Say(0x200+Random(6))` (generic "that doesn't work" lines 512..517) and abort.
   Without an item, execution runs from PC and stops at the first ItemLabel or FlagLabel.
3. Prescan from PC to the next ItemLabel/FlagLabel: every `Goto` starts that character walking (char.+0xf7 "busy"=1);
   `Walk` starts the hero walking to the click; ops 3/4/0x22/0x27/0x29 set world+0x130.
4. Each frame `0x45a8a8` waits until no character with +0xf7 set is still moving (+0xcc), then calls the interpreter
   `0x45c8c8(world, region, reportEmpty=1)` which executes ops until ItemLabel/FlagLabel/end. If zero ops executed and
   reportEmpty → `Say(0x206+Random(4))` (lines 518..521). Afterwards pending region reset (0x45abfc).
5. After a script that changed room (+0x131) or in room 0x40a when flag 0x40a is clear, the room's walk-area script
   (room.walk.script = "Enter" script) is run via 0x45d7ac.

### Click handling (0x45a204, called from main loop on left click with y<420)
- Find topmost enabled region containing the point; else test 2nd char (if in same room & idle) → 0x801, hero → 0x800.
- No region / cursor 0 / (item and cursor 7): just walk hero there (0x45c70c, only if point is in walk polygon,
  snaps y downward ≤ 0x1a3).
- Hardcoded gate: if flag **0x200** (hero dressed) is clear, only cursor-4 regions and the special cases
  (room 0x200 region 1, room 0x204 regions 1/3, room 0x205 region 0) are allowed; otherwise `Say(0x20b)`
  ("No snad se tu nebudu špacírovat nahej!").
- Else world+0x128=region, StartRegion.
- Hover (0x45b7a0 / 0x462dd8): returns cursor (region cursor, 2 over walk area, 5 default) and label → tooltip at mouse+10.

## 5. Main loop & timing (TMainForm game loop 0x4630a4, called from `entry` 0x467bcc)

`entry`: Application.Initialize, CreateForm(TMainForm), `0x4648b4` (init: SetPriorityClass(GetCurrentThread(),0x80 HIGH),
volumes from Main.cfg, create world, load UI), Application.Run-ish, then `0x4630a4`.
- Video: `0x464c0c(form, aviName, fxIdx)` plays Logo.avi (start, fx 0x1fa) and Titulky.avi (credits after intro / at game over,
  fx 0x1f9) through TMediaPlayer onto the DXDraw window; ESC or click skips.
- Outer loop per game session: stop sounds, reload world from Data.dat[0x30], `0x45d7ec` NewGame:
  skin 3, flags cleared, room 0x210 ("Jama – absolútny začiatok"), hero at (400,300), 2nd char in room 0x646 at (200,350),
  default speaker 0. If no `SaveGame\auto.sav`: PlayFull(Anim 0x200, default delay, stopMusic, mode0, fx 0x300) intro, then
  Titulky.avi. Music set 0 (0x45f7c0). Run current room Enter script. If auto.sav exists → open main menu (0x464538) first.
- **Tick = 70 ms** (0x46): inner `do { poll input; handle } while (timeGetTime()-tickBase < 70); tickBase += 70`.
  Rendering happens once per tick: character update/draw 0x45c010 (draw order by y: chars sorted by y,
  back anims 0x45e140, ambient FX), cursor, tooltip. Script progress `0x45a8a8` once per tick.
- Blocking actions (PlayPartial, PlayFull, Say, Dialog, StaticWalk…) run their own nested loops with the same
  timeGetTime pacing (PlayFull: frame delay param, default 114 ms).
- Exit: GameOver (+0x132) → Titulky.avi + menu; otherwise autosave on quit: `0x45be70(world, "SaveGame\auto.sav", "AUTOSAVE", thumb)`.
- Music: `0x459714` each tick: if music enabled and track finished, next track of playlist (cyclic).
- FPS counter computed but unused (local_2c).

### Input (form event handlers, method table @0x462c1d)
| handler | addr | effect |
|---|---|---|
| DXDrawMouseDown | 0x462da4 | mouseX→[0x4696a4], mouseY→[0x46965c]; left→[0x469734]=1, right→[0x4694f4]=1 |
| DXDrawMouseUp | 0x463b2c | left-up [0x469458], right-up [0x4696dc] |
| DXDrawMouseMove | 0x4644c8 | coords + moved flag [0x469624] |
| FormKeyDown | 0x463b4c | [0x469648]=1, key→[0x4694cc]; F4(0x73)+Alt cancelled |
| FormKeyPress | 0x464c00 | char → [0x4698a4] |
Flags cleared each poll by 0x464880.
Keys: ESC(0x1b) = skip speech/dialog/anim only (menu opens only from panel button, or at start if auto.sav exists); 'I'(0x49) = inventory; Enter in dialogs.
Right click: holding item → drop it from cursor; else open inventory (0x463d94). In inventory: left = select/deselect/combine, right = close and put selected item on cursor, ESC = close empty-handed.

### Screen layout (640×480×16bpp, DXDraw fullscreen, BitCount 16, doWaitVBlank, doFlip)
- Game view y 0..419 (0x1a4); bottom panel y 420..479 (60 px), panel buttons at x 0 and 320 (table 0x4693cc).
- Panel hit zones: x 80..243 / y 427..474 → inventory button (id 10); x 370..499 → menu (id 11).
- Inventory screen (0x463d94): grid origin (107,100), cell 46×46, 9 columns, index=row*9+col (0x463c18).
- Back buffer pointer [0x469758], pitch [0x469828]; bg copy [0x4694c0] (stride 0x500 = 640×2).

## 6. Inventory & items
Per character: `u8 items[128]` at +0x48, count at +0xc8 (0x457324 has, 0x457348 add, 0x456b58 remove, CleanUpInv).
Item ids index `strings_0x20` and icon list Data.dat[0x11]. Using an item = pick from inventory (cursor becomes the icon,
local_10 in main loop), click a region → StartRegion with item. Combining in inventory is **hardcoded** (0x463d94):
| A + B | result | condition |
|---|---|---|
| 0x18 + 0x0b | 0x0c (Dělová koule s mouchou) | – |
| 0x0f + 0x0d | 0x0e (Přeseknuté svíčky) (only 0x0d consumed) | – |
| 0x50 + 0x51 | 0x52 | – |
| 0x5d + 0x5c | 0x5f | – |
| 0x3d + 0x36 | 0x3e | – |
| 0x42 + 0x47 | 0x43 | flag 0x44d set, else Say 0x412 |
otherwise Say(0x200+Random(6)). (Both orders accepted.)

## 7. Dialogue / subtitles / speech
`Say(line)` → 0x45d908: record = lineTable[line] (`speaker, textIdx, speechIdx`, each 0 → default: speaker=0x469144,
textIdx=line, speechIdx=line). If speech enabled (`[0x4695dc]`) and subtitles off (Main.cfg[15]==0) → only voice
(Speech.dat[speechIdx]); else text strings0x22[textIdx] shown above character `speaker` via TCharacter.Say 0x4577d0
(color = character talk colour from Data.dat 0x25, palette 0x46c898) with voice. Line 0x1f9 (505 "Glo glo.") replaces text+voice when speaker skin==0x0c (under water). Voice-only also requires speech volume>0. Duration without voice = max(len,15) ticks; with voice = until sample ends. Music/FX ducked ×0.3 during Say. Full details: ENGINE_A_dialog.md.
`Dialog(n)` plays lines dialogTable[n].first..last sequentially (0x45da10 advance, 0x45da6c stop; click skips line, ESC aborts dialog).
Text renderer: TToolTip (0x451834 layout: `"`→0xff, `'`→0xfe, word-wrap via 0x40a0a8, '\r' = new line; glyph 10 px wide
(advance 9), 17 px high; char map 0x451610/0x4516b8: ASCII 0x20..0x7f → glyph c−0x20; cp1250 accents via table 0x451688).

## 8. Music / ambient sound
- Playlists hardcoded: two variants selected by Main.cfg[16] (0x470320): 0 → table 0x469258, 1 → 0x469148.
  4 sets × {i32 count; i32 track[16]} (stride 0x44). Current set 0x469140 (saved), track pos 0x469130.
  Set 0 at game start; `SpecialFX 2/3/4` → set 1/2/3.
  - 0x469258: s0 [7,8,21,24,3,4,6,25,13,23,9,5,10] s1 [19,10,24,9,8,20,6,16,3,11,13] s2 [12,2,11,24,7,14,4,18,1,15] s3 [13,14,10,23,12,1,25,9,16,2,22,20,19,17]
  - 0x469148: s0 [6,3,4,12,13,24,7,8,10] s1 [4,9,10,24,13,8,6,15,3,12] s2 [10,12,13,24,7,2,3,5,1] s3 [9,13,15,2,4,1,5,6,8,7]
  Track = Music.dat index (0x4597a8 → 0x455b80). Music is NOT chosen per room.
- Ambient table 0x468f38: 48 × {roomId, FX2.dat idx}; each tick, if no ambient playing, Random(100000)<600 → play.
  (room,fx): 0x200:781 0x201:782 0x202:775 0x204:776 0x206:778 0x20a:768 0x20b:769 0x20c:783 0x20e:778 0x20f:768 0x602:772
  0x603:774 0x604:774 0x601:772 0x600:772 0x605:772 0x606:772 0x636:773 0x63e:772 0x63f:773 0x640:772 0x641:772 0x642:773 0x643:773
  0x644:773 0x645:773 0x646:773 0x647:772 0x648:771 0x649:771 0x64a:775 0x64b:778 0x64c:779 0x64e:771 0x3:779 0x2:778 0x405:781
  0x40e:779 0x403:771 0x404:776 0x406:782 0x407:782 0x408:779 0x40a:770 0x40b:783 0x40c:778 0x40d:771 0x40f:782
- Back-anim sound table 0x468f20 (3 × {anim, fx}): anim 666 → FX 58 at anim frame 1.
- Generic "can't" lines: 0x200..0x205 (Random(6)), "nothing happens" 0x206..0x209 (Random(4)).

## 9. D3DRM
Not used. DelphiX links IDirect3DRM interfaces (strings 0x444a4a…, Direct3DRMCreate 0x44be80) but the DFM options are
`doFullScreen, doAllowReboot, doWaitVBlank, doAllowPalette256, doCenter, doSelectDriver` – no doRetainedMode/do3D.
Rendering is pure 16-bit software blitting into the locked DirectDraw back buffer (RGB565/555 masks at 0x46ceac..).

## 10. Main.cfg / Backup.cfg (17 bytes, read 0x459c9c into 0x470310; write 0x45a14c)
Backup.cfg is the installer default; Main.cfg the live one (rewritten on exit).
| byte | var | meaning |
|---|---|---|
| 0 | 0x470310 | data drive letter (`.` = game dir, else CD letter e.g. 'V'); also CD-check (0x45958c) |
| 1–11 | – | unused by the exe (installer; observed 01×7, 00×4) |
| 12 | 0x47031c | music volume 0..100 (options trackbar +0xe0) |
| 13 | 0x47031d | FX volume 0..100 (+0xdc) |
| 14 | 0x47031e | speech volume 0..100 (+0xe4); while >0 music/FX are ducked during speech (×const @0x45982c) |
| 15 | 0x47031f | subtitles on (checkbox +0xd0) |
| 16 | 0x470320 | music variant (radio +0xd8 / +0xd4) → playlist table |

## 11. Save games (`SaveGame\Game%d.sav`, 8 slots; `SaveGame\auto.sav`)
Writer 0x45be70, reader 0x45bc1c. Little-endian, raw Delphi field dumps:
```
u16 nameLen; char name[nameLen]            (plain, "AUTOSAVE" for auto.sav; user text for slots)
u16 thumb[0x10a0]                          (76×56 RGB565 screenshot, 0x2140 bytes; built by 0x46763c, downscale of 640×420)
for each character (40):                    (0x457638)
  i32 roomId(+0xfc); i32 x(+0x14); i32 y(+0x18); i32 segStart x,y / segEnd x,y (+0x1c..+0x28); queued leg x,y (+0x2c,+0x30);
  i32 +0x34 (dest x); i32 +0x38 (dest y); u8 dir(+0x11); u8 +0x12; u8 finalDir(+0x13); i32 +0x3c (anim frame);
  f64 scale(+0x40); u8 walking(+0xcc); u8 +0xcd; u8 busy(+0xf7); u8 skin(+0x10); u8 +0xce; u8 visited[20](+0xcf);
  u8 isAnimNPC(+4); u8 items[128](+0x48); i32 itemCount(+0xc8)
for each room (124, file order):            (0x451368)
  i16 bg; u8 bgType; i16 anim1; i16 anim2; i32 fg; i32 fgY1; i32 fgY2; i16 pal;
  for each region: u8 enabled; u8 cursor; i16 x; i16 y; i16 image; u8 imageType; i16 label
u8 flags[256]; i32 currentRoom; i16 skin; i16 world+0x16; i32 pendingRegion; i32 pc; u8 busy(+0x130); u8 gameOver(+0x132);
i32 musicSet(0x469140); i32 defaultSpeaker(0x469144)
```
Loading calls ChangeRoom(saved id) + reloads skin and restarts music set (0x45f7c0).
Save/Load dialog (TSaveLoadDialog 0x466d8c): 8 slot thumbnails at (31,106) (121,106) (211,106) (301,106) (31,178)…(301,178)
(table 0x4693d8, size 76×56), slot files via 0x46537c (`SAVEGAME\` + `Game%d.sav`).

## 12. Menus / UI
Custom widget toolkit drawing into the back buffer (unit 0x464e55–0x467900); widgets: button (0x465a6c: two images
normal/hover, id), checkbox (0x46674c), trackbar (0x4669d8, max 100), stamp (save slot, 0x4676a0).
Dialog loop 0x465820/0x466250 (modal: draws, polls input, returns pressed id in +0x28).
- Game menu (0x465f50, image Data.dat 0x40, centered): buttons at (10,9) id10, (10,34) id13, (10,64) id14, (10,93) id12,
  (10,118) id11, (92,257) id15; checkboxes (10,312) id19 (subtitles), (120,312) id21 / (180,312) id20 (music variant radio);
  trackbars (10,178) FX, (10,228) speech, (10,278) music. Buttons: id10 Continue, id13 Load, id14 Save, id12 New game (confirm), id11 Quit (no confirm), id15 next music track. Radios id21 → variant 0, id20 → variant 1 (restarts playlist). Menu result: 0 continue,1 load,2 save,3 new,4 quit.
- Confirm dialogs 0x466cb0 (images 0x5a/0x5b): yes (17,62) id1, no (117,62) id2; ESC=2, Enter=1.
- Save/Load dialogs 0x466d8c (images 0x60/0x61): OK (245,261) id1, Cancel (304,261) id2, 8 stamps, text edit (31,263) img 0x66.

## 13. Copy protection (skip in port)
0x459328: builds registry key from obfuscated string 0x458fcc (`Tqixwbth\`Mjeussphw\`LnHlvsuRdws` = "Software\Microsoft\Windows\CurrentVersion"
with per-char −(i mod 5)), computes a value, two timers (400 ms 0x458ff4, 120 s 0x458e8c) re-check the CD (drive Main.cfg[0]).

## 14. Detail files
- ENGINE_A_dialog.md – Say/Dialog/Delay/Meanwhile/text renderer.
- ENGINE_B_character.md – character fields, walking, pathfinding, sprites, scaling, ChangeRoom (flag 0x7ff = companion follows hero).
- ENGINE_C_ui.md – anims, minigame, menus, save/load UI, cursor, inventory.
