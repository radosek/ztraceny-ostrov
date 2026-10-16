# B. TCharacter: layout, walking, pathfinding, scaling, drawing (from asm)

Unit 0x456494–0x458694. 40 instances in world+0x10: [0]=hero, [1]=2nd playable char, [2..39]=NPCs.
Ctor 0x456984(cls, alloc, hasSaveBuffer): +0x3c=-1, +0x13=0xff; if hasSaveBuffer alloc 0xfb5e bytes (+0x104, 390×165 RGB16 save-under).

## Field layout
| off | type | meaning |
|---|---|---|
| +0x04 | u8 | isAnimNPC: drawn by an FLC anim object (+0xc) instead of sprite list; also "can't walk" (0x456ea0) |
| +0x08 | TRLEImageList* | sprite sheet (hero = Data.dat 3, char2 = Data.dat 0xf) |
| +0x0c | anim* | NPC anim (from world NPC anim list, Data.dat 0x25/0x26) |
| +0x10 | u8 | skin = Data.dat index of sprite set (hero default 3; ChangeSkin writes it). Special-cased: 3, 8..11, 0x0c, 0x0e, 0x0f |
| +0x11 | i8 | current dir 0..7 |
| +0x12 | i8 | target dir (turning) |
| +0x13 | i8 | final dir after walk (0xff = none) |
| +0x14/+0x18 | i32 | x,y (feet point). (-255,-255) = not drawn |
| +0x1c/+0x20 | i32 | current segment start |
| +0x24/+0x28 | i32 | current segment end |
| +0x2c/+0x30 | i32 | queued 2nd leg end (−1 = none) |
| +0x34/+0x38 | i32 | final destination |
| +0x3c | i32 | walk frame counter (−1 = standing) |
| +0x40 | f64 | scale divisor (≥1.0) |
| +0x48 | u8[128] | inventory, +0xc8 i32 count |
| +0xcc | u8 | moving/turning |
| +0xcd | u8 | "just stopped" countdown (set 2, −1/tick) |
| +0xce | u8 | path goes via waypoint (enables arrival tolerance) |
| +0xcf | u8[20] | waypoint visited |
| +0xe3 | u8[20] | waypoint tried (per plan) |
| +0xf7 | u8 | busy (script-driven walk pending) |
| +0xf8 | TRoom* | room; +0xfc i32 room id |
| +0x100 | u8 | currently drawn |
| +0x104 | ptr | save-under buffer |
| +0x108 | i8 | sprite frame being drawn |
| +0x109 | u8 | 0..5 cycling tick counter (bob for skin 0xe) |
| +0x10a | u8 | talk colour (palette idx, Data.dat 0x25 hi; hero 5, char2 0xd) |
| +0x10b | u8 | talking; +0x10c i32 talk ticks; +0x110 i32 talk duration ticks (0 = until speech sample ends); +0x114 u8 toggle |

Setters: SetDir 0x457b58 (dir=target=v, stop), SetRoom 0x457b70 (+0xf8, recompute scale), Turn 0x457b7c (target=v; moving if ≠),
SetX 0x457bac / SetY 0x457bcc (pos=seg end=dest=v, stop, rescale), Stop 0x457c34 (dest=seg=pos, +0xcd=2),
StopTalk 0x457c20, Show/Hide 0x457b90 (save-under + draw 0x457bec / restore 0x457080).

## Angle & directions
`angle(x1,y1,x2,y2)` 0x456540: dx=x2−x1, dy=y1−y2 (screen-up positive); a=deg(atan(|dy|/|dx|)), quadrant-fixed to 0..360,
axis cases 0/90/180/270, dx=dy=0 → 0; result `(round(a)+360) mod 360`. 0°=east, 90°=up(away).
Dirs: 0=back(up), 2=right, 4=front(down), 6=left, odd = diagonals (only used while turning/idle/talking).
Walk facing (0x457430, at each segment start): angle 31..150→0, 151..210→6, 211..330→4, else→2 (via Turn).
Turning (in update): if target≠dir: |diff|==1 → snap, else step ±1 (mod 8) along shorter way; frame=−1; no translation that tick.
TurnTowards (0x45fb44, op 40 a1,a2): char a2 turns toward char a1: angle(a2→a1) 151..210→5, 211..330→4, >330 or <31→3, else 0;
executed as StaticTurn(dir, a2).

