// Ztracený Ostrov browser player: takes the visitor's own copy of the game and
// runs it on the Rust engine (ostrov-core) compiled to WebAssembly.
//
//   import { mount } from "/ztraceny-ostrov/player.js";
//   const player = mount(element, { lang: "cs", onQuit });
//   player.setLang("en"); player.eject();
//
// The bundle is self-contained: every path resolves relative to this file.
// Controls are plain HTML; the host page styles them (boxb uses 98.css).

import { db } from "./store.js";

const base = new URL(".", import.meta.url);
const at = (path) => new URL(path, base).href;
const W = 640;
const H = 480;
const RATE = 22050;
/** Game files the engine reads, by lower-case name. */
const NEEDED = ["anim.dat", "backgnd.dat", "partial.dat", "data.dat", "music.dat", "fx.dat", "fx2.dat", "speech.dat"];
const VIDEOS = ["logo.avi", "titulky.avi"];
const SAVE = /^(auto|game[0-7])\.sav$/i;

const TEXT = {
  cs: {
    insert: "Vlož disk",
    intro: "Vyber složku se hrou Ztracený Ostrov (s Data.dat, Anim.dat a dalšími soubory .dat a .avi), nebo ty soubory. Můžeš je sem i přetáhnout.",
    ready: "Disk je vložený.",
    missing: (names) => `Chybí: ${names.join(", ")}.`,
    play: "Hrát",
    browse: "Procházet…",
    folder: "Vybrat složku…",
    storing: "Ukládám hru do prohlížeče…",
    loading: "Nahrávám hru…",
    notGame: "Tohle nevypadá jako Ztracený Ostrov: chybí Data.dat.",
    backup: "Zálohovat pozice",
    restore: "Obnovit pozice",
    eject: "Vysunout disk",
    badBackup: "Tohle není záloha pozic ze Ztraceného Ostrova.",
    restored: (n) => `Obnoveno pozic: ${n}.`,
    status: ["Hra zůstává jen ve tvém prohlížeči.", "Ovladač: páčka = kurzor, A = klik, B = pravý klik"],
  },
  en: {
    insert: "Insert disc",
    intro: "Pick the Ztracený Ostrov game folder (with Data.dat, Anim.dat and the other .dat and .avi files), or those files. You can also drop them here.",
    ready: "Disc inserted.",
    missing: (names) => `Missing: ${names.join(", ")}.`,
    play: "Play",
    browse: "Browse…",
    folder: "Choose folder…",
    storing: "Storing the game in your browser…",
    loading: "Loading the game…",
    notGame: "That doesn't look like Ztracený Ostrov: there's no Data.dat.",
    backup: "Back up saves",
    restore: "Restore saves",
    eject: "Eject disc",
    badBackup: "That isn't a Ztracený Ostrov save backup.",
    restored: (n) => `Saves restored: ${n}.`,
    status: ["The game stays in your browser.", "Controller: stick = pointer, A = click, B = right click"],
  },
};

const STYLE = `
.zo { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.zo-tray { flex: 1; display: grid; grid-template-columns: 120px minmax(0, 1fr); gap: 16px; align-items: start; align-content: start; padding: 16px; }
.zo-tray p { margin: 0 0 10px; max-width: 56ch; }
.zo-tray h4 { margin: 0 0 6px; font-size: 16px; }
.zo-row { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.zo-error { color: #a00000; margin-top: 8px; }
.zo-disc { width: 120px; max-width: 100%; aspect-ratio: 1; border-radius: 50%;
  background: radial-gradient(circle, #c0c0c0 0 10%, #e0e0e0 10.5% 13%, transparent 13.5%),
    conic-gradient(from 20deg, #e3f3ef, #9fd3c7, #f6e7b0, #4f9e8c, #c2e3d0, #e3f3ef);
  box-shadow: 0 0 0 1px #808080; }
.zo.is-loading .zo-disc { animation: zo-spin 1.4s linear infinite; }
@keyframes zo-spin { to { transform: rotate(1turn); } }
.zo.is-over .zo-tray { outline: 2px dashed #000080; outline-offset: -6px; }
.zo-bar { display: flex; flex-wrap: wrap; gap: 4px 12px; align-items: center; padding: 4px 6px; }
.zo-stage { flex: 1; min-height: 240px; background: #000; position: relative; }
.zo-stage canvas { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain;
  image-rendering: pixelated; cursor: none; touch-action: none; outline: none; display: block; }
.zo-crt::after { content: ""; position: absolute; inset: 0; pointer-events: none;
  background: repeating-linear-gradient(to bottom, rgba(0,0,0,0) 0 1px, rgba(0,0,0,.28) 2px 3px);
  box-shadow: inset 0 0 120px rgba(0,0,0,.55); }
[hidden] { display: none !important; }
@media (max-width: 560px) { .zo-tray { grid-template-columns: 1fr; } }
@media (prefers-reduced-motion: reduce) { .zo-disc { animation: none !important; } }
`;

