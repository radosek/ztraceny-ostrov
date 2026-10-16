# ENGINE addendum E — UI widgets, dialog loops, char hit-test, inventory backdrop, save format (from asm)

Source: Ostrov.exe sha256 fc1241fe…6af6 (copy OstrovUi.exe), capstone. CODE va→file `va-0x400c00`, DATA `va-0x400e00`.
Image indices below are **Data.dat slot numbers** (same as the constants in code). Mouse x = [0x4696a4], y = [0x46965c].
Input events (one-shot flags, cleared each loop iteration by 0x464880): LMB-down 0x469734, **LMB-up 0x469458**, RMB-down
0x4694f4, key-down 0x469648 (vk code u16 0x4694cc), char 0x4698a4 (KeyPress char), mouse-moved 0x469624.
Blitters (see ENGINE_D): O = opaque 0x44fec4, T = transparent clipped 0x450010, Tu = transparent unclipped 0x4500b8.
All UI images (Data 16–19, 64–102) only use palette indices 0..45 → fixed UI colours (entries 0..55 from Data #20, never
overwritten by room palettes). No palette switch happens for menus/inventory.

## E1. Widget base (TWidget, ctor 0x46541c)
Fields: +0xc x, +0x10 y (relative to parent), +0x14 w, +0x18 h (= size of the first image), +0x1c parent, +0x20 focusable,
+0x22 focused, +0x23 save-under (panels), +0x24 save buffer, +0x28 / +0x29 / +0x2a / +0x2b per-class flags (below), +0x2c id.
Absolute pos 0x465584 = sum of parent offsets. Hit test 0x4655d8: `ax<=mx<ax+w && ay<=my<ay+h`.
VMT: [0] Update (called per input event), [4] DrawNormal, [8] DrawHover / key handler (panels), [0xc] Execute (panels).
**Clicks fire on LMB-down, not on release.** Widgets redraw themselves only; nothing erases first (images are opaque except overlays).

