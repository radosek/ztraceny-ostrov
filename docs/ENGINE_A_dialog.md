# A. Speech, dialogs, subtitles, Delay, Meanwhile (from asm)

Input globals used inside all blocking loops (pointer var → storage):
left-click `[0x469734]→0x4690b8`, right-click `[0x4694f4]→0x4690bc`, key-down `[0x469648]→0x4690d4`,
key code `[0x4694cc]→0x470300` (u16), key char `[0x4698a4]→0x470302`, mouse-moved `[0x469624]→0x4690d0`.
`0x45f7a0` clears b8/bc/d4/d0. `0x441d40` = Application.ProcessMessages (fills them).
"SKIP" below = `left || right || (keydown && key==ESC(0x1b))`.
Every blocking loop is paced by the global tick base `0x470304`: busy-wait until `timeGetTime()-base >= 70`, then `base += 70`
(`0x44f084` is called when time remains — idle/vsync helper). On exit `base = timeGetTime()`.
`0x459714` (music playlist advance) is called once per tick in every loop.

## Sound flags
- `0x468ed0` = DirectSound initialised. `0x468edc` = speech channel enabled (`[0x4695dc]` points to it).
- Speech enabled ⇔ speech volume > 0: `SetSpeechVolume 0x456018(v)` sets mixer channel 2 volume then calls
  `0x455f90` (edc=1) or `0x455fc0` (edc=0, also stops speech). So **Main.cfg[14]==0 ⇒ no voice, text always shown**.
- Mixer `0x4701e8`, channel 2 = speech (streamed TAUDStream from Speech.dat entry): `PlaySpeech 0x455ecc(archive, idx)`
  (only if ed0&&edc; stops ch2, creates stream at archive offset, starts). `StopSpeech 0x455f6c`. `SpeechPlaying 0x455ffc`
  (= mixer 0x4561dc(ch2)). Ambient `0x456494` = ch3 playing.
- Ducking: `0x4597e0` (before a Say/Dialog): if speechVol(Main.cfg[14])>0 → musicVol = cfg[12]*0.3, fxVol = cfg[13]*0.3
  (constant 0.3 as float80 at 0x45982c; 0x402958 = Round). `0x459838` (after) restores cfg[12]/cfg[13].

## Say(line, keepScene) – 0x45da88 (World, edx=line, cl=keepScene; interpreter passes 0)
```
wait CanDraw; lock; cursor.hide(0x450544 false); hoverTip(0x46989c).hide;
if !keepScene { redraw characters 0x45c154(world,true); 0x4690e0=0 }  unlock
duck(0x4597e0); SayLine(line) (0x45d908); done=false; base=now
loop until done || appTerminated:
   music tick; if CanDraw { lock; speechTip(0x4694f0).hide;
        if !keepScene  world.drawFrame 0x45c010   // full scene update (chars animate, talk anim)
        else           char[lineTable[line].speaker].talkTick(true) 0x4585d4  // NB raw speaker, no default
        speechTip.show; unlock+flip }
   if no character talking (0x45dd54: any char.+0x10b) → done
   repeat { clearInput; ProcessMessages; if SKIP { StopAllTalk 0x45dd20 (StopSpeech + every char.+0x10b=0); done } }
   until tick elapsed || done ; base+=70
lock; speechTip.hide; char[raw speaker].endTalk 0x456948 (restore NPC anim bg); unlock; unduck 0x459838; clearInput; base=now
```

## SayLine(line) – 0x45d908
```
rec = lineTable[line] (Data.dat 0x23, 6 bytes: i16 speaker, i16 text, i16 speech)
text   = rec.text   != 0 ? rec.text   : line
speech = rec.speech != 0 ? rec.speech : line;  if !speechEnabled(0x468edc) speech = 0
spk    = rec.speaker!= 0 ? rec.speaker: defaultSpeaker(0x469144, set by op ChangeStatic)
if !subtitles(Main.cfg[15]=0x47031f) && speechEnabled && speech!=0:
     char[spk].Say(text=None, alt=None, altSpeech=0x1f9, speech, Speech.dat)      // voice only
else char[spk].Say(strings22[text], strings22[0x1f9], 0x1f9, speech, Speech.dat)
```
**0x1f9 (505, "Glo glo.")** = substitute line used when the speaker's skin `+0x10 == 0x0c` (hero under water):
both text and speech index are replaced by 505.

## TCharacter.Say(text, altText, altSpeech, speech, archive) – 0x4577d0
```
if text empty && speech<=0 → return
if !hidden(+4) { if dir(+0x11)==2 Turn(3); if dir==6 Turn(5); StopWalk 0x457c34 }
s = text; if skin(+0x10)==0x0c { if text!=empty s=altText; if speech!=0 speech=altSpeech }
talking(+0x10b)=1; talkTicks(+0x10c)=0
assert speechTip not visible ("Speech tool tip must be already hidden…", 0x457978)
speechTip.setText(s)
if s nonempty:
   h = spriteHeight(0x456e54)                       // current frame height
   pos = (x(+0x14) - tipWidthPx/2,  y(+0x18) - h - tipHeight - 17)
   speechTip.setPos(pos) (clamped, see renderer); speechTip.color = talkColor(+0x10a)
   duration(+0x110) = max(len(s), 15) ticks        // 1 tick=70 ms per character, min 1.05 s
if speech>0 { PlaySpeech(archive, speech); duration=0 }   // voice: ends when sample ends
if npcAnim(+0xc) start talk anim 0x44fb54(anim,0)
```
Per-tick talk update `0x4585d4(char, onlySpeaker)` (called from draw loop): if talking: talkTicks++, toggle +0x114;
for hidden NPC with anim (+0xc) and toggle==1 (and !onlySpeaker… note: branch taken when dl==0 only): draw next frame of
NPC anim (talk anim advances every 2nd tick); stop talking (0x457c20 clears +0x10b) when
`(speechEnabled && !SpeechPlaying && duration==0) || (duration!=0 && talkTicks > duration)`.
Hero/2nd-char talk frames are chosen in sprite-frame selector 0x456ba4 (frames 0x25..0x3b cycle `(talkTicks/2)%6`; covered by task B).
`0x456948` endTalk: for hidden NPC with anim, re-blit its static frame from background copy (0x44fc40).

