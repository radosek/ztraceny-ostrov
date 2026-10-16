# ENGINE addendum C — animations, minigame, menus, cursor, inventory (from asm)

Globals resolved (pointer var → storage): [0x469734]→0x4690b8 LMB-down, [0x4694f4]→0x4690bc RMB-down,
[0x469648]→0x4690d4 key-down, [0x4694cc]→0x470300 key code (u16), [0x469624]→0x4690d0 mouse-moved,
mouse x/y 0x4690c8/0x4690cc, [0x469894]→0x4690d8 / [0x469840]→0x4690dc "full redraw requested",
[0x46972c]→0x468b40 cursor object, [0x46989c]→0x468ca4 hover tooltip, [0x4694f0]→0x468ca8 speech tooltip,
[0x469718]→0x468ed4 music enabled, [0x4695dc]→0x468edc speech enabled, [0x469708]→0x482f38 "in game" (menu),
[0x469658]→0x482f44 menu result, [0x469728]→0x482f3c chosen save file, [0x4696f8]→0x482f40 save name,
[0x469810]→0x468b2c PlayFull y-offset, [0x4697f4]→0x468ca0 tooltip width param, [0x469494]→0x469130 playlist pos.
0x444738 = timeGetTime thunk; 0x441d40 = Application.ProcessMessages; 0x44f084 = Sleep/yield.

## C1. PlayPartial / PlayPartialX — 0x45ac50(world, partialIdx, delayMs, mode, fx)
Script mapping: PlayPartial a1 a2 a3 → (a1, delay=a2, mode=0, fx=a3); PlayPartialX a1 a2 a3 a4 → (a1, delay=a2, fx=a3, mode=a4) (corrected; earlier notes had fx/mode swapped).
1. delay 0 → 0x72 (114 ms/frame).
2. mode 2: first wait until back anim 1 finishes its cycle (0x45f81c(world, 0, 1)).
3. fx>0: FX.dat[fx] loaded (0x455cf8(FX, fx, 1)) and started after anim load (0x455e54) → sound synchronised with frame 0.
4. Load Partial.dat[idx] into a TFLC player (VMT 0x44e8c4, 0x44f4bc mode 0). Frame count = u16 @anim+0xb, frames 0..count-1.
5. Characters (corrected from asm 0x45ad47..0x45ae93): both hero and 2nd char are hidden first; then the
   hero is shown again only for mode ∈ {3,4,10}, the 2nd char (if in the room) only for mode ≤ 3 or 10.
   Kept characters are erased (0x457080) and redrawn (0x457bec) around every frame (0x45af73..0x45b13a).
   mode 1: after hiding, current frame of back-anim 1 is stamped into the background buffer (0x44fc40 → 0x4694c0).
6. Loop per frame: music tick (0x459714); if mode 2 and anim1 active, skip drawing until anim1 frame counter (+0x90) returns to 0
   (sync start); draw frame into back buffer (0x44f744 = decode delta frame onto [0x469758], pitch [0x469828]);
   mode 10: additionally rewind (0x44fb54 to saved frame pos) and decode the same frame into the background buffer
   (0x4694c0, pitch 0x500) → result stays painted on the room (permanent change); then back anims (0x45e140), hidden chars stay
   hidden, flip; busy-wait until `timeGetTime()-T >= delay`, `T += delay` (T=0x470304, absolute pacing).
7. ESC (key-down & code 0x1b) during wait → skip: remaining frames are decoded without delay (mode 10 also into background)
   and one flip shows the final frame.
8. End: stop FX (0x455da0), free, T=now. Characters redrawn (unless 0x45c154 later).
PlayPartial is blocking; the interpreter continues afterwards. Partial frames are drawn at the anim's own coordinates
(full-frame 640×420 deltas) – no x/y in the script.