## E2. Panel (TDialog, ctor 0x465724(self, imgIdx))
- Image Data[imgIdx]; position **x=(640-w)/2, y=(480-h)/2** (truncating), drawn Tu (transparent index = image's trans byte).
- Open 0x4654e4: save-under the panel rect from the back buffer, draw panel + all children (DrawNormal). Close 0x46553c:
  restore save-under. So the dialog sits over whatever is on screen: **game view as is, not darkened, not animated**.
- Update 0x465640: focused child first, then the others in insertion order (max 31 children).
- Result word `+0x28` (0 = none; child sets its id on click).

| panel | img | w×h | pos |
|---|---|---|---|
| Game menu TOptions | 0x40 (64) | 300×350 | (170,65) |
| Confirm "new game?" [0x482f48] | 0x5a (90) | 174×116 | (233,182) |
| Confirm "quit?" [0x482f4c] | 0x5b (91) | 174×116 | (233,182) |
| Load dialog [0x482f54] | 0x60 (96) | 407×349 | (116,65) |
| Save dialog [0x482f50] | 0x61 (97) | 407×349 | (116,65) |

## E3. Button (0x465a6c; VMT 0x464f70: Update 0x465bf4, Normal 0x465b5c, Hover 0x465ba8)
Ctor pushes (id, imgNormal, imgHover). +0x30 normal image, +0x34 hover image, size = normal image.
Flags: +0x28 **enabled**, +0x29 **visible**, +0x2a "show normal while pressed".
- Normal/Hover draw: only if visible; image O at abs pos. Invisible = nothing drawn (panel art shows).
- Update: if mouse inside && enabled && visible: LMB-down → parent.result = id; draw Hover — except buttons with +0x2a
  (menu Load/Save) draw Normal on the LMB-down frame. Otherwise draw Normal.
- **Disabled (+0x28=0) buttons are drawn with the normal image and never highlight/click.** No separate disabled art.

## E4. Checkbox / radio (0x46674c; VMT 0x464fd0: Update 0x46692c, Normal 0x46686c, Hover 0x4668d0)
Ctor pushes (id, imgNormal, imgHover, imgOverlay, isRadio). +0x30 normal, +0x34 hover, +0x38 overlay ("X" cross),
+0x28 enabled, **+0x29 value**, +0x2a isRadio.
- Normal: base O; if value==0 && enabled → overlay T at same pos. Hover: hover image O; if value==0 → overlay T.
  i.e. **X drawn = off (0); no X = on (1)**.
- Update: inside && enabled: on LMB-down: checkbox → toggle value, result=id; radio → only if value==0: value=1, result=id
  (clicking a selected radio does nothing). Draw Hover. Else draw Normal.
- Radio exclusivity done in menu handler (E7).

## E5. Trackbar (0x4669d8; VMT 0x465034: Update 0x466b98, Draw 0x466ad4; no hover image)
Ctor pushes (id, imgTrack, imgThumb, max=100). +0x2c track, +0x30 thumb, +0x34 dragging, +0x38 max, +0x3c value,
+0x40 thumb px offset, +0x44 drag start mouse x, +0x48 drag start offset. range R = trackW − thumbW.
- value→px (0x4669c0): `px = R*value/max` (int div). px→value (0x466994): `value = px*max/R`.
- Draw: track O at pos; if enabled thumb T at (x+px, y). **Disabled → track only, no thumb.**
- Update (needs enabled; runs if mouse inside OR dragging):
  - dragging: LMB-up event → stop. Else `px = mx + startPx - startMx` clamped [0,R]; value; result=id; redraw.
  - LMB-down (not dragging): mouse x in [x+px, x+px+thumbW) → start drag (save mx, px); else **jump**: `px = mx - x - thumbW/2`
    clamped [0,R], value, result=id, redraw.
  - Handler fires on every drag event → volume applied live.

## E6. Edit box (0x465c6c; VMT 0x465048: Update 0x465dec, Draw 0x465d20)
+0x34 background image, +0x28 enabled, +0x29 caret visible, +0x2c text (AnsiString), +0x30 caret pos (u8).
- Draw: bg O at (x,y). Enabled: text at (x+6, y+4) colour 2 (0x45176c, 9 px/char, font Data 0x15); if caret visible:
  "_" at (x+4+9*caret, y+4) colour 1. **Disabled: text centred**: `x + (w - 9*len)/2 - 1`, y+4, colour 2 (this is how the
  hovered slot name shows). Colours are palette indices (ink=lut[c], outline=lut[0]).
- Update (enabled && key-down): key-down consumed unless Enter/ESC (those reach the dialog).
  - char ≥ 0x20 (any CP1250 byte ≥ 0x20) and len < **16** → insert at caret, caret++.
  - else vk: Left 0x25 caret-- (≥0); Right 0x27 caret++ (≤len); Home 0x24 → 0; End 0x23 → len;
    Delete 0x2e → delete char after caret; Backspace 8 → delete char before caret, caret--. Then redraw.
- Caret blink: in save/load loop every **150 ms** toggle +0x29 and redraw (only while enabled).

## E7. Game menu (ctor 0x465f50, Execute 0x466250, key handler 0x4666fc, result handler 0x4664fc)
Positions relative to panel (170,65); absolute = +170/+65.
| field | id | widget | pos | images (normal, hover[, overlay]) | size |
|---|---|---|---|---|---|
| +0xb8 | 10 | button Continue | (10,9) | 0x41, 0x42 | 91×23 |
| +0xbc | 13 | button Load (+0x2a) | (10,34) | 0x43, 0x44 | 105×29 |
| +0xc0 | 14 | button Save (+0x2a) | (10,64) | 0x45, 0x46 | 95×29 |
| +0xc4 | 12 | button New game | (10,93) | 0x47, 0x48 | 75×24 |
| +0xc8 | 11 | button Quit | (10,118) | 0x49, 0x4a | 50×24 |
| +0xcc | 15 | button next track | (92,257) | 0x4b, 0x4c | 90×22 |
| +0xd0 | 19 | checkbox subtitles | (10,312) | 0x4d, 0x4e, 0x4f | 65×29 |
| +0xd4 | 21 | radio variant 0 | (120,312) | 0x50, 0x51, 0x52 | 55×24 |
| +0xd8 | 20 | radio variant 1 | (180,312) | 0x53, 0x54, 0x55 | 55×24 |
| +0xdc | 17 | trackbar FX | (10,178) | track 0x56, thumb 0x59 | 150×28 / 43×28, R=107 |
| +0xe4 | 18 | trackbar speech | (10,228) | track 0x57, thumb 0x59 | same |
| +0xe0 | 16 | trackbar music | (10,278) | track 0x58, thumb 0x59 | same |

Setup in 0x464538(form, inGame, fromAutosave):
- inGame → Continue+Save enabled; else fromAutosave → Continue enabled, Save disabled; else both disabled (+0x28=0, drawn normal).
- trackbars value = cfg[12] music, cfg[13] FX, cfg[14] speech; subtitles = cfg[15]; radio1 = cfg[16], radio0 = !cfg[16].
- No DirectSound (form+0x304 +0x44 == 0): 3 trackbars, subtitles checkbox, next-track button, both radios disabled;
  subtitles and both radios forced value 0.
- Before opening: hide cursor, cursor shape 0, hide hover tooltip, **thumbnail snapshot** (E10), flip. If !inGame: 0x44ef8c
  clears screen to black (menu over black at startup).
Handlers: 10 → result 0, close. 11 → result 4, close-request. 12 → confirm[0x482f48] (img 0x5a); yes → result 3, close.
13 → Load dialog (edit disabled); OK → result 1, close. 14 → Save dialog (OK button+edit disabled); OK → result 2, close.
15 → next track 0x455c0c. 16/17/18 → set volume (0x455cb8 music / 0x455e78 FX / 0x456018 speech) + store cfg. 19 → cfg[15]=value.
21 → if radio1 set: clear radio1 (redraw), cfg[16]=0, playlist pos=-1, next track. 20 → symmetric, cfg[16]=1.
Sub-dialog cancel → menu sets +0xe8 to force one full menu redraw next tick (sub-dialog's save-under restored first).
Keys: ESC → id 10 if inGame else id 11; RMB-down while inGame → id 10.
**Quit confirm (correction to C10):** in the Execute loop, if the panel result is 1/2 and menu result is still 4
(id 11 or ESC outside game) → confirm[0x482f4c] (img 0x5b); No → continue menu.
After close (0x464538): 2 → save; 1 → black screen + load chosen file; 0 && fromAutosave → black + load SaveGame\auto.sav;
if result ≠ 4: clear input, full-redraw flag, form+0x330=1, tick base = now−35 ms.

## E8. Confirm dialog (ctor 0x466cb0, keys 0x466d50, Execute = generic 0x465820)
| id | pos (rel) | abs | images | size |
|---|---|---|---|---|
| 1 Yes | (17,62) | (250,244) | 0x5c, 0x5d | 50×25 |
| 2 No | (117,62) | (350,244) | 0x5e, 0x5f | 37×25 |
ESC → 2, Enter → 1. Returns true on 1.

## E9. Save/Load dialog (ctor 0x466d8c, Execute 0x466eb4, keys 0x46740c, result 0x4672cc)
Panel (116,65). +0xe8 = save mode (Save dialog 1, Load dialog 0).
| widget | pos rel | abs | images | size |
|---|---|---|---|---|
| OK id1 (+0xb8) | (245,261) | (361,326) | 0x62, 0x63 | 40×27 |
| Cancel id2 (+0xbc) | (304,261) | (420,326) | 0x64, 0x65 | 62×27 |
| edit (+0xe4) | (31,263) | (147,328) | bg 0x66 | 169×25 |
| stamp i (+0xc4+4i), id 0x16 | table 0x4693d8 | +116/+65 | — | 76×56 |
Stamp rel pos: (31,106) (121,106) (211,106) (301,106) / (31,178) (121,178) (211,178) (301,178).
- **Load dialog: OK button invisible (+0x29=0)**; Save dialog: OK visible but disabled until a slot is chosen.
- Stamp (0x4676a0, VMT 0x4678a0/0x467764/0x467798): +0x28 used, +0x29 visible, +0x2a hovered, +0x2b locked,
  +0x30 display thumb, +0x34 colour copy, +0x38 slot name.
  - Open: for each slot file exists → read header (name, thumb into +0x30), copy to +0x34 (colour), then **+0x30 converted to
    grey** (0x467544); used=1. Missing/broken → used=0, thumbs zeroed. All stamps unlocked.
  - Normal draw: used && visible → +0x30 (grey) O. Hover draw: used && visible && !locked → +0x34 (colour) O.
    **Unused slot: nothing drawn** (panel art). Normal = grey, hover = colour.
  - Update: locked → nothing. Inside && visible: hovered=1; LMB-down → result 0x16; if used draw hover. Else hovered=0, draw normal if used.
- Result 0x16: find hovered stamp.
  - Save mode: lock **all** stamps (no further slot changes this session), copy the menu snapshot into that stamp's +0x30
    (shown in colour), used=1; edit enabled, caret=len(text) (text = name currently shown, i.e. the hovered slot's name or
    empty); OK enabled; file = slot file.
  - Load mode: only if used: save name cleared, file = slot file, result 1 (closes immediately).
- Keys/tick (0x46740c): ESC → 2; Enter → 1 only if OK visible && enabled. While edit disabled: edit text := name of the
  hovered used stamp (else cleared) and redraw → name shown centred in the edit box (E6), colour 2.
- On close: [0x482f40] = edit text. Slot file 0x46537c = gameDir+"SAVEGAME\"+"Game%d.sav" (i=0..7).
- Grey conversion 0x467544 (per 16-bit pixel, using runtime masks/shifts): `r8=(p&Rm)>>Rpos<<Rloss` etc. (8-bit, low bits 0),
  `s=r8+g8+b8; a=s/3; rem=s%3; v=(rem<<8)|a` (u16, x86 `div dl` leaves rem in AH), then
  `out = (v>>Rloss<<Rpos) | (v>>Gloss<<Gpos) | (v>>Bloss<<Bpos)` without masking. RGB565: `((v>>3)<<11)|((v>>2)<<5)|(v>>3)`
  truncated to 16 bits (rem bits leak into G/R low bits — tiny tint; port can use plain grey).

## E10. Thumbnail (0x46763c, stamp blit 0x4674f8)
76×56 u16, row-major, native back-buffer 16-bit format (565 or 555 per card; store as RGB565). Nearest sample:
`dst[row*76+col] = back[ (row*420/56) * pitch + (col*640/76)*2 ]` (int div; rows 0..55 → y 0,7,15,22…, game view only).
Captured when the menu opens (cursor + hover tooltip hidden first; speech text may be on it). Blit opaque 76×56.

## E11. Dialog loops 0x465820 (generic) / 0x466250 (menu) / 0x466eb4 (save/load)
1. Wait until DXDraw can draw. Lock; hide cursor; Open (save-under + draw panel & children); show cursor; flip.
2. Loop: result=0; music tick (0x459714); clear events (0x464880); ProcessMessages (fills events);
   if any of key/LMB-down/LMB-up/mouse-moved/RMB (menu also: +0xe8 redraw request): Lock, hide cursor, result=0,
   vmt[0] Update (widgets redraw themselves), vmt[8] keys/result handler, cursor move+anim tick (0x450574), show, flip.
   **No full-screen redraw, no background/char animation, no darkening; nothing is drawn without input.**
   Save/load: extra 150 ms tick (base 0x4696b4, += 150) for caret blink; loop yields when <150 ms elapsed.
3. Exit when result ∈ {1,2} (or app terminating). Return result==1. Clear key-down; Lock, hide cursor, Close (restore
   save-under), show, flip.

## E12. Clicking hero / 2nd character (0x45a204 click, 0x45b7a0 hover) — room 0x7ff regions 0/1
Order: 1) current room's enabled regions, highest index first, polygon test 0x450ba4 at (mx,my); 2) if none: **2nd char**
(if its room == current and not walking +0xcc) → id 0x801 = room 0x7ff region 1; 3) hero (not walking) → 0x800 = region 0.
**Room 0x7ff polygons are never hit-tested**; only their cursor (+0x15), label (+0x2a) and scripts are used.
Character hit 0x4570a0 (bounding box of current sprite frame, scaled):
```
w = Round(frame.w / scale); h = Round(frame.h / scale)     // scale = f64 char+0x40 (divisor); 0 if isAnimNPC(+4)
left = x - trunc(w/2); top = y - h                          // x,y = feet (char +0x14/+0x18)
mx = 16.0/scale; my = 10.0/scale                            // float insets
hit = left+mx <= px < left+w-mx  &&  top+my <= py < top+h-my   (float compare)
```
Frame = 0x456ba4 current frame of list char+8. Hover: default cursor 5; region hit → cursor = region.cursor if >0 else
(walk area contains (mx,my+2) ? 2 : 5), label = region+0x2a; char hit → region0/1 cursor & label; nothing → walk test (y+2) → 2.
Click after hit: heldItem ≥ 0 && cursor 7 → ignore; cursor 0 → ignore; then flag 0x200 / room 0x200/0x204/0x205 limits
(else Say 0x20b); else hero stop (0x457c34), world+0x128 = region id, walk/exec 0x45c294. No hit + walkOnly + no item → walk 0x45c70c.

