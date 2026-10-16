#!/usr/bin/env bash
# Builds the browser player into web/dist: player.js (mount API) and lib/ (the
# engine as WebAssembly). The boxb.org site serves it at /ztraceny-ostrov/.
set -euo pipefail
cd "$(dirname "$0")"
# Prefer rustup's toolchain (it has the wasm32 target) over a Homebrew rustc.
[ -d "$HOME/.cargo/bin" ] && export PATH="$HOME/.cargo/bin:$PATH"

cargo build -p ostrov-web --target wasm32-unknown-unknown --release
rm -rf dist && mkdir -p dist/lib
wasm-bindgen --target web --out-dir dist/lib --no-typescript \
  ../target/wasm32-unknown-unknown/release/ostrov_web.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    -o dist/lib/ostrov_web_bg.wasm dist/lib/ostrov_web_bg.wasm
fi

npm ci --no-audit --no-fund
npx esbuild src/player.js --bundle --format=esm --minify --outfile=dist/player.js
echo "Player bundle in web/dist (served at /ztraceny-ostrov/ on the site)"