## C2. PlayFull / PlayFullX — 0x45b2f0(world, animIdx, delayMs, stopMusic, mode, fx)
- PlayFull a1 a2 a3 → (Anim.dat a1, a2, stopMusic=0, mode=1, fx=a3). PlayFullX a1 a2 a3 a4 → stopMusic=(a4>=3), mode=a4%3, fx=a3.
- mode 0: save bottom panel rows 420..479 to 0x47032c and clear them (0x459860), fade/clear screen (0x44ef8c), y-offset 0 (full 640×480 anim).
- mode 2: like 0 but no fade-out.
- mode 1: panel kept, anim drawn with y-offset 0x38 (56) → letterboxed inside game view.
- stopMusic && music enabled → pause music (0x455c60) (resumed by caller/next tick).
- Palette 256 dwords backed up (0x4697c4→0x469440) and restored after. Anim.dat loaded in mode 2 (0x44f4bc(…,2)), flag +0x94=1.
- Same absolute-time frame pacing, default 114 ms; ESC skips (dword [0x470300]==0x1b).
- End: mode 0 → 0x44f008 (fade-in/restore) then restore panel; mode 2 → restore panel; palette restore; 0x44ed18 (apply palette).

## C3. Background / region-image redraw paths
- 0x45b5b0 RedrawRoom(world): if room.bg>0: load Backgnd[bg] (GIF if bgType=1 else RLE, 0x459990), blit rows 0..419 to
  back buffer, copy full 640×480×2 (0x96000) bytes back buffer → background buffer [0x4694c0]; restamp current frames of
  back-anim1/2 (0x44fc10). Then every *enabled* region with image≠0, highest index first: image>0 → draw Backgnd[image] at (x,y)
  into background; image<0 → 0x44ffa0 (restore/erase). Sets redraw flag 0x4690d8.
- 0x45baac(world, oldImg, oldType, newY, newX, newType, newImg, …): partial update used by Enable/Disable/SetRegionImage:
  if oldImg≠0 → restore the old image's rectangle from the clean room background (re-decodes Backgnd[room.bg] sub-rect),
  then draw newImg (>0 at its x,y; <0 erase).
- ChangeGround (op 8, asm 0x45cdf0): set room.bg/bgType; if room is current → 0x45b5b0.
- ChangeForeground (op 26, 0x45d316): set room.fg; if current → 0x45c1fc (draw Backgnd[fg] via image VMT+0x10 onto [0x469598]
  foreground layer).
- ChangeBackAnim (op 21): if current: wait anim1 end (0x45f81c(…,?,1))… then anim1=a1 and reload (0x45ba34: TFLC from Backgnd
  archive index |anim| mode 1, first frame stamped).
- ChangeBackAnimX (op 38, a1 anim, a2 room, a3 mode): if current room: a3≤2 → 0x45f81c(world, 0, a3) (a3=0 none, 1 wait anim1
  cycle, 2 wait anim2 cycle); a3=3/4 → stamp current frame of anim1 (3) / anim2 (4) into background first. Then
  a3∈{2,4} → room.anim2=a1 else room.anim1=a1; if current → 0x45b5b0 + reload that anim (0x45ba70 / 0x45ba34).
- Back anims tick: once per 70 ms tick in 0x45e140: anim>0 plays continuously (one frame/tick, loops); anim<0 = idle
  until Random(100000)<500 at frame 0, then plays one full cycle. Frame drawn to back buffer and background.
  Sound: table 0x468f20 {anim 666 → FX 58} triggered when anim frame counter == 1.

## C4. Wait helper 0x45f81c(world, n, mode) — used by Delay
- mode 0: Delay n ticks of 70 ms (op Delay a1 → n=a1). Each tick: music, redraw chars+anims (0x45c010), flip, pace 70 ms.
  LMB/RMB/ESC ends the delay early (only in mode 0).
- mode 1/2: run ticks until back anim 1/2 finishes its cycle (frame back to 0; ends at once if no anim or 1-frame anim).