## Sprite frame selection 0x456ba4 (index into sprite list)
base by dir: 0→22, 1→34, 2→43 if skin 0x0e else 0, 3→33, 4→11, 5→33, 6→0, 7→34.
Walking (+0xcc) and dir ∈{0,2,4,6}: frame = base + walkFrame + 1 (walkFrame 0..9; skins 8..11 cycle 0..7).
Idle and frame==33: → 35 if +0xcd≠0 else 36.
Talking (+0x10b): t=(talkTicks/2)%6: skin 0x0e → 54+t; else if frame==36 or skin==3 → 43+t; else if frame==11 → 37+t.
Mirror (0x456e8c): dir<4 and skin≠0x0e → draw horizontally flipped (sheets are authored facing left/front-left).
NPC (+4): its FLC anim is drawn instead; while talking the FLC advances every second tick (0x4585d4).

## Scaling 0x4574ac
`scale = (room.scaleB(+0x58) − room.scaleA(+0x56)) / (y − scaleA)`; y==scaleA → 1; clamp scale<1 → 1.0.
(Globals 0x468ef8/efc/f00 = −1 → fixed-scale/no-room branches unused.) Rendered size = sprite size / scale, so a
character is full-size at y ≥ scaleB and shrinks toward scaleA. Recomputed after every move tick and SetX/SetY/SetRoom.

## Per-tick update 0x457ff0 (called for all 40 chars every 70 ms tick from 0x45c010)
1. +0x109=(+0x109+1)%6; if +0xcd: −1.
2. Turning handled as above (returns after, except stops moving if dir==target and pos==segEnd).
3. If pos≠segEnd: step = round(base/scale), base (px/tick):
   dir0: 5 (skin 0xe: 15 when walkFrame==9); dir4: 4 (same 0xe rule); else skin3 3, skin0xe 5 (25 at frame 9), skin0xf 8, others 10.
   skin 0xe: step=max(step,3).
   Move along the **major axis** of the segment (|dx|>|dy| → x, else y) by step toward segEnd; overshoot: clamp to segEnd if
   +0xce==0, else step back (it oscillates within tolerance). Minor axis linearly interpolated from segment start:
   `y = segStartY + |x−segStartX|·(segEndY−segStartY)/|segEndX−segStartX|` (integer div), resp. symmetric for y-major.
   walkFrame = (walkFrame+1) % 10 (skins 8..11: %8).
4. Segment reached if pos==segEnd, or (+0xce and |x−ex|<round(11/scale) and |y−ey|<round(6/scale)). Then:
   - queued leg (+0x2c≠−1): new segment pos→queued, clear queue, re-face (0x457430);
   - else if pos≠final dest: replan `0x456ed4(char, x, y, resetVisited=0, destY, destX)`;
   - else arrive: frame=−1, moving=0, +0xcd=2; if final dir (+0x13≠0xff, ≠dir) → Turn(+0x13), +0x13=0xff;
     if not busy(+0xf7): dir 2→turn to 3, dir 6→turn to 5 (idle 3/4 pose).
5. Rescale; talk tick 0x4585d4: +0x10c++, toggle; stop talking when talkTicks > duration, or duration==0 and speech
   enabled and voice finished (0x455ffc).

## Path planning
`WalkTo(char, x0, y0, reset, destY, destX)` 0x456ed4 (Goto op → 0x457040: if already at target → just Turn(dir); else
WalkTo(...,reset=1) and +0x13=dir):
- reset → clear visited[0..nWaypoints]; always clear tried[]; dest=(destX,destY); +0xce=0.
- `TryDirect(start, dest, force)` 0x457c64:
  * If dx==0 or dy==0, or atan(|dx|/|dy|) ≤ 20°?—precisely: straight if axis-aligned, or angle-from-vertical ≤ 20° (const 0x457fd8),
    or angle-from-horizontal ≤ 10° (0x457fe4), or |dx|<6 and |dy|<6. Straight succeeds if both endpoints are inside room.walk
    polygon and the segment is inside (0x450c48), or force. Sets segment start=pos, end=dest, face, moving.
  * Else dog-leg: Ox=sign(dx)·round(|dy|·tan20°), Oy=sign(dy)·round(|dx|·tan10°).
    Corner A=(destX−Ox, y0+Oy); if start→A and A→dest walkable → legs (start→A, queued A→dest).
    Else (if !force) corner B=(x0+Ox, destY−Oy) same test → legs; else fail. Forced: corner A used regardless.