const MARKUP = `
<div class="zo">
  <div class="zo-tray" data-ref="tray" aria-live="polite">
    <div class="zo-disc" aria-hidden="true"></div>
    <div>
      <h4 data-t="insert"></h4>
      <p data-ref="trayText"></p>
      <div class="zo-row">
        <button type="button" data-ref="play" class="default" hidden data-t="play"></button>
        <button type="button" data-ref="pickFolder" data-t="folder"></button>
        <button type="button" data-ref="pickFiles" data-t="browse"></button>
      </div>
      <p class="zo-error" data-ref="error" hidden></p>
      <input type="file" data-ref="files" multiple hidden accept=".dat,.avi,.cfg,.sav,.DAT,.AVI,.CFG,.SAV">
      <input type="file" data-ref="folder" webkitdirectory hidden>
    </div>
  </div>
  <div class="zo-bar" data-ref="bar" hidden>
    <button type="button" data-ref="backup" data-t="backup"></button>
    <button type="button" data-ref="restore" data-t="restore"></button>
    <button type="button" data-ref="eject" data-t="eject"></button>
    <input type="file" data-ref="restoreFile" accept=".json" hidden>
  </div>
  <div class="zo-stage" data-ref="stage" hidden>
    <canvas data-ref="canvas" width="${W}" height="${H}" tabindex="0"></canvas>
  </div>
  <div class="status-bar" data-ref="status"></div>
</div>`;

let engine;
/** Loads the WebAssembly engine once per page. */
function loadEngine() {
  engine ??= (async () => {
    const mod = await import(at("lib/ostrov_web.js"));
    await mod.default({ module_or_path: at("lib/ostrov_web_bg.wasm") });
    return mod;
  })();
  return engine;
}

const toBase64 = (bytes) => {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
};
const fromBase64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

/** Windows virtual key for a KeyboardEvent, or 0 if the game doesn't read it. */
function vk(e) {
  const named = { Escape: 0x1b, Enter: 0x0d, Backspace: 8, ArrowLeft: 0x25, ArrowRight: 0x27, ArrowUp: 0x26, ArrowDown: 0x28, Home: 0x24, End: 0x23, Delete: 0x2e, " ": 0x20, F1: 0x70, F5: 0x74, F9: 0x78, F12: 0x7b };
  if (named[e.key]) return named[e.key];
  if (/^Key[A-Z]$/.test(e.code)) return e.code.charCodeAt(3);
  if (/^Digit[0-9]$/.test(e.code)) return e.code.charCodeAt(5);
  return 0;
}