## C5. Cannon minigame — SpecialFX 1 → 0x45ef1c (returns 1 = win → region 10, 0 = lost → region 11)
"Naval battleship" grid.
- Setup: palette backup; grid u8[8 rows][21 cols] zeroed. Ship: 2 rows × 6 cols at random top-left (col=Random(16), row=Random(7));
  cell value = row*6+col+1 (1..12 = ship tile index). Shots = 25.
- Assets: Data.dat 0x29 background (RLE, drawn at 0,0 full), Data.dat 0x2a TFLC anim (cannon/effects, Data archive), Data.dat 0x2b
  RLEImageList (ship tiles/markers), palette Data.dat 0x28. Cursor hidden-shape 0, tooltip used.
- Cell from mouse 0x45e7bc: x∈[46,591], y∈[42,249] → col=(x-46)/26, row=(y-42)/26, else (-1,-1). Tooltip shows "[%d,%d]"
  (col,row) at mouse+10 while hovering a cell (format @0x45f798).
- LMB on a cell: shots--; board redraw 0x45ee38; cannon-fire effect 0x45ebbc(effect 0x0c, x=320, y=305, frame 0x101).
  - empty cell: splash effect 0x45ebbc(0x18, x=46+26*col+12, y=42+26*row+14, 0x100).
  - ship cell: mark hit (v+=0x80 if v<0x81), draw hits 0x45e84c, explosion 0x45ebbc(0x12, 46+26c+10, 42+26r+10, 0x102),
    caption line 501 (0x45e930 = say line inside minigame). Win when no cell with 1..0x80 remains.
  - not won and shots==0 → line 502 → lose.
- Pacing 114 ms/frame; loop ends on win/lose. Teardown: free assets, fade, restore palette, redraw flag, T=now-57.
- Then the interpreter runs region 10 (win) or 11 (lose) script of the current room (room 0x20b "Pri dele") from PC 0.

## C6. Room-enter flow (0x45a9b8 ChangeRoom(world, id)) — supplement
1. ignore if id == current. old pal = current.pal.
2. current room = RoomById(id); hero.roomId=id, hero.room ptr (0x457b70).
3. If palette index changed (except 0x0e↔0x12): erase chars in y order; if id ∉ [0x64f..0x654] → fade out (0x44f008).
4. If flag 0x7ff (party together): 2nd char moved to same room, same x,y,dir as hero.
5. Load palette (0x45c274 → Backgnd[pal]), back anims 1/2 (0x45ba34/0x45ba70). Full redraw happens on next frame
   (0x4690e0 set by op ChangeRoom → 0x45c154 draws).
6. When the running script ends and +0x131 is set, the room's walk-area script (Enter script) runs (0x45d7ac) from its first
   matching FlagLabel block (StartRegion(walk, 0, -1, 0) + exec with reportEmpty=0).

## C7. Cursor object (0x468b40, unit 0x4502c4–0x4505ac)
Fields: +4 animated flag, +8 base cursor id, +0xc anim counter, +0x10 TRLEImageList (Data.dat 0x12), +0x14 current image,
+0x18 visible, +0x1c/+0x20 mouse x/y (GetCursorPos), +0x24 save-under buffer.
- SetAnimated(id) 0x4504d8: image = list[((counter*2)/3)*8 + id]. Tick 0x4503d4: counter=(counter+1)%9 → 6 frames, each shown
  1.5 ticks; list layout = 6 frames × 8 cursor ids.
- SetImage(img) 0x450514 (static cursor, e.g. held item icon from Data.dat 0x11 list).
- Hotspot = image centre (x-w/2, y-h/2). Drawn last onto the back buffer with save-under (0x45041c save, 0x450478 restore).
- Cursor id selection (0x462dd8 hover): over view y<420 → region cursor (0x45b7a0); bottom panel x 80..243/y 427..474 → 10,
  x 370..499 → 11, else −1; ids 1..7 used as-is, everything else → 0 (arrow). Holding an item → static icon list[item] instead.
  Ids: 0 arrow, 2 walk, 5 default/look, 1/4/6 per region data (take/use/exit…), 7 = walk-only hotspot.
