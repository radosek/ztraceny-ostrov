# D. Rendering, timing, RNG, movement exactness (from asm)

All addresses Ostrov.exe VA. FPU control word is the Delphi default 0x1332: **53-bit precision, round-to-nearest-even**.
`Round()` = 0x402958 (`fistp` with the default CW, so **banker's rounding**: 2.5→2, 3.5→4). `Trunc()` = 0x402964 (CW 0x1F32).
Integer `div` is x86 `idiv` (truncates toward zero); `x div 2` on signed values is compiled as `sar`+`adc` (also toward zero).

## D0. Buffers (corrections to FORMATS.md)
- Screen = locked DirectDraw back buffer: ptr `0x46ce98`, pitch `0x46ce9c`, height `0x46cea0` = 480 (set on every Lock 0x44edf4).
  16 bpp. Every 8→16 conversion goes through `LUT u16[256]` at 0x46c898.
- `0x468b30` bg16: 640×480 u16, pitch 0x500. Clean room background + region images + back-anim frames. No characters.
- `0x468b34` copy of bg16 taken after the bg decode (0x45b5b0), used for region-image erase.
- `0x468b38` **priority mask** 640×480 u8 (only rows 0..419 written by the RLE unpack).
- `0x469598` is **not** a separate foreground layer: it is a pointer variable holding `&0x468b38`. `0x45c1fc` unpacks room
  `+0x5c` (the "fg"/mask image) into the mask. There is **no foreground image drawn over characters**. Occlusion is only
  "skip the sprite pixel where mask > depth", so the background (which already contains the foreground art) shows through.
  ChangeForeground (op 26) only swaps the mask.
- `0x468b3c` 8-bit index mirror of the last FLC frame (used for palette-chunk redraw). `0x468b4c` 8-bit scratch: every image
  is decoded here first (image VMT+0x10), then converted.
- Characters own a save-under buffer `+0x104` (390 bytes pitch × 165 rows, so 195×165 px max). The cursor owns `+0x24`
  (0x1c20 bytes, pitch 2w). Each tooltip owns `+0x14` (pitch 2·W).

## D1. Palette → 16-bit LUT (0x44ed18)
Palette `0x46c498` = 256 × {B,G,R,0} (RGBQUAD). The FLC COLOR_256 chunk stores file `r,g,b` into `[2],[1],[0]`, full 8-bit values,
so FLC palettes are not 6-bit.
```
for i in 0..256 { let (b,g,r) = (pal[i].0, pal[i].1, pal[i].2);
  lut[i] = ((b >> sB) << pB) | ((g >> sG) << pG) | ((r >> sR) << pR) }   // u16
// shifts: 0x46ceb4/b5 (B), 0x46ceb0/b1 (G), 0x46ceac/ad (R), filled from the DDPIXELFORMAT masks (not written by any
// direct-address instruction; derived by DelphiX surface code)
// RGB565: lut = ((r>>3)<<11) | ((g>>2)<<5) | (b>>3)        RGB555: ((r>>3)<<10)|((g>>3)<<5)|(b>>3)
```
Runs only if the DXDraw object exists (0x468b28≠0). Room palette load 0x44edb0: reads 1024 bytes and copies **entries 56..255 only**
(200 entries from byte 0xe0), then rebuilds the LUT. 0x44ed98: full 256-entry load and rebuild.
FLC palette chunk (type 4, only when anim `+0x94` = palette enabled): entries are written for index ≥ `[0x468b2c]` (0 in the image,
so all of them), then the LUT is rebuilt and **rows 0..419** of the current target are redrawn from the 8-bit mirror 0x468b3c:
`dst[y][x] = lut[mirror[y*640+x]]`. Nothing already on screen outside that target is re-converted, so anything drawn earlier
(panel, other buffer) keeps its old colours until it is redrawn.

## D2. Blitters
`img`: +4 w, +8 h, +0x10 transparent key byte. Every blitter first decodes img into the 0x468b4c scratch (w×h bytes).

### Opaque 0x44fec4(img, x=edx, y=ecx; maxY, maxRowBytes, dstPitch, dst)  (row helper 0x44fe9c)
```
let mut w = img.w; let mut h = img.h; let (mut sx, mut sy) = (0, 0); let mut xb = 2*x;
if 2*w + xb > maxRowBytes { w = (maxRowBytes - xb) / 2 }      // trunc toward 0
if h + y > maxY { h = maxY - y }
if xb < 0 { w += xb/2; sx -= xb/2; xb = 0 }                    // xb/2 == x
if y < 0  { h += y; sy -= y; y = 0 }
if w == 0 || h <= 0 { return }
for r in 0..h { for c in 0..w { dst[(y+r)*pitch + xb + 2c] = lut[src[(sy+r)*img.w + sx + c]] } }
```
Callers: room bg/regions `(…, maxY=420, 0x500, 0x500, bg16)`; panel buttons `(…, maxY=480, 0x500, pitch, screen)`.

### Transparent unclipped 0x44ffa0(img, _, pitch, dst, y, x)
Key = img+0x10 always. `if pix != key { dst = lut[pix] }`. No clipping at all.
### Transparent clipped to screen 0x450010(img, y, x)
Writes to the screen; a row is skipped unless `0 <= y < 480`, a pixel unless `0 <= x < 640`; `pix != key`. Used for cursor and inventory items.
### Transparent unclipped to screen 0x4500b8(img, y, x)
Same without clipping. Used for the inventory backdrop at (0,0).

### Rect copy 0x44eea4 (src=eax, srcPitch=edx, srcH=ecx; rows, rowBytes, dstY, dstXb, srcY, srcXb, dstWb, dstH, dstPitch, dst, srcWb)
```
if srcWb < rowBytes + srcXb { rowBytes = srcWb - srcXb }
if srcY + rows > srcH      { rows = srcH - srcY }
if dstWb < dstXb + rowBytes { rowBytes = dstWb - dstXb }
if dstH < dstY + rows      { rows = dstH - dstY }
if srcXb < 0 { rowBytes += srcXb; dstXb -= srcXb; srcXb = 0 }
if srcY  < 0 { rows += srcY;  dstY -= srcY;  srcY = 0 }
if dstXb < 0 { rowBytes += dstXb; srcXb -= dstXb; dstXb = 0 }
if dstY  < 0 { rows += dstY;  srcY -= dstY;  dstY = 0 }
for r in 0..rows { memmove(dst + (dstY+r)*dstPitch + dstXb, src + (srcY+r)*srcPitch + srcXb, rowBytes) }  // rowBytes<=0: no-op
```
All x values are in **bytes** (2·px).

### Scaled masked sprite 0x44fe5c(img, x=edx, y=ecx, depth:u8, scale:f64, useTrans:bool, flip:bool) → 0x44fcac
```
let mut wd = round(img.w as f64 / scale); let mut hd = round(img.h as f64 / scale);   // banker's
if wd <= 0 || hd <= 0 { return }
wd = min(wd, img.w); hd = min(hd, img.h);                     // never upscales
let (w, h) = (img.w, img.h);
let key = if useTrans { img.key } else { 0xff };              // NB: with useTrans=0, index 0xff is still transparent
decode(img → scratch);
if flip { for each row: reverse the w bytes in place }        // mirror happens on the source
let mut ey = 2*hd - h; let mut dy = y;
for sy in 0..h {
  if ey < 0 { ey += 2*hd; continue }                          // source row dropped, dy not advanced
  ey += 2*(hd - h);
  if 0 <= dy && dy < 480 {
    let mut ex = 2*wd - w; let mut dx = x;
    for sx in 0..w {
      if ex < 0 { ex += 2*wd; continue }
      ex += 2*(wd - w);
      let p = scratch[sy*w + sx];
      if 0 <= dx && dx < 640 && depth >= mask[dy*640 + dx] && p != key { screen[dy][dx] = lut[p] }
      dx += 1;
    }
  }
  dy += 1;
}
```
So this is Bresenham decimation (nearest-neighbour, no averaging). It emits exactly `wd` columns and `hd` rows. The mask is read at
`dy*640+dx` for any dy < 480 (rows ≥ 420 are uninitialised GetMem memory, but the feet y is clamped ≤ 419 and sprites end at feet).
Depth: `0xff` = no mask (room +0x5c ≤ 0), so everything passes. Otherwise depth 0/1/2 comes from fgY1/fgY2 (ENGINE_B), and a pixel
is drawn iff `mask <= depth`. Mask values are layer ids (0 = free floor).

### Character draw 0x456a18 (inside Show 0x457bec)
```
if x == -255 && y == -255 { return }
if isAnimNPC { anim.drawFrames(screen, pitch, also bg16 with pitch 0x500) via 0x44fc40; return }
frame = spriteFrame(); img = sheet[frame];
depth = if room.mask<=0 {0xff} else if room.fgY1<=0 || y<room.fgY1 {0} else if room.fgY2<=0 || y<room.fgY2 {1} else {2};
yb = if skin==0x0e { y - round(((bob as u32)/3) as f64 / scale) } else { y };     // bob = +0x109 (0..5), u32 div
wR = round(img.w/scale); hR = round(img.h/scale);
blit(img, x - wR/2 /*i64 div, toward 0*/, yb - hR, depth, scale, useTrans=1, flip = dir<4 && skin!=0x0e)
```
Show 0x457bec: `+0x100=1`; if not animNPC: `+0x108 = spriteFrame()`, **save-under** 0x457380, then draw.
Save-under / erase rectangle (0x457380 / 0x457584, using frame +0x108):
`wR = round(w/scale)`, `hR2 = round((h+2)/scale)`, rect = x `[x - wR/2, +wR)`, y `[y - hR2, y)`, clipped by 0x44eea4 to 640×480
(save: screen → buffer pitch 390; erase: buffer → screen). The rect does **not** include the skin-0x0e bob offset (it is covered by the +2).
Hide 0x457080: `+0x100=0`; if not animNPC: restore. Show/Hide 0x457b90(c, on) does nothing if the state is already `on`.

### Glyph 0x4516e8(glyph:u8, colour:u8, y, x)
Font `*0x468cac`, glyph = 10 wide × 17 tall bytes (0xAA). Unclipped, written to the screen:
`v==0` → skip; `v==2` → `lut[0]` (outline); any other value → `lut[colour]`. Text 0x45176c(x, y, colour, s): char i at `x + 9*i`
(9 px advance), glyph = map(char) 0x4516b8.

### Tooltips (0x468ca4 hover, 0x468ca8 speech; class at 0x4519b0)
- Fields: +4 text, +8 colour, +9 visible, +0xc left, +0x10 top, +0x14 save buffer, +0x18 lines, +0x1c maxLen.
- **Colour**: hover tooltip is constructed with colour **1** and nothing ever changes it. The speech tooltip starts at 0 and
  SetColor 0x451c84 is called only from 0x4578f7 with the speaking character's `+0x10a`.
- SetText 0x451834: `"`→0xff and `'`→0xfe, then `WrapText(s, #13, [#0..' '], [0x468ca0])`, then swap back. Wrap width
  `0x468ca0` = 40 (the minigame sets 70 via `0x4697f4` and resets it to 40; this is **not** a fade speed). lines = 1 + count(#13),
  maxLen = longest line in chars.
- Size: `Wpx = 9*maxLen + 1`, `H = 17*lines`. Clamp 0x4517c4: `left=max(left,0); if left+Wpx>639 {left=639-Wpx}; top=max(top,0);
  if top+H>480 {top=480-H}`.
- Draw 0x451a74: for line i: `x = left + (Wpx - 9*len)/2` (toward 0), `y = top + 17*i`, text in colour +8.
- Show: save-under (screen rect left,top,Wpx,H → buffer) and then draw. Hide: restore. SetText/SetPos/SetColor on a visible tooltip
  does hide, change, show. Hover position = mouse + (10,10).

### Cursor (0x468b40, class 0x4502c4)
Fields: +4 animated, +8 base image index, +0xc anim counter, +0x10 image list, +0x14 image, +0x18 visible, +0x1c/+0x20 mouse x,y.
Top-left = `(mx - img.w/2, my - img.h/2)` (div toward 0). Show 0x450560 = save-under 0x45041c (screen → +0x24, rect 2w bytes × h,
clipped to 0x500 bytes × 480) then draw 0x450338 (0x450010 transparent clipped). Hide 0x4503cc → 0x450478 restore (same rect).
Animate 0x4503d4 (only if +4): `c = (c+1) % 9; img = list[base + 8*((2*c)/3)]` (sub-frames 0,0,1,2,2,3,4,4,5).
SetShape 0x4504d8(n): `base=n`, image from current counter, `+4=1`. 0x450574: hide, GetCursorPos → +0x1c/+0x20, show.

### Bottom panel 0x463b80
Two buttons, y = 420, x = table 0x4693cc = {0, 320}. image = `form.list318[i + 2]` if `form+0x328 == 10+i` (hovered) else `[i]`.
Opaque 0x44fec4 onto the screen. Hover ids (0x462dd8, mouse y ≥ 420): x∈[80,244) & y∈[427,475) → 10; x∈[370,500) & y∈[427,475) → 11.
Panel save/restore 0x459860 / 0x459910: copy screen rows 420..479 (0x500 bytes/row) to `0x47032c`, then clear rows 420..479 to 0.
Restore copies them back. Each does its own Lock/Unlock and no Flip.

### "Fades": there are none
- 0x44ef8c: hide cursor, then **memset rows 0..479 of the screen to 0**, Unlock, Flip (0x44f084).
- 0x44f008: hide cursor, memset rows **0..419** to 0, Unlock, Flip.
- 0x44f084 = `DXDraw.Flip` (0x44d380: if initialised and not in "no-flip" state → primary.Flip). It is not a wait or yield.
  Wait loops call it after drawing to present the frame.
- 0x44ee68: copy bg16 rows 0..419 → screen (full background restore).

## D3. Main loop tick (0x4630a4 body at 0x4632d2..0x463936)
Timing: `T` (`*0x4696b4`) is a fixed-step clock. After rendering, the loop polls input/ProcessMessages (0x46355c..) until
`timeGetTime() - T >= 70`, then **`T += 70`** (not `T = now`). So ticks catch up after stalls.
```
tick:
  music tick 0x459714; FPS bookkeeping
  if DXDraw.CanDraw {
    Lock
    talk = 0x46303c()                 // speech/dialog active (advances it via 0x45da10)
    cursor.hide(); hoverTT.hide(); speechTT.hide()          // LIFO restore of save-unders
    if talk changed || firstFrame { hoverUpdate 0x462dd8 (cursor shape, hover label text/pos); panel 0x463b80 }
    if *0x469894 || *0x469840 { 0x45c154(world, all=1); panel; clear both flags }
    0x45c010(world)                   // erase chars, update chars, back anims, draw chars
    if !talk { if form+0x331 {hoverTT.show()}; cursor.updatePos(); cursor.animate(); cursor.show() }
    else     { speechTT.show() }
    Unlock
    if debugFPS (form+0x33c) draw FPS text via GDI
    if timeGetTime() - T < 70 { Flip }          // a late frame is NOT flipped
  }
  … input handling until 70 ms elapsed, then T += 70
```
Frame contents therefore: back buffer = previous frame with the cursor, tooltips and characters removed through their save-unders.
On it go the back-anim FLC deltas (screen and bg16), then the characters (each saves the screen under itself first), then the
hover tooltip, then the cursor. The panel is only redrawn on hover/talk change or a full redraw. A port that wants byte-exact output
can render `bg16(0..419) → anims → chars in order → panel(420..479) → tooltip → cursor` each tick. That gives the same pixels, because
every save-under restore puts back exactly bg16 + earlier layers.

### 0x45c010 Frame(world)
```
let order_up = hero.y < char2.y;            // evaluated before the update
if !order_up { for i in 0..40 { if c[i].room==cur { hide(c[i]) } } }
else         { for i in (0..40).rev() { if c[i].room==cur { hide(c[i]) } } }
for i in 0..40 { update(c[i]) }             // 0x457ff0, all chars regardless of room
backAnims(world)                            // 0x45e140
let order_up = hero.y < char2.y;            // re-evaluated with new positions
if order_up { for i in 0..40 { if c[i].room==cur { show(c[i]) } } }
else        { for i in (0..40).rev() { if c[i].room==cur { show(c[i]) } } }
```
(`room==cur` compares char `+0xf8` with world `+0x118`.) Hide uses the reverse of the previous draw order, so restores nest
correctly. NPCs are ordered by index only. Only hero and char2 are y-sorted, and only against each other.

### 0x45c154 RedrawChars(world, includeHero)
`0x44ee68` (bg16 rows 0..419 → screen), then Show (0x457bec, unconditional: saves and draws even if already drawn) for every char in
the current room, ascending if hero.y < char2.y else descending, skipping index 0 when `includeHero == 0`.

### Back anims 0x45e140 (per tick, inside Frame)
```
for (slot, field) in [(world+0x120, room.anim1 +0x50), (world+0x124, room.anim2 +0x52)] {
  if field > 0 { step(slot) }
  if field < 0 { if slot.frame(+0x90) != 0 { step(slot) } else if Random(100000) < 500 { step(slot) } }
  if field != 0 && slot.frame == 1 { for e in table468f20[0..3] { if e.anim == field { FX.Play(e.fx) } } }
}
if !fxBusy() /*0x456494*/ { for e in ambient468f38[0..48] { if e.room == world.curRoomId(+0x11c) && Random(100000) < 600 { FX2.Play(e.fx) } } }
step(a) /*0x45e0fc*/: f=a.frame; a.drawNext(screen, pitch); a.seek(f); a.drawNext(bg16, 0x500)   // same delta applied to both
```
FLC frame draw 0x44f744 writes 16-bit pixels via the LUT straight into the given target (chunk 0x80 = raw rect blocks
`{x,y,w,h}` + w·h bytes). After the last frame it goes back to frame 1 if the anim loops (0x44f4b0), else to 0.

## D4. Hero click-walk 0x45c70c(world, clearPending, x, y)  (exact)
```
let s = hero.scale;
if !( (|hero.x - x| as f64) > 14.0/s || (|hero.y - y| as f64) > 7.0/s ) { return }
let mut y2 = y + max(2, trunc(10.0 / s));  y2 = min(y2, 419);
if PtInRegion(room.walk, x, y2) { hero.clearFinalDir(); WalkTo(hero, hero.x, hero.y, reset=1, y2, x) }
else if PtInRegion(room.walk, x, y+2) && PtInRegion(room.walk, x, y) { hero.clearFinalDir(); WalkTo(hero, hero.x, hero.y, 1, y, x) }
else { return }
if clearPending { 0x45abfc(world) }
```
(`trunc`, not round. 14/7/10 are f32 constants at 0x45c8bc/c0/c4.)

## D5. Movement exactness
### PtInRegion 0x450ba4(poly, pt)
`n = poly+4` points at `poly+8` (i32 x,y). Requires n > 2. Pre-test bbox (i16 at +0xc,+0xe,+0x10,+0x12):
`left <= x < right && top <= y < bottom`. Then `rgn = CreatePolygonRgn(pts, n, WINDING)`, `PtInRegion(rgn, x, y)`, DeleteObject.
GDI semantics: non-zero winding fill, a pixel is in the region when its centre lies inside, which makes the
right and bottom edges exclusive and the top and left edges inclusive. A port can use **sample at (x+0.5, y+0.5) with non-zero winding**.
That is exact for axis-aligned edges. On slanted edges GDI's own edge DDA can differ by one pixel (not reproducible without GDI).
The bbox (Region.CalcBBox 0x450e58) also bounds the result.

### Segment test 0x450c48(poly, A, B)
```
if !bbox(A) || !bbox(B) { return false }          // endpoints only bbox-tested here
for k in 1..=10 {                                 // A itself is never polygon-tested; B is (k=10)
  let px = A.x + (B.x - A.x)*k / 10;              // i32, idiv toward zero
  let py = A.y + (B.y - A.y)*k / 10;
  if !PtInRegion(rgn, px, py) { return false }
}
true
```

### Angle 0x456540(x1,y1,x2,y2) → 0..359
```
dx = x2-x1; dy = y1-y2;
a = if dx==0 || dy==0 {0.0} else { atan((|dy| as f64)/(|dx| as f64)) * 180.0 / PI };   // fpatan(q,1) = atan, not atan2
if dx<0 && dy>0 {a = 180.0 - a}  if dx<0 && dy<0 {a += 180.0}  if dx>0 && dy<0 {a = 360.0 - a}
if dx==0 && dy>0 {a=90.0}  if dx<0 && dy==0 {a=180.0}  if dx==0 && dy<0 {a=270.0}
return (round(a) + 360) % 360                     // banker's round, i64 mod
```
Facing 0x457430: `a>30 && a<=150 → 0; 150<a<=210 → 6; 210<a<=330 → 4; else → 2` (Turn).

### TryDirect 0x457c64(char, x0, y0, destX, destY, force, &ok)
```
ok = true; if dest == (x0,y0) { return }
dx = destX-x0; dy = destY-y0;
straight = dx==0 || dy==0
        || |atan(dx as f64 / dy as f64)| <= 20°(0.3490658503988659 rad)
        || |atan(dy as f64 / dx as f64)| <= 10°(0.17453292519943295 rad)
        || (|dx| <= 5 && |dy| <= 5);
W = room.walk;
if straight {
  if PtInRegion(W,dest) && PtInRegion(W,(x0,y0)) && !seg(W,(x0,y0),dest) && !force { ok=false; return }
  // NB: if dest or start is OUTSIDE the polygon the straight walk is accepted
  queued=(-1,-1); segEnd=dest; segStart=(char.x,char.y); face(); moving=1; return
}
ox = sign(dx) * round(|tan20 * dy|);  oy = sign(dy) * round(|tan10 * dx|);   // tan = sin/cos in f64
A = (destX - ox, y0 + oy);  B = (x0 + ox, destY - oy);
let both_in = PtInRegion(W,dest) && PtInRegion(W,(x0,y0));
corner = if !both_in || (seg(W,start,A) && seg(W,A,dest)) || force { A }
         else if seg(W,start,B) && seg(W,B,dest) { B }
         else { ok=false; return };
queued = dest; segEnd = corner; segStart = (char.x,char.y); face(); moving = 1;
```
(`start` = (x0,y0). `sign(0)=0`.)

### Waypoint pick 0x4579c0(char, start, dest) → index or -1
```
best = 1_000_000.0f64; res = -1;
for i in 0..room.waypoints.len() {          // room+0x18 count, get 0x4510c4
  let w = wp[i];
  if visited[i] || tried[i] || w.x==start.x || w.y==start.y || w.x==dest.x || w.y==dest.y { continue }
  let d1 = sqrt(((start.x-w.x)^2 + (start.y-w.y)^2) as i32 as f64);
  let d2 = sqrt(((dest.x-w.x)^2 + (dest.y-w.y)^2) as i32 as f64);
  let mut cost = d1 + d2;                                   // stored as f64
  if walkable(char, w, dest) /*0x456648*/ { cost -= 50.0 }
  let mut ad = |angle(start,w) - angle(start,dest)|; if ad > 180 { ad = 360 - ad }
  cost += (ad / 2) as f64;                                  // integer halve first
  if cost < best { best = cost; res = i }                   // strict: first minimum wins
}
```

### Per-tick step 0x457ff0 (exact math, extends ENGINE_B)
- `step = round(base / scale)` (banker's, f64). For skin 0x0e: `max(step, 3)` (0x44e4dc = max).
- Major axis: x if `|ex-sx| > |ey-sy|` (strict), else y. Direction: x-major moves `-step` if `ex < sx` else `+step`; y-major `-step`
  if `sy > ey` else `+step`. Overshoot past segEnd: `+0xce==0` → snap to segEnd; else move back by one more step (`pos ∓= step`).
- Minor axis: `y = sy + (|x - sx| * (ey - sy)) / |ex - sx|` (i32 product, idiv toward zero); y-major is symmetric
  `x = sx + (|y - sy| * (ex - sx)) / |ey - sy|`.
- Turning: `d = target - dir`; if `|d| == 1` snap; else compare `(d+8) mod 8` vs `(dir-target+8) mod 8` (C-style remainder made
  non-negative); if the first is larger → `dir = (dir+7) mod 8` else `dir = (dir+1) mod 8`; frame=-1.

### Scale 0x4574ac
`scale = ((room.y1(+0x58) - room.y0(+0x56)) as f64) / ((y - y0) as f64)` (fild/fdivp, rounded to f64 on store); `y == y0 → 1.0`;
`scale < 1.0 → 1.0`. (Negative results, y < y0, are also clamped to 1.0.)

## D6. Random
- `Random(n)` 0x402ae0: `seed = seed*0x08088405 + 1 (wrapping u32); return ((seed as u64 * n as u64) >> 32) as u32`. Seed at 0x46a03c
  (initialised to 0 by the System unit, 0x405fae).
- `Randomize` 0x4028fc: `GetSystemTime` (UTC) → `seed = ((hour*60 + minute)*60 + second)*1000 + ms`. Called from two unit
  initialisers (0x4595dd, 0x45ff8d) at startup, so effectively seed = UTC millisecond-of-day at launch.
- All call sites:
  | addr | call | purpose |
  |---|---|---|
  | 0x4593fe | Random(1500) | copy-protection registry value (0x459328). Consumes **one** value at startup; a port that skips CP must call Random once to stay in sync |
  | 0x4597c5 | 0x200+Random(6) | "that doesn't work" line; callers 0x45c5e3 (use/combine fail in world click) and 0x464209 (inventory combine fail) |
  | 0x4597d5 | 0x206+Random(4) | "nothing happens" line; caller 0x45d764 |
  | 0x45e17e | Random(100000) < 500 | anim1 random trigger, per tick while room.anim1 < 0 and the anim is at frame 0 |
  | 0x45e21e | Random(100000) < 500 | same for anim2 (after anim1 in the same tick) |
  | 0x45e2a7 | Random(100000) < 600 | ambient FX: one roll per table-0x468f38 entry whose room == current, in table order, only if no ambient FX is playing at the start of the loop (several can fire in one tick) |
  | 0x45f08d / 0x45f099 | Random(16), Random(7) | minigame ship column, then row |
  Order inside one tick: anim1 roll → anim2 roll → ambient rolls (inside Frame, after character updates).

## Unknown / not reproducible
- Exact R/G/B shift values: read at runtime from the DirectDraw surface pixel format (565 vs 555 depends on the card). The
  formula is exact; assume 565.
- GDI polygon scan conversion on slanted edges (PtInRegion) can differ by one pixel from a centre-sample rasteriser.
- `[0x468b2c]` (first palette index the FLC palette chunk may write) is 0 in the image and never written by direct-address code.
  Treated as 0.
- Mask rows 420..479 are uninitialised. They matter only if a sprite extends below y = 419, which the feet clamp prevents.

## D7. Cannon minigame drawing (0x45ef1c, frame-local vars relative to its ebp)
Locals: `-0xa8` grid u8[8][21], `-0xac` image list, `-0xb4` world, `-0xb8` FLC (keeps animating under everything), `-0xbc`
effect sprite (a cursor-class object: centred, save-under, transparent clipped 0x450010), `-0xc0` shots left, `-0xc8` backdrop
image, `-0xad` hover-tooltip visible. Pacing is a fixed step of **114 ms** (`T470304 += 0x72`), and Flip only happens if the frame finished within 114 ms.
- Setup redraw: backdrop opaque at (0,0) (maxY 480), then the ammo bar, then show the cursor.
- Tick: hide cursor and hover tooltip. If redraw flag `0x4690dc`: backdrop at (0,0), FLC seek(0), board (0x45e84c), clear flag.
  Then FLC next frame → screen only, hover tooltip visible = `-0xad`, show cursor.
- **Ammo bar 0x45ee38**: image 31 opaque at (0,420); for i in 0..shotsLeft: image 30 at `(105 + 17*i, 437)` (0x4500b8, unclipped).
- **Board 0x45e84c**: hide cursor and hover tooltip; for row r 0..8, col c 0..21: `v = grid[r][c]; if v >= 0x81 { draw list[v-0x81] at
  (46 + 26*c, 42 + 26*r) }` (0x4500b8); hover tooltip visible = `-0xad`; show cursor.
- Click → cell 0x45e7bc: `x∈[46,592) && y∈[42,250)` → `(col,row) = ((x-46)/26, (y-42)/26)`, else (-1,-1).
- Shot: `shots -= 1`; ammo bar; `effect(12, 320, 305, fx 0x101)` (muzzle). Then if `grid[r][c]==0` → `effect(24, 46+26c+12, 42+26r+14,
  fx 0x100)` (splash). Otherwise, if `grid < 0x81` then `grid += 0x80` (ship cells hold 1..; hit shows image `orig-1`); board;
  `effect(18, 46+26c+10, 42+26r+10, fx 0x102)` (explosion); `Say(0x1f5)`. Win = no cell left in 1..0x80. If not won and shots==0 → `Say(0x1f6)` and lose.
- **Effect player 0x45ebbc(first, x, y; fx, frame)**: if fx>0 FX.Play(fx, loop=1). Hide cursor, hover tooltip and sprite; sprite.pos=(x,y)
  (hotspot = image centre). If fx>0 call 0x455e54. Then for f in 0..=5 (6 frames, 114 ms each): hide sprite;
  `sprite.img = list[first+f]`; FLC next frame → screen; show sprite (save-under + draw); Flip if in time. After that, hide sprite,
  stop FX 0x455da0, 0x45f7a0. So effect "frames" are 6 consecutive **images** from the minigame image list (12..17 muzzle,
  18..23 hit, 24..29 splash), not FLC frames.
- **In-minigame Say 0x45e930(id)**: hide cursor and hover tooltip; duck music 0x4597e0; start Say 0x45d908. Loop at 114 ms: hide speech
  tooltip; FLC next frame → screen; talk tick (dl=1, no NPC-anim stepping) for `World.Char(table 0x469138[id].speaker)`; show
  speech tooltip; Flip if in time. Ends when speech is done (0x45dd54) or on click/ESC (0x45dd20). Then hide the tooltip and unduck (0x459838).
  The cursor stays hidden during the Say. The tick redraws the cursor afterwards.

## D8. Glyph index table (0x451610 init, lookup 0x4516b8)
```
glyph(c) = if c < 0x20 { 0 } else if c < 0x80 { c - 0x20 } else { T[c - 0x80] }   // T = bytes at 0x468cb1, returned as-is
init: for c in 0x80..=0xff {
  T[c-0x80] = 0x5f;                                    // unknown → glyph 0x5f ('\x7f'-0x20)
  for i in 0..47 { if c == ACC[i] { T[c-0x80] = 0x60 + i; break } }    // first match
}
ACC (cp1250, 0x451688) = "ÁÄČĎÉĚÍĽĹŇÓÔÖŔŘŠŤÚÜÝŽŁáäčďéěíľĺňóôöŕřšťúüýžłßŮů"   // glyphs 0x60..0x8e (font has 143 glyphs)
overrides, in this order, after the loop:
  T[0x04] (0x84 „) = 2   T[0x11] (0x91 ‘) = 7   T[0x12] (0x92 ’) = 7   T[0x34] (0xb4 ´) = 7
  T[0x13] (0x93 “) = 2   T[0x14] (0x94 ”) = 2   T[0x3d] (0xbd ˝) = 2   T[0x20] (0xa0 nbsp) = 0
```
(2 = glyph of `"`, 7 = `'`, 0 = space.)

## D9. WrapText as used (Delphi 5 SysUtils 0x40a0a8, verified structure)
Call: `WrapText(s, BreakStr = #13, BreakChars = [#0..#32], MaxCol = [0x468ca0] (40))`. `"` and `'` are swapped to 0xff/0xfe first, so
the quote logic never triggers. Algorithm (1-based):
```
col=1; pos=1; linePos=1; breakPos=0; existing=false; out="";
while pos <= len {
  ch = s[pos];
  if ch == '\r' { existing = s[pos..pos+1] == "\r"; if existing { breakPos = pos } }      // BreakStr length 1
  else if ch in [#0..#32] { breakPos = pos }
  pos += 1; col += 1;
  if existing || (col > MaxCol && breakPos > linePos) {
    col = pos - breakPos;
    out += s[linePos ..= breakPos];                         // the break char (space or CR) stays at the end of the line
    while pos <= len && s[pos] in [#0..#32, #13, #10] { pos += 1 }   // only advances the scan
    if !existing && pos < len { out += "\r" }
    breakPos += 1; linePos = breakPos;                      // NB: next line starts right after the break char,
    existing = false;                                       //     so extra skipped blanks are still copied
  }
}
out += s[linePos..];
```
Consequences: a wrapped line keeps its trailing space, which counts in `len` for centring (D2 tooltip x) and in maxLen. A word
longer than 40 is never split. A wrap is only taken once `col > 40` and a blank exists after the line start. No CR is appended when
the break is at the last char (`pos >= len`).

## D10. Anim NPCs (char +4 = 1)
- Their FLC (`+0xc`, Partial.dat) frames are full-screen 640-wide deltas at **absolute** screen coordinates. The position is baked
  into the FLC. `x,y` from Data 0x27 only position the speech tooltip and the y-sort, and the NPC is never blitted at x,y.
- They have **no** save-under: Show 0x457bec / Hide 0x457080 do nothing for them, and Frame/RedrawChars never draw them. The idle FLC
  frame is therefore part of whatever is in bg16/screen.
- Talk tick 0x4585d4(char, noAnim): `talkTicks += 1; toggle ^= 1;` if `!noAnim && isAnimNPC && anim && toggle==1`:
  `f = anim.frame; anim.drawNext(screen); anim.seek(f); anim.drawNext(bg16, 0x500)`. So the FLC advances **every second tick** while
  talking and writes both buffers. Then the stop rules from ENGINE_B apply (0x457c20).
- EndTalk 0x456948 (called from the Say/Dialog end paths 0x45dce5, 0x45deb1, 0x45dfe7): if isAnimNPC && anim: 0x44fc40, which
  does `if anim.frame != 0 && anim.frames > 1 { repeat (frames - frame) times { f=frame; drawNext(screen); seek(f); drawNext(bg16) } }`.
  It plays the rest of the cycle at once, so the anim wraps to its rest frame (0, or 1 if it loops), and both buffers end
  on that pose. The same call is used by 0x456a18's NPC branch, ChangeRoom 0x45aebf and ops 0x45d5b9/0x45d5f6.
