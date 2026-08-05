# normfix browser playground

The playground is a deliberately simple, old-school workbench: plain
HTML/CSS/TypeScript, a file list, text editor, and formatted/diagnostic/diff
views. Pinned Vite 8.1.5 builds the static site, while WebAssembly reuses the
same conservative native C action crate as the desktop CLI. Source files pass
directly from JavaScript to WebAssembly and remain inside the browser tab.
There is no server-side formatter, upload endpoint, analytics dependency, or
application runtime.

## Requirements

- Node.js 20.19+ or 22.12+
- Rust 1.97.1 with the browser compilation target
- `wasm-bindgen-cli` 0.2.126
- Clang with WebAssembly support (used to compile the embedded C grammar)
- a POSIX shell for the checked-in build scripts

Windows visitors can use the deployed playground in a modern browser without
installing anything. Windows development and local builds should run inside
WSL; the scripts do not claim native PowerShell support.

Install the two Rust additions once:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
```

`wasm32-unknown-unknown` is Rust's internal browser compilation target name,
not a public normfix release archive name.

Install the pinned frontend dependency from the lockfile:

```sh
npm ci
```

## Develop and build

Start the local Vite server:

```sh
npm run dev
```

The command rebuilds the WASM bindings before starting the site at
<http://127.0.0.1:5173>.

Create and inspect the exact production bundle:

```sh
npm run build
npm run preview
```

Vite writes the deployable site to `web/dist/`. Both `web/pkg/` and `web/dist/`
are generated and intentionally ignored; CI and Vercel rebuild them instead of
trusting checked-in binary blobs.

## Browser contract

`normfix-wasm` exports `formatProject(requestJson)`. The request schema is:

```json
{
  "files": [
    { "path": "src/main.c", "source": "int main(void)\n{\n}\n" }
  ]
}
```

The versioned response contains each formatted source, accepted safe fixes,
remaining diagnostics with line and display column, function budgets, and a
unified diff. A malformed C file fails independently so other selected files
can still be inspected. Requests are bounded to 128 files, 1 MiB per file and
4 MiB in total to keep browser memory use predictable. Paths must be canonical,
portable relative paths of at most 240 UTF-8 bytes so duplicates and downloaded
tar entries cannot acquire platform-dependent meanings. Any non-convergent
formatter result is discarded rather than exposed as a usable partial edit.

## Why the browser scope is smaller

The runtime bundle contains no filesystem, environment, subprocess, Git, or
network adapter. This boundary makes the privacy claim directly auditable and
keeps the WASM operation deterministic.

Consequently, the playground provides native safe C formatting, structural
diagnostics, and function budgets. Use the desktop CLI for official 42 header
identity/timestamps, project-wide header-guard proofs, the official Norminette
oracle, compiler/analyzer checks, Makefile updates, allowed-function policy,
Git-scoped runs, backups, and undo.

## Vercel

The repository-root `vercel.json` is monorepo-safe: it installs only the
`web/` package, runs its dedicated Vercel build, and publishes only `web/dist/`.
Keep the Vercel project Root Directory at the repository root so these paths
remain valid.

Vercel's standard Node build image does not guarantee the Rust toolchain. The
dedicated build script therefore ensures Clang is present and installs the
pinned minimal Rust toolchain, browser target, and matching binding generator
into `.vercel-rust/` before the Vite build. This happens only at build time;
none of those tools ship to the browser.

No environment variable, secret, function, rewrite, or backend service is
required. The configured content-security policy only allows same-origin
scripts, styles, WASM, and fetches.

To publish, import the repository into Vercel and leave the project Root
Directory at the repository root. The checked-in `vercel.json` supplies the
Vite framework, install command, build command, output directory, cache
headers, and browser security headers. Each connected-branch deployment then
builds a preview; promoting the chosen deployment publishes the production
site. No separate API or server project is needed.