- Panel buttons (0x463b80): two images from Data.dat 0x10 list at x 0 and 320, y 420; hovered button id 10/11 shows image +2
  (highlight) – list idx 0/1 normal, 2/3 hover.

## C8. Main-loop input (0x4630a4, verified)
Not talking:
- LMB, y<420 → 0x45a204(world, x, y, walkOnly=(hoverCursor==2), heldItem).
- LMB on inventory button (x 80..243, y 427..474) → heldItem = InventoryScreen().
- LMB on menu button (x 370..499, y 427..474) → GameMenu(form, inGame=1, fromAutosave=0); result 3 → restart game, 4 → quit.
- RMB: heldItem≥0 → drop it from cursor (heldItem=-1); else heldItem = InventoryScreen().
- Key 'I' (0x49) → InventoryScreen(). **ESC does nothing here** (no menu hotkey).
- Each tick: if heldItem no longer in hero inventory → heldItem=-1.
Talking (some char +0x10b or a Dialog active, 0x46303c):
- In Dialog: LMB/RMB → hide text, next line (0x45da10); ESC → abort whole dialog (0x45da6c).
- Single Say: LMB/RMB/ESC → stop speech (0x45dd20).
GameOver (+0x132) → Titulky.avi + menu(0,0).

## C9. Inventory screen 0x463d94 (modal, returns item id or −1)
- Draw 0x463c58: hide cursor, cursor=arrow anim 0, hide tooltip, draw panel/backdrop (0x4500b8), then hero items i<count at
  grid cell i (origin 107,100, 46×46, 9 per row) using icon list Data.dat 0x11 — the currently selected item is *not* drawn
  (it is on the cursor).
- Loop (music tick, input poll): hover over an item → tooltip with item name (strings 0x20) at mouse+10.
- LMB on item, none selected → select (cursor = its icon). LMB on the selected item → deselect. LMB on another item while one is
  selected → combine (table in ENGINE §6); success → close, nothing held; failure → "can't" line, stay. LMB outside grid →
  deselect.
- RMB → close and return the selected item (it stays on the cursor; −1 if none). ESC → close, return −1.
- After close: full redraw, tick base = now−35 ms.

## C10. Game menu 0x464538(form, inGame, fromAutosave) — TOptions (Data.dat 0x40, centred 640×480)
Buttons (id → handler 0x4664fc jump table @0x46651d):
| id | pos | widget | action → menu result [0x482f44] |
|---|---|---|---|
| 10 | (10,9) | button (+0xb8) | Continue → 0 (enabled only if inGame or fromAutosave) |
| 13 | (10,34) | button (+0xbc) | Load → opens Load dialog [0x482f54] (img 0x60); OK → 1 |
| 14 | (10,64) | button (+0xc0) | Save → opens Save dialog [0x482f50] (img 0x61); OK → 2 (enabled only if inGame) |
| 12 | (10,93) | button | New game → confirm dialog [0x482f48] (img 0x5a); yes → 3 |
| 11 | (10,118) | button | Quit → 4 (no confirm) |
| 15 | (92,257) | button | next music track (0x455c0c) |
| 17 | (10,178) | trackbar (+0xdc) | FX volume → cfg[13] |
| 18 | (10,228) | trackbar (+0xe4) | speech volume → cfg[14] |
| 16 | (10,278) | trackbar (+0xe0) | music volume → cfg[12] |
| 19 | (10,312) | checkbox (+0xd0) | subtitles → cfg[15] |
| 21 | (120,312) | radio (+0xd4) | music variant 0 → cfg[16]=0, playlist restart |
| 20 | (180,312) | radio (+0xd8) | music variant 1 → cfg[16]=1, playlist restart |
Sound widgets disabled when no DirectSound ([form+0x304]+0x44==0). Keys (0x4666fc): ESC → Continue if inGame else Quit;
RMB while inGame → Continue. Closing: result 2 → save world to [0x482f3c] with name [0x482f40] + thumbnail 0x469788;
1 → load [0x482f3c] (0x45bc20); 0 && fromAutosave → load SaveGame\auto.sav; 3/4 handled by caller (0x463004: 3 restart,
4 Application.Terminate). Entering the menu snapshots the game view into the thumbnail (0x46763c: 76×56 nearest sample of
640×420).
Startup: if auto.sav exists the menu is opened first with (inGame=0, fromAutosave=1) → only Continue/Load/New/Quit.

