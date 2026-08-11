#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(CDPATH='' cd -- "$script_dir/../.." && pwd)

wasm_clang=""
brew_clang=""
if command -v brew >/dev/null 2>&1; then
  brew_llvm=$(brew --prefix llvm 2>/dev/null || true)
  if [ -n "$brew_llvm" ]; then
    brew_clang="$brew_llvm/bin/clang"
  fi
fi

for candidate in \
  "${CC_wasm32_unknown_unknown:-}" \
  "${WASM_CLANG:-}" \
  "$brew_clang" \
  "/opt/homebrew/opt/llvm/bin/clang" \
  "/usr/local/opt/llvm/bin/clang" \
  "clang"
do
  [ -n "$candidate" ] || continue
  case "$candidate" in
    */*) resolved_clang="$candidate" ;;
    *) resolved_clang=$(command -v "$candidate" 2>/dev/null || true) ;;
  esac
  [ -x "$resolved_clang" ] || continue
  if printf 'int normfix_wasm_probe;\n' | \
    "$resolved_clang" --target=wasm32-unknown-unknown -x c -c -o /dev/null - \
      >/dev/null 2>&1
  then
    wasm_clang="$resolved_clang"
    break
  fi
done

if [ -z "$wasm_clang" ]; then
  echo "error: no Clang installation with the wasm32 target was found" >&2
  if [ "$(uname -s)" = "Darwin" ]; then
    echo "Apple/Swift clang normally omits WebAssembly; install upstream LLVM:" >&2
    echo "  brew install llvm" >&2
    echo "Then retry, or set WASM_CLANG=\"$(brew --prefix llvm 2>/dev/null || echo /opt/homebrew/opt/llvm)/bin/clang\"." >&2
  else
    echo "Install the complete LLVM/Clang package for your distribution, then retry." >&2
  fi
  exit 1
fi

CC_wasm32_unknown_unknown="$wasm_clang"
export CC_wasm32_unknown_unknown
# tree-sitter-language 0.1.7 declares its allocator's self-reference through
# an equivalent incomplete struct type. LLVM 22 diagnoses that upstream C as
# an error, while older Clang releases only warn. Keep the compatibility flag
# scoped to C dependencies compiled for this WASM target and preserve any
# flags supplied by the caller.
case " ${CFLAGS_wasm32_unknown_unknown:-} " in
  *" -Wno-error=incompatible-pointer-types "*) ;;
  *)
    CFLAGS_wasm32_unknown_unknown="${CFLAGS_wasm32_unknown_unknown:+${CFLAGS_wasm32_unknown_unknown} }-Wno-error=incompatible-pointer-types"
    ;;
esac
export CFLAGS_wasm32_unknown_unknown
wasm_llvm_ar="$(dirname "$wasm_clang")/llvm-ar"
if [ -x "$wasm_llvm_ar" ]; then
  AR_wasm32_unknown_unknown="$wasm_llvm_ar"
  export AR_wasm32_unknown_unknown
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: Rust and Cargo are required" >&2
  exit 1
fi
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen-cli 0.2.126 is required" >&2
  echo "install it with: cargo install wasm-bindgen-cli --version 0.2.126 --locked" >&2
  exit 1
fi

installed_bindgen=$(wasm-bindgen --version 2>/dev/null || true)
if [ "$installed_bindgen" != "wasm-bindgen 0.2.126" ]; then
  echo "error: wasm-bindgen-cli 0.2.126 is required; found ${installed_bindgen:-nothing}" >&2
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