## Dialog(n) – 0x45dd90
```
prep exactly as Say (hide cursor/tip, redraw chars, 0x4690e0=0), duck
StartDialog 0x45da4c: world.+0x134=n; +0x138=dialogTable[n].first-1; Advance 0x45da10
  Advance: if +0x134>0 { +0x138++; if +0x138 > dialogTable[n].last → StopDialog 0x45da6c (+0x134=0; StopAllTalk)
                          else SayLine(+0x138) }
loop until done:
  tick: lock; speechTip.hide; if no char talking { char[lineTable[cur].speaker(raw)].endTalk; Advance; done = !dialogActive }
        drawFrame 0x45c010; speechTip.show; unlock
  input: if SKIP { lock; tip.hide; spk.stopTalk(0x457c20); spk.endTalk; unlock; Advance; if !active done=true }
         if ESC { StopDialog; done }            // click = next line, ESC = abort whole dialog
end: tip.hide, unduck, clearInput, base=now
```
Note dialog lines skip line-table "first-1" semantics: lines first..last inclusive.
Main-loop also handles a dialog left running (0x4630a4: if dialog active, click/ESC→0x45da6c).

## Delay(ticks, mode) – 0x45f81c (interpreter op 37: `Delay(a1, mode=0)`)
```
prep (hide cursor/tip, redraw chars); done=false; count=0
mode1: done immediately if room.anim1==0 or anim1 not running / already at end (anim.+0x90 frame, +0xb, 0x44f4b0 finished)
mode2: same for anim2 (+0x124)
loop until done: tick { drawFrame 0x45c010 }, input: mode0 && SKIP → done
   after each tick: count++; mode1/2: done when back-anim wrapped (finished && frame==1, or frame==0)
   mode0: done when count >= ticks
```
So Delay a1 = a1 × 70 ms, skippable by click/ESC. Modes 1/2 = "wait until background anim 1/2 completes"
(used by ChangeBackAnimX a3>2).

## Meanwhile(textIdx, unused) – 0x45fbe0 (op 41: text = strings22[a1])
```
fadeSpeed [0x4697f4]=70; hide cursor/tip; save bottom panel (0x459860, rows 420..479 to 0x47032c) ; fade out (0x44ef8c)
lock; captionTip(0x46989c).setText(text); pos=((640 - wPx)/2, (420 - h)/2); show; unlock
0x44f084; ticks = max(2*len(text), 30)  (1 tick=70 ms)
wait ticks, SKIP ends early
restore panel (0x459910); fadeSpeed=40; clearInput; base=now
```
Caption uses the hover-label tooltip object (0x46989c) with its current colour.

## Text renderer (TToolTip, 0x451610–0x451d7c)
Object: +4 text(AnsiString), +8 colour index, +9 visible, +0xc x, +0x10 y, +0x14 saved-background buffer, +0x18 lines, +0x1c max cols.
- Glyph map init 0x451610: table 0x468cb1[0x80..0xff] → `'_' + i` where i = position of the cp1250 char in
  0x451688 ("ÁÄČĎÉĚÍĽĹŇÓÔÖŔŘ…" then lowercase at 0x45169d, 47 chars), unknown → '_' glyph; overrides: 0x84→2,0x91..0x93→7,0xb5→7,
  0x94,0x95,0xbe→2, 0xa1→0. `glyphIndex 0x4516b8(c)`: c<0x20 → 0, c<0x80 → c−0x20, else table[c].
- Font = Data.dat[0x15], 0x5ef6 bytes = 143 glyphs × 0xaa (10×17 bytes, value 0 transparent, 1 ink, 2 outline). Draw 0x4516e8:
  ink = palette16[colour], outline = palette16[0] (0x46c898 = current room palette converted to 16-bit, 256 entries, set 0x44ed31).
- Layout 0x451834: `"`→0xff and `'`→0xfe (protect), WrapText(text, "\r", [#0..' '], maxCol=[0x468ca0]=40), restore quotes;
  lines = 1 + count('\r'); maxCols = longest line. Width px = maxCols*9+1 (0x451b98 returns bytes = 2×), height = lines*17 (0x451b8c).
- Line draw 0x45176c: char k at x+k*9.
- Clamp 0x4517c4: x,y ≥ 0; x+w ≤ 639 → x=639−w; y+h ≤ 480 → y=480−h (bubble may overlap panel).
- show 0x451d7c: save background under rect, draw lines (0x451a74). hide 0x451ba4/0x451c18: restore background.
- Instances: speech bubble `[0x4694f0]`, hover label/caption `[0x46989c]` (label at mouse+(10,10)).