## C11. Confirm dialog 0x466cb0 (img 0x5a / 0x5b): Yes (17,62) id1, No (117,62) id2; ESC→2, Enter→1. Returns true on 1.

## C12. Save/Load dialog 0x466d8c (TSaveLoadDialog; +0xe8 = save mode)
- 8 stamps (TStamp 0x4676a0, 76×56) at table 0x4693d8; OK (245,261) id1 (+0xb8), Cancel (304,261) id2 (+0xbc), name edit
  TEdit (31,263) img 0x66 (+0xe4). ESC→2, Enter→1.
- Slot file 0x46537c(i) = gameDir + "SAVEGAME\" + Format("Game%d.sav", i), i = 0..7.
- Open (0x466eb4): for each slot, if file exists, read header only (0x4677d0: name string 0x44e254 → stamp+0x38, thumbnail
  0x2140 bytes → stamp+0x30, copy → +0x34, dimmed copy via 0x467544 = each RGB channel /3) and mark used (+0x28).
- Stamp click (id 0x16, 0x4672cc): save mode → all stamps deselected, current thumbnail copied into the clicked stamp, edit box
  enabled with that slot's current name, OK enabled, filename = slot file. Load mode → only used slots react: filename = slot,
  name cleared, result OK.
- Name typed via FormKeyPress chars ([0x4698a4]); stored in 0x482f40.

## C13. Video 0x464c0c(form, aviName, fxIdx)
TMediaPlayer (form+0x308) plays the AVI onto the DXDraw window, audio comes separately from FX.dat[fxIdx] played at start.
Music paused during and resumed after (if enabled). Waits until playback starts, then until it stops or LMB/ESC.
- Logo.avi (0x459664) + FX 0x1fa: once at program start (0x4630a4 prologue).
- Titulky.avi (0x459678) + FX 0x1f9: after the intro anim on a fresh start (no auto.sav), and at GameOver.

## Corrections to ENGINE.md
- §5 "ESC … opens menu": wrong — ESC in main loop only skips speech/dialog; the menu opens only via the panel button (or at
  startup when auto.sav exists). Inventory: panel button, RMB (when nothing held) or 'I'.
- §5 panel zones: inventory x 80..243, menu x 370..499, both y 427..474 (0x1ab..0x1da) — confirmed; panel button images at x 0/320.
- §5 Logo.avi plays once before the session loop, Titulky.avi after intro/at GameOver (fx 0x1fa / 0x1f9 confirmed).
- §6 item use: held item comes from the inventory screen; it is returned on RMB close, not by LMB pick alone.
- §12 trackbar mapping corrected: (10,178) id17 = FX, (10,228) id18 = speech, (10,278) id16 = music. Button meanings:
  id10 Continue, id13 Load, id14 Save, id12 New game (confirm), id11 Quit, id15 next track. Radios: (120,312) id21 = variant 0,
  (180,312) id20 = variant 1.
- §4 Delay: implemented by 0x45f81c mode 0 = a1 ticks of 70 ms (skippable); ChangeBackAnimX a3 semantics in C3.
- §4 SpecialFX 1: win → region 10, lose → region 11 (ENGINE has fail/success in that order: "11 (fail) / 10 (success)" = correct).
