# Ztracený Ostrov — native

A Rust port of the Czech adventure game *Ztracený Ostrov* (2000) for macOS, Windows, Linux and
the browser. The original is a Delphi/DirectX 6 program that no longer runs on current Windows;
this reimplements its engine from the shipped executable and runs the game's own data and
scripts unchanged.

No game data is included. Point it at your own copy.

## Run

```sh
cargo run --release -p ztraceny-ostrov -- "/path/to/Ztracený Ostrov"   # folder with Data.dat
```

Without an argument it uses the last folder, then the folder next to the binary, then asks.
Saves and settings live in the per-user data folder (`ztraceny-ostrov/`). The game autosaves on every room change and on quit; window size and fullscreen are remembered.

| Key | |
|---|---|
| mouse | left = act, right = inventory / put the held item away |
| double-click | run there |
| `I` | inventory: click an item to take it, click another to combine, move off the grid to use it |
| hold `Space` | outline every hotspot (F1: always / off) |
| `H` | hint: outlines the next useful hotspot |
| `F5` / `F9` | quick save / quick load (the game also autosaves on every room change and on quit) |
| `F1` | settings (below) |
| `F12` | bug report: saves from before the last 10 actions, action log and screenshot → `Bugs/` |
| `Esc` | skip speech, animation or video |
| `Cmd+F` / `Alt+Enter` / `F11` | fullscreen |

| Controller | |
|---|---|
| left stick | pointer |
| A / RT · B / LT | left / right click |
| LB / RB, d-pad ← → | jump to the previous / next hotspot |
| d-pad ↑ (hold) | show hotspots |
| X · Y | hint · inventory |
| Start · Select | skip (Esc) · settings |

Touch (browser): tap = click, hold = right click.

**F1 settings:** volumes, subtitles, hotspots, fullscreen, picture (sharp integer / sharp fill /
smooth / CRT / xBRZ), centred cut-scenes, widescreen side fill, smooth motion (walking is
interpolated between the game's 70 ms ticks), autosave on room change, inventory style (modern /
original), language, text size 1×/2×. Stored in `settings.cfg` next to the saves.

**Translations:** the game text can be replaced from `lang-<code>.txt` in the data folder
(`T<n>`/`I<n>`/`L<n>` lines, tab, text). Translations are not part of this repository.

## Layout

- `core/` — engine: archives, FLC/RLE/GIF/ADPCM/Cinepak decoding, script VM, characters,
  rendering, UI, saves. Deterministic: driven by a virtual clock.
- `native/` — desktop runner (winit, pixels/wgpu, rodio, gilrs).
- `web/` — browser player for boxb.org (`web/build.sh` → `web/dist`).
- `extract/` — dumps the archives to PNG/WAV.
- `tools/probe/` — `ddraw.dll` for capturing reference frames from the original under Wine.
- `docs/` — file formats and the reverse-engineered engine spec.

## Testing

```sh
cargo test -p ostrov-core
cargo run --release -p ostrov-core --example headless -- <game-dir> <out-dir> "t=20000,c=20400:288:286,t=26000"
cargo run --release -p ostrov-core --example autoplay -- <game-dir> 5000   # exploration bot
```

## Building releases

- macOS universal app: `./scripts/package-macos.sh`
- Linux AppImage: `./scripts/package-linux.sh`
- CI (`.github/workflows/release.yml`) builds all three plus the web bundle; tags `v*` publish a release.
