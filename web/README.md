# normfix browser playground

The playground is a deliberately simple, old-school workbench built with
HTML, CSS, TypeScript, Vite, Monaco, and WebAssembly. It formats in-memory C,
headers, Makefiles, and Markdown using the same conservative Rust crates as the
CLI. There is no formatting backend, upload endpoint, account, or analytics
runtime.

Desktop browsers get Monaco with line numbers, search, multiple cursors,
bracket matching, and language highlighting. Narrow or coarse-pointer devices
get a lightweight textarea because Monaco does not officially support mobile
browsers. Both paths expose the same formatter and keyboard shortcut.

## Requirements

- Node.js 20.19+ or 22.12+
- Rust 1.97.1 with `wasm32-unknown-unknown`
- `wasm-bindgen-cli` 0.2.126
- a Clang installation that can actually compile a `wasm32` target
- a POSIX shell for the checked-in build scripts

Windows visitors can use the deployed playground directly. Windows local
development should use [WSL](https://learn.microsoft.com/windows/wsl/install);
the scripts do not claim native PowerShell support.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
npm ci
```

`web/build.sh` probes candidate compilers instead of assuming that a command
named `clang` supports WebAssembly. On macOS it checks configured and Homebrew
LLVM paths and prints an actionable `brew install llvm` message if none works.

## Develop, test, and build

From the repository root:

```sh
npm run dev
npm run check --workspace normfix-playground
npm run build
npm run preview
```

The development and production commands regenerate the WASM bindings before
they serve or publish the site. `web/pkg/` and `web/dist/` are generated and
ignored, so a fresh CI or Vercel build never depends on a stale binary blob.

Vite emits the English route at `/` and crawlable localized entries at `/pt/`,
`/es/`, and `/fr/`. UI copy and browser-side validation live in `web/i18n.ts`.
See the [localization guide](../docs/LOCALIZATION.md) before adding a locale.

## Browser API and supported files

`normfix-wasm` exports `formatProject(requestJson)`. A request can include the
42 identity and a deterministic timestamp used by official-header generation:

```json
{
  "files": [
    { "path": "src/main.c", "source": "int main(void)\n{\n}\n" },
    { "path": "Makefile", "source": "NAME=app\n" },
    { "path": "README.md", "source": "# app\n" }
  ],
  "identity_email": "login@student.42.fr",
  "timestamp": "2026/08/10 12:00:00"
}
```

The response contains formatted source, accepted safe fixes, remaining native
diagnostics, function budgets, and unified diffs. Native diagnostic text stays
English until CLI diagnostic localization lands; all browser UI and validation
messages are localized.

The browser accepts `.c`, `.h`, `.md`, and files named `Makefile`. Requests are
bounded to 128 files, 1 MiB per file, and 4 MiB total. Paths must be NFC-normalized
portable relative paths no longer than 240 UTF-8 bytes and must also fit a
portable tar header. Case-insensitive path collisions are rejected before the
WASM call. Imported files must be valid UTF-8; a leading UTF-8 BOM is consumed.
Any non-convergent result is discarded instead of exposed as a usable edit.

## 42 identity and privacy boundary

The identity panel accepts 42 student addresses only. **Remember on this
device** is off by default, so the email stays in memory unless the visitor
explicitly enables persistence. When enabled, it is stored under
`normfix.identity.v1` in same-origin local storage and can be removed with
**Forget**. It is passed directly to WebAssembly to generate the official 42
header and is never sent to a server.

Source buffers likewise stay inside the browser process. The only external
browser request is an unauthenticated, no-referrer fetch of the repository's
public GitHub star count; a bundled fallback is shown if GitHub is unavailable.

## Why the browser scope is smaller

The WASM bundle intentionally has no filesystem, environment, subprocess, or
Git adapter. That makes the source privacy claim auditable and the operation
deterministic, but it also means the browser cannot run the official
[Norminette](https://github.com/42school/norminette), a C compiler, Git, Make,
or the CLI transaction/backup/undo system. The playground is a native formatter
preview, not an official evaluation.

## Deployment and security policy

The root `vercel.json` installs the workspaces, regenerates WASM, builds the
playground and VitePress documentation, and publishes `web/dist/`. Keep the
Vercel project Root Directory at the repository root.

Content security policies are split so the playground and `/docs/` never
receive competing CSP headers. Scripts remain same-origin and disallow inline
execution. Monaco requires same-origin/blob workers and injects editor styles,
so the playground permits `worker-src 'self' blob:` and `style-src 'unsafe-inline'`.
`connect-src` additionally permits only GitHub's public API for the star count.
The inline-style exception is intentionally limited to styles; it does not
weaken `script-src`.
