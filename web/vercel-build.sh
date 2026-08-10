#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH='' cd -- "$script_dir/.." && pwd)
rust_cache="$project_dir/.vercel-rust"

export CARGO_HOME="$rust_cache/cargo"
export RUSTUP_HOME="$rust_cache/rustup"
export PATH="$CARGO_HOME/bin:$PATH"

mkdir -p "$CARGO_HOME" "$RUSTUP_HOME"

if ! command -v clang >/dev/null 2>&1; then
  if ! command -v dnf >/dev/null 2>&1; then
    echo "error: clang with WebAssembly support is required" >&2
    exit 1
  fi
  dnf install clang --assumeyes
fi

if ! command -v rustup >/dev/null 2>&1; then
  installer="$rust_cache/rustup-init.sh"
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    https://sh.rustup.rs --output "$installer"
  sh "$installer" -y --profile minimal --default-toolchain 1.97.1
fi

rustup toolchain install 1.97.1 --profile minimal
rustup target add --toolchain 1.97.1 wasm32-unknown-unknown

CC_wasm32_unknown_unknown=$(command -v clang)
export CC_wasm32_unknown_unknown
if command -v llvm-ar >/dev/null 2>&1; then
  AR_wasm32_unknown_unknown=$(command -v llvm-ar)
  export AR_wasm32_unknown_unknown
fi

expected_bindgen="wasm-bindgen 0.2.126"
installed_bindgen=$(wasm-bindgen --version 2>/dev/null || true)
if [ "$installed_bindgen" != "$expected_bindgen" ]; then
  cargo +1.97.1 install wasm-bindgen-cli --version 0.2.126 --locked
fi

cd "$script_dir"
npm run build