- While TryDirect fails: pick waypoint 0x4579c0 among room waypoints (+0x14 list, ≤20) not visited/tried and not sharing
  x or y with start or dest, minimising `|start−wp| + |wp−dest| − (50 if wp→dest directly walkable 0x456648) + angleDiff/2`
  (angleDiff = |angle(start,wp) − angle(start,dest)| folded to ≤180). None → force direct to dest and stop.
  Else tried[wp]=1, +0xce=1, TryDirect(start, wp); success → visited[wp]=1 (and walk there; replanning continues on arrival).
- 0x456648 = same straight/dog-leg walkability test without side effects.

## Hero click-walk 0x45c70c(world, clearPending, x, y)
Ignores clicks within 14/scale (x) and 7/scale (y) of the hero. y' = y + max(2, round(…hero offset)) clamped ≤ 0x1a3; if (x,y')
inside walk polygon → WalkTo(x,y'); else if (x,y+2) and (x,y) inside → WalkTo(x,y). +0x13=0xff. clearPending → 0x45abfc.

## Drawing 0x456a18 & order 0x45c010
- Draw at (x − round(w/scale)/2, y − round(h/scale)) scaled 1/scale, mirror flag, skin 0xe bobs y −= round((+0x109/3)/scale).
- Foreground mask mode passed to blitter 0x44fe5c: room.fg(+0x5c) ≤0 → 0xff (no fg); else fgY1=+0x60, fgY2=+0x64:
  `if !(fgY1>0 && y>=fgY1) → 0; else if !(fgY2>0 && y>=fgY2) → 1; else 2` (0 = behind fg layer, 2 = in front; 1 = middle band;
  blit semantics: graphics agent).
- Frame 0x45c010: if hero.y < char2.y: erase chars in current room from index 39→0, else 0→39; update all 40 (0x457ff0);
  back anims + ambient (0x45e140); draw in the opposite order (so hero/char2 sorted by y; NPCs ordered by index only).
  Original uses save-under rects; a port can simply redraw bg+regions+anims+chars+fg each tick.

## Blocking moves
- StaticWalk op32 (a1=x,a2=y,a3=dir,a4=char) → 0x45e2cc(world,x,y,char,dir): hide cursor/tooltip, Goto, loop at 70 ms
  (music tick, redraw 0x45c010) until char stops; ESC/mouse click (0x4690b8/0x4690bc/ESC 0x470300) → skip: restore bg,
  teleport to (x,y), SetDir(dir). (ENGINE.md row "a3,a4 unused" is wrong.)
- StaticTurn op33 (a1=dir,a2=char) → 0x45e510: Turn and wait at 70 ms ticks until done; skip → SetDir.

## ChangeRoom 0x45a9b8(world, id) (op5 a2==0)
If id≠current: room=RoomById, hero.room=id. If new room pal(+0x54) ≠ old pal and not the 0x0e↔0x12 pair: erase chars (y-order),
and unless id∈[0x64f,0x654] clear game view to black (0x44f008). If **flag 0x7ff** set ("companion follows"): char2.roomId=id,
char2 placed at hero x,y,dir. Then load palette (0x45c274), start anim1/anim2 (0x45ba34/0x45ba70), draw bg+region images
(0x45b5b0), foreground (0x45c1fc). Chars elsewhere just keep their room id (op5 a2≠0 moves them silently).
Clicking char2 (region 0x801) only possible if char2.room==current and idle.

## NPCs
World ctor: NPC i (2..39): +0x10a = Data.dat0x25[i].hi (talk colour), +0xc = anim list[Data.dat0x25[i].lo] (Partial.dat idx
from 0x26; 2000 = none), +4=1, pos from 0x27. They never path-find; placed via ChangePos/ChangePosX/ChangeRoom, speak via Say.
