# Browser playground

The playground is a deliberately simple, old-school workbench backed by
`normfix-wasm` and a static Vite frontend: a file list, a text editor, and
formatted/diagnostic/diff views. It accepts in-memory `.c` and `.h` buffers and
returns native format proposals, English diagnostics, unified diffs, and
function budgets.

<p class="playground-launch">
  <a class="playground-launch-button" href="/" target="_self" rel="noopener">
    Open the playground &rarr;
  </a>
</p>

<style scoped>
.playground-launch {
  margin: 2rem 0;
}
.playground-launch-button {
  display: inline-block;
  border: 1px solid var(--vp-c-brand-1);
  border-radius: 20px;
  padding: 0.5rem 1.25rem;
  font-weight: 600;
  text-decoration: none;
  color: var(--vp-c-brand-1);
}
.playground-launch-button:hover {
  background-color: var(--vp-c-brand-soft);
}
</style>

The playground is served from the root of this site; the documentation you are
reading lives beneath `/docs/` on the same origin.

## Privacy

All source stays inside the browser process. The playground has no filesystem
access and no source-upload path: buffers pass directly from JavaScript to
WebAssembly and never leave the tab. There is no server-side formatter, upload
endpoint, analytics dependency, or application runtime.

The deployed site is published under a content security policy that allows no
external origin, no inline script, and no framing.

## What the browser cannot do

The browser sandbox cannot execute the external Norminette binary, a C
compiler, Git, or Make, and it has no transaction, undo, identity discovery, or
official-header update path. Its result is a convenient native formatter
preview, **not** an official evaluation.

| Capability | CLI | Playground |
|---|---|---|
| Safe C formatting | yes | yes |
| Structural diagnostics and function budgets | yes | yes |
| Unified diffs | yes | yes |
| Official Norminette oracle | yes | no |
| Strict compiler preflight and analyzer | yes | no |
| Official 42 header identity and timestamps | yes | no |
| Project-wide header-guard and allowed-function proofs | yes | no |
| Makefile updates | yes | no |
| Git-scoped runs | yes | no |
| Backups, transactions, and undo | yes | no |

Use the [command line](/guide/command-line) for everything in the right-hand
column.

## Request limits

Requests are bounded to keep browser memory use predictable:

- at most 128 files;
- at most 1 MiB per file;
- at most 4 MiB in total.

Paths must be canonical, portable relative paths of at most 240 UTF-8 bytes, so
duplicates and downloaded tar entries cannot acquire platform-dependent
meanings. A malformed C file fails independently, so other selected files can
still be inspected. Any non-convergent formatter result is discarded rather
than exposed as a usable partial edit.

## Run it locally

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
npm ci --prefix web
npm run dev --prefix web
```

The dev command rebuilds the WebAssembly bindings before starting the site at
<http://127.0.0.1:5173>. Building also requires Clang with WebAssembly support,
used to compile the embedded C grammar.
