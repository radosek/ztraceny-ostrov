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
| mouse | as in the original: left = act, right = inventory / drop held item |
| `I` | inventory |
| hold `Space` | outline every hotspot (F1: always / off) |
| `F5` / `F9` | quick save / quick load |
| `F12` | bug report: saves from before the last 10 actions, action log, screenshot → `Bugs/` |
| `H` | hint: outlines the next useful hotspot |
| `F1` | settings: volumes, subtitles, hotspots, fullscreen, scaling, centred cut-scenes, autosave on room change |
| double-click | run there |
| `Esc` | skip speech, animation or video |
| `F11`, `Cmd+F` (macOS), `Alt+Enter` | fullscreen |
| controller | stick = pointer, A = click, B = right click, Y = inventory, Start = Esc |

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