## E13. Inventory backdrop (0x463c58)
Hide cursor, cursor shape 0, hide tooltip; **Data 0x13 (640×480, trans idx 0) drawn Tu at (0,0)**; 142022 px are idx 0 →
the game view shows through those (not cleared first). Items: icons Data 0x11 list (44×44, trans 5) drawn T at
`(107 + 46*(i%9), 100 + 46*(i/9))` for i < hero.itemCount, skipping the item held on the cursor. Show cursor, flip.
Data 0x14 / panel strip 0x10 are not drawn here. No palette change (UI colours 0..45).
Cell from mouse 0x463c18: x<107 || y<100 || x≥521 → −1 else `((y-100)/46)*9 + (x-107)/46` (caller checks < count).
Hover (on mouse move): hovered item ≠ held → tooltip (0x46989c) text = item name (strings Data 0x20), pos (mx+10,my+10),
shown; otherwise hidden. Cursor = held item icon (static) or arrow anim 0. Tooltip colour is not set here: it keeps the
hover tooltip's current colour index (+8) as left by the main game. Bottom panel buttons are covered by the backdrop
(no panel hover in this screen).

## E14. Save file (writer 0x45be70, reader 0x45bc1c) — byte-exact, little-endian, no padding
```
u16 nameLen; u8 name[nameLen]                    // plain (0x44e1e4)
u16 thumb[76*56]                                 // 0x2140 bytes, E10; reader skips it (Seek +0x2140)
40 × character (0x457638 / 0x457188), 217 bytes:
  i32 roomId +0xfc; i32 x +0x14; i32 y +0x18; i32 +0x1c; i32 +0x20; i32 +0x24; i32 +0x28; i32 +0x2c; i32 +0x30;
  i32 +0x34; i32 +0x38; u8 dir +0x11; u8 +0x12; u8 finalDir +0x13; i32 +0x3c; f64 scale +0x40;
  u8 walking +0xcc; u8 +0xcd; u8 busy +0xf7; u8 +0x10; u8 +0xce; u8 [20] +0xcf; u8 isAnimNPC +4;
  i8 items[128] +0x48; i32 itemCount +0xc8
124 × room (0x451368 / 0x4512bc), file order:
  i16 bg +0x4c; u8 bgType +0x4e; i16 anim1 +0x50; i16 anim2 +0x52; i32 fg +0x5c; i32 fgY1 +0x60; i32 fgY2 +0x64; i16 pal +0x54;
  per region (count from world data), 11 bytes: u8 enabled +0x14; u8 cursor +0x15; i16 x +0x26; i16 y +0x28;
  i16 image +0x22; u8 imageType +0x24; i16 label +0x2a
u8 flags[256] (world+0x18); i32 currentRoom (+0x11c); i16 skin (+0x14); i16 heroAnim (+0x16);
i32 pendingRegion (+0x128); i32 pc (+0x12c); u8 busy (+0x130); u8 gameOver (+0x132);
i32 musicSet (0x469140); i32 defaultSpeaker (0x469144)
```
ENGINE.md §11 is correct. Load restores in this order:
1. each char: fields read, speaking flag +0x10b = 0, room pointer re-resolved from roomId (0x457b70);
2. rooms + regions (data only, no redraw yet); flags;
3. world+0x11c = −1 then **ChangeRoom(saved id)** (0x45a9b8: palette, back anims, companion follow, fade);
4. skin read → Data.dat[skin] reloaded into hero sprite list (world+8 item 0); heroAnim read → if hero isAnimNPC,
   hero+0xc = world+0xc list[heroAnim];
5. pendingRegion, pc, busy, gameOver, musicSet (local), defaultSpeaker;
6. after file closed: 0x45f7c0(musicSet): playlist pos 0, play first track of that set (table 0x469148 variant 0 / 0x469258
   variant 1 by cfg); full-redraw flag 0x4690d8 = 1.
Writer opens with mode 0xffff (fmCreate); errors → delete file + re-raise.

## Corrections to earlier docs
- C10/ENGINE §12 "Quit (no confirm)": wrong — Quit and ESC-outside-game open the quit confirm (img 0x5b); New game uses 0x5a.
- C12 "dimmed copy = each channel /3": wrong — it is a greyscale average (E9); normal = grey, hover = colour.
- C12: Load dialog has no OK button (hidden); load happens on stamp click. Save mode allows one slot pick per opening.
- Disabled buttons are drawn normally (not hidden); disabled trackbars lose their thumb; disabled checkbox shows no X.
- Widget clicks trigger on LMB-down.
