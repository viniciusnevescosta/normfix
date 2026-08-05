#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: Rust and Cargo are required" >&2
  exit 1
fi
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen-cli 0.2.126 is required" >&2
  echo "install it with: cargo install wasm-bindgen-cli --version 0.2.126 --locked" >&2
  exit 1
fi

cd "$project_dir"
cargo build --release --locked -p normfix-wasm --target wasm32-unknown-unknown
wasm-bindgen \
  "target/wasm32-unknown-unknown/release/normfix_wasm.wasm" \
  --out-dir "web/pkg" \
  --target web \
  --no-typescript

echo "Playground built in web/pkg"
echo "Run npm --prefix web run dev to start the Vite development server"