export function mount(root, { lang = "cs", onQuit } = {}) {
  if (!document.getElementById("zo-style")) {
    document.head.append(Object.assign(document.createElement("style"), { id: "zo-style", textContent: STYLE }));
  }
  root.innerHTML = MARKUP;
  const el = root.querySelector(".zo");
  const ref = Object.fromEntries([...el.querySelectorAll("[data-ref]")].map((n) => [n.dataset.ref, n]));
  const ctx = ref.canvas.getContext("2d");
  const image = ctx.createImageData(W, H);

  let text = TEXT[lang] ?? TEXT.cs;
  let missing = NEEDED;
  let busy = "";
  let game = null;
  let clock = 0;
  let last = 0;
  let raf = 0;
  let audio = null;
  let audioAt = 0;
  let mouse = { x: W / 2, y: H / 2 };

  function render() {
    for (const n of el.querySelectorAll("[data-t]")) n.textContent = text[n.dataset.t];
    ref.trayText.textContent =
      busy ? text[busy] : missing.length === 0 ? text.ready : missing.includes("data.dat") ? text.intro : text.missing(missing);
    ref.status.replaceChildren(
      ...text.status.map((s) => Object.assign(document.createElement("p"), { className: "status-bar-field", textContent: s })),
    );
  }

  function showError(message) {
    ref.error.textContent = message;
    ref.error.hidden = !message;
  }

  // --- Inserting the game --------------------------------------------------

  async function insert(fileList) {
    showError("");
    busy = "storing";
    el.classList.add("is-loading");
    render();
    try {
      const d = await db;
      let sawData = false;
      for (const file of fileList) {
        const name = file.name.toLowerCase();
        const inSaveDir = /(^|\/)savegame\//i.test(file.webkitRelativePath ?? "");
        if (NEEDED.includes(name) || VIDEOS.includes(name)) {
          await d.put("files", file, name);
          sawData ||= name === "data.dat";
        } else if (name === "main.cfg" && file.size < 4096) {
          if ((await d.get("kv", "cfg")) == null) await d.put("kv", new Uint8Array(await file.arrayBuffer()), "cfg");
        } else if (SAVE.test(name) && (inSaveDir || fileList.length < 20)) {
          const key = `SaveGame/${name === "auto.sav" ? "auto" : `Game${name[4]}`}.sav`;
          if ((await d.get("saves", key)) == null) await d.put("saves", new Uint8Array(await file.arrayBuffer()), key);
        }
      }
      busy = "";
      await refresh();
      if (!sawData && missing.includes("data.dat")) showError(text.notGame);
    } catch (e) {
      console.error(e);
      showError(String(e.message ?? e));
    } finally {
      busy = "";
      el.classList.remove("is-loading");
      render();
    }
  }

  async function refresh() {
    const keys = await (await db).getAllKeys("files");
    missing = NEEDED.filter((n) => !keys.includes(n));
    ref.play.hidden = missing.length > 0;
    render();
  }

  ref.pickFiles.addEventListener("click", () => ref.files.click());
  ref.pickFolder.addEventListener("click", () => ref.folder.click());
  ref.files.addEventListener("change", (e) => insert([...e.target.files]));
  ref.folder.addEventListener("change", (e) => insert([...e.target.files]));
  ref.tray.addEventListener("dragover", (e) => {
    e.preventDefault();
    el.classList.add("is-over");
  });
  ref.tray.addEventListener("dragleave", () => el.classList.remove("is-over"));
  ref.tray.addEventListener("drop", async (e) => {
    e.preventDefault();
    el.classList.remove("is-over");
    const items = [...e.dataTransfer.items].map((i) => i.webkitGetAsEntry?.()).filter(Boolean);
    insert(items.length ? await filesOf(items) : [...e.dataTransfer.files]);
  });

  /** Files in dropped entries; folders are walked (SaveGame one level down). */
  async function filesOf(entries, prefix = "") {
    const out = [];
    for (const entry of entries) {
      if (entry.isFile) {
        const f = await new Promise((ok, err) => entry.file(ok, err));
        Object.defineProperty(f, "webkitRelativePath", { value: prefix + f.name });
        out.push(f);
      } else if (entry.isDirectory && prefix.split("/").length < 3) {
        const reader = entry.createReader();
        const children = [];
        for (let batch; (batch = await new Promise((ok, err) => reader.readEntries(ok, err))).length; ) children.push(...batch);
        out.push(...(await filesOf(children, `${prefix}${entry.name}/`)));
      }
    }
    return out;
  }

  // --- Audio ---------------------------------------------------------------

  function startAudio() {
    const AC = window.AudioContext ?? window.webkitAudioContext;
    if (!AC) return;
    audio ??= new AC({ latencyHint: "interactive" });
    audio.resume().catch(() => {});
  }

  function playAudio(samples) {
    if (!audio || audio.state !== "running" || samples.length === 0) return;
    const now = audio.currentTime;
    if (audioAt < now + 0.02) audioAt = now + 0.05;
    if (audioAt > now + 0.3) return; // too far ahead (tab was throttled): drop
    const buf = audio.createBuffer(1, samples.length, RATE);
    buf.copyToChannel(samples, 0);
    const src = audio.createBufferSource();
    src.buffer = buf;
    src.connect(audio.destination);
    src.start(audioAt);
    audioAt += samples.length / RATE;
  }

  // --- Playing -------------------------------------------------------------

  async function persist() {
    if (!game) return;
    const writes = game.takeWrites();
    if (writes.length === 0) return;
    const d = await db;
    for (const [path, bytes] of writes) {
      if (path.toLowerCase() === "main.cfg") await d.put("kv", bytes, "cfg");
      else await d.put("saves", bytes, path);
    }
  }

  async function play() {
    showError("");
    startAudio();
    busy = "loading";
    el.classList.add("is-loading");
    ref.play.disabled = true;
    render();
    try {
      const mod = await loadEngine();
      const d = await db;
      const files = {};
      for (const name of [...NEEDED, ...VIDEOS]) {
        const blob = await d.get("files", name);
        if (blob) files[name] = new Uint8Array(await blob.arrayBuffer());
      }
      const saves = {};
      for (const key of await d.getAllKeys("saves")) saves[key] = new Uint8Array(await d.get("saves", key));
      const cfg = (await d.get("kv", "cfg")) ?? null;
      const n = new Date();
      const seed = ((n.getUTCHours() * 60 + n.getUTCMinutes()) * 60 + n.getUTCSeconds()) * 1000 + n.getUTCMilliseconds();
      game = new mod.Game(files, cfg && new Uint8Array(cfg), saves, seed);
    } finally {
      busy = "";
      el.classList.remove("is-loading");
      ref.play.disabled = false;
      render();
    }
    clock = 0;
    last = performance.now();
    ref.tray.hidden = true;
    ref.stage.hidden = false;
    ref.bar.hidden = false;
    ref.canvas.focus();
    raf = requestAnimationFrame(loop);
  }

  let lastScaling = -1;
  function loop(now) {
    raf = requestAnimationFrame(loop);
    if (!game) return;
    clock += Math.min(now - last, 100);
    last = now;
    pollPad();
    game.tick(clock);
    const warp = game.takeWarp();
    if (warp) mouse = { x: warp[0], y: warp[1] };
    if (game.frame(image.data)) ctx.putImageData(image, 0, 0);
    const mode = game.scaling();
    if (mode !== lastScaling) {
      lastScaling = mode;
      ref.canvas.style.imageRendering = mode >= 2 ? "auto" : "pixelated";
      ref.stage.classList.toggle("zo-crt", mode === 3);
    }
    playAudio(game.takeAudio());
    persist().catch(console.error);
    if (game.finished()) {
      stop();
      onQuit?.();
    }
  }

  /** Lets the game write its autosave and Main.cfg, then drops it. */
  function stop() {
    cancelAnimationFrame(raf);
    if (game) {
      if (!game.finished()) {
        game.quit();
        game.tick(clock + 1000);
      }
      persist().catch(console.error);
      game.free();
      game = null;
    }
    ref.stage.hidden = true;
    ref.tray.hidden = false;
    ref.bar.hidden = true;
  }

  function eject() {
    stop();
  }

  ref.play.addEventListener("click", () => play().catch((e) => (console.error(e), showError(String(e.message ?? e)))));
  ref.eject.addEventListener("click", eject);
  window.addEventListener("pagehide", stop);

  // --- Mouse, touch, keyboard ----------------------------------------------

  function toGame(e) {
    const r = ref.canvas.getBoundingClientRect();
    const s = Math.min(r.width / W, r.height / H);
    const x = (e.clientX - r.left - (r.width - W * s) / 2) / s;
    const y = (e.clientY - r.top - (r.height - H * s) / 2) / s;
    return { x: Math.max(0, Math.min(W - 1, Math.floor(x))), y: Math.max(0, Math.min(H - 1, Math.floor(y))) };
  }

  const c = ref.canvas;
  c.addEventListener("pointermove", (e) => {
    mouse = toGame(e);
    game?.mouseMove(mouse.x, mouse.y);
  });
  c.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 && e.button !== 2) return;
    e.preventDefault();
    c.focus();
    startAudio();
    c.setPointerCapture?.(e.pointerId);
    mouse = toGame(e);
    game?.mouseMove(mouse.x, mouse.y);
    if (e.pointerType === "touch") {
      // Touch: tap = left click, hold half a second = right click (inventory / drop item).
      clearTimeout(longPress);
      const at = mouse;
      longPress = setTimeout(() => {
        longPress = 0;
        game?.mouseDown(at.x, at.y, true);
        game?.mouseUp(true);
      }, 500);
      return;
    }
    game?.mouseDown(mouse.x, mouse.y, e.button === 2);
  });
  let longPress = 0;
  c.addEventListener("pointerup", (e) => {
    if (e.pointerType !== "touch" || !longPress) return;
    clearTimeout(longPress);
    longPress = 0;
    game?.mouseDown(mouse.x, mouse.y, false);
    game?.mouseUp(false);
  });
  c.addEventListener("pointerup", (e) => {
    if (e.button !== 0 && e.button !== 2) return;
    game?.mouseUp(e.button === 2);
  });
  c.addEventListener("contextmenu", (e) => e.preventDefault());
  c.addEventListener("keydown", (e) => {
    if (!game || e.ctrlKey || e.metaKey || e.altKey) return;
    startAudio();
    const code = vk(e);
    if (code) game.keyDown(code);
    if (e.key.length === 1) game.text(e.key);
    if (e.key === " ") game.highlight(true);
    if (code || e.key.length === 1) e.preventDefault();
  });
  c.addEventListener("keyup", (e) => {
    if (e.key === " ") game?.highlight(false);
  });

  // --- Saves ---------------------------------------------------------------

  ref.backup.addEventListener("click", async () => {
    await persist();
    const d = await db;
    const saves = {};
    for (const key of await d.getAllKeys("saves")) saves[key] = toBase64(new Uint8Array(await d.get("saves", key)));
    const cfg = await d.get("kv", "cfg");
    const data = { game: "ztraceny-ostrov", saves, ...(cfg ? { cfg: toBase64(new Uint8Array(cfg)) } : {}) };
    const blob = new Blob([JSON.stringify(data, null, 1)], { type: "application/json" });
    const a = Object.assign(document.createElement("a"), { href: URL.createObjectURL(blob), download: "ztraceny-ostrov-saves.json" });
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  });
  ref.restore.addEventListener("click", () => ref.restoreFile.click());
  ref.restoreFile.addEventListener("change", async (e) => {
    const file = e.target.files[0];
    e.target.value = "";
    if (!file) return;
    try {
      const data = JSON.parse(await file.text());
      if (data.game !== "ztraceny-ostrov" || typeof data.saves !== "object") return showError(text.badBackup);
      const entries = Object.entries(data.saves).filter(([k, v]) => /^SaveGame\/(auto|Game[0-7])\.sav$/.test(k) && typeof v === "string");
      const d = await db;
      await d.clear("saves");
      for (const [k, v] of entries) await d.put("saves", fromBase64(v), k);
      if (typeof data.cfg === "string") await d.put("kv", fromBase64(data.cfg), "cfg");
      showError("");
      // A running game has its saves in memory: restart it on the restored ones.
      if (game) {
        cancelAnimationFrame(raf);
        game.free();
        game = null;
        await play();
      }
      alert(text.restored(entries.length));
    } catch {
      showError(text.badBackup);
    }
  });

  // --- Controller ----------------------------------------------------------

  const pad = { a: false, b: false, start: false, y: false, last: performance.now() };
  function pollPad() {
    const now = performance.now();
    const dt = Math.min((now - pad.last) / 1000, 0.1);
    pad.last = now;
    const gp = [...(navigator.getGamepads?.() ?? [])].find(Boolean);
    if (!gp || !game) return;
    const [sx, sy] = [gp.axes[0] ?? 0, gp.axes[1] ?? 0];
    if (Math.hypot(sx, sy) > 0.18) {
      mouse = {
        x: Math.max(0, Math.min(W - 1, Math.round(mouse.x + sx * 500 * dt))),
        y: Math.max(0, Math.min(H - 1, Math.round(mouse.y + sy * 500 * dt))),
      };
      game.mouseMove(mouse.x, mouse.y);
    }
    const edge = (key, pressed, down, up) => {
      if (pressed !== pad[key]) (pad[key] = pressed) ? down() : up?.();
    };
    edge("a", Boolean(gp.buttons[0]?.pressed || gp.buttons[7]?.pressed), () => (startAudio(), game.mouseDown(mouse.x, mouse.y, false)), () => game.mouseUp(false));
    edge("b", Boolean(gp.buttons[1]?.pressed || gp.buttons[6]?.pressed), () => game.mouseDown(mouse.x, mouse.y, true), () => game.mouseUp(true));
    edge("start", Boolean(gp.buttons[9]?.pressed), () => game.keyDown(0x1b));
    edge("y", Boolean(gp.buttons[3]?.pressed), () => game.keyDown(0x49));
    edge("x", Boolean(gp.buttons[2]?.pressed), () => game.keyDown(0x48));
    edge("select", Boolean(gp.buttons[8]?.pressed), () => game.keyDown(0x70));
    // Bumpers and d-pad left/right jump between hotspots; d-pad up shows them all.
    edge("next", Boolean(gp.buttons[5]?.pressed || gp.buttons[15]?.pressed), () => game.cycleHotspot(1));
    edge("prev", Boolean(gp.buttons[4]?.pressed || gp.buttons[14]?.pressed), () => game.cycleHotspot(-1));
    edge("up", Boolean(gp.buttons[12]?.pressed), () => game.highlight(true), () => game.highlight(false));
  }

  render();
  refresh().catch((e) => showError(String(e.message ?? e)));
  loadEngine().catch((e) => showError(String(e.message ?? e)));

  return {
    setLang(next) {
      text = TEXT[next] ?? TEXT.cs;
      render();
    },
    eject,
    /** Stores game files (File/Blob with a name), as the pickers do. */
    insert,
    destroy() {
      stop();
      window.removeEventListener("pagehide", stop);
      audio?.close();
      root.replaceChildren();
    },
  };
}
