# Browser playground

The [playground](/) runs the safe `normfix` core in WebAssembly. It accepts C,
header, Makefile, and Markdown project files and returns formatted source,
native diagnostics, function budgets, and unified diffs without uploading the
project.

Desktop browsers use Monaco, with line numbers, search, multiple cursors,
bracket matching, and syntax highlighting for every supported file type.
Mobile and coarse-pointer devices use a lightweight textarea because Monaco
does not officially support mobile browsers.

## Official 42 headers

Enter a valid student email in **42 identity**. With **Remember on this device**
enabled, the address is stored only in this browser's same-origin local storage
and can be removed at any time with **Forget**. Otherwise it lasts for the
current tab only.

The address is passed to WebAssembly in the tab to generate the official 42
header. It is never sent to a formatting server. Without a valid identity, the
source stays without a generated header and the result includes a diagnostic.

## Getting the result out

- **Copy file** copies the selected stable result. If clipboard access is
  denied, the browser selects the text for a keyboard copy.
- **Download file** saves the selected result.
- **Download all (.tar)** saves every stable result in one portable archive.
- **Use as new input** feeds a result back into the editor for another run.

## Privacy and network behavior

Source and identity stay in the tab. There is no source upload, account,
analytics dependency, or formatting backend. The only external request is an
unauthenticated, no-referrer fetch of the official repository's public GitHub
star count; the UI uses a bundled fallback when that request is unavailable.

## CLI and playground boundaries

| Capability | CLI | Playground |
|---|---:|---:|
| Safe C/header formatting | yes | yes |
| Safe Makefile and Markdown formatting | yes | yes |
| Official 42 header from a supplied identity | yes | yes |
| Structural diagnostics and function budgets | yes | yes |
| Unified diffs | yes | yes |
| Official Norminette oracle | yes | no |
| Strict compiler preflight and analyzer | yes | no |
| Automatic identity discovery | yes | no |
| Git scopes | yes | no |
| Backups, transactions, and undo | yes | no |

The browser sandbox cannot execute the official Norminette binary, a compiler,
Git, or Make. Use the [command line](/guide/command-line) for official checking
and the complete pre-defense workflow.

## Limits and portability

The playground accepts at most 128 files, 1 MiB per file, and 4 MiB total.
Paths must be NFC-normalized portable relative paths of at most 240 UTF-8 bytes
and must fit a portable tar header. It rejects case-insensitive duplicates,
reserved platform names, invalid UTF-8, and archive-unsafe paths before running
the formatter. A leading UTF-8 BOM is consumed consistently. Any formatter
result that does not reach a fixed point is discarded instead of exposed as a
usable partial edit.

## Run locally

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
npm ci
npm run dev
```

Building also requires a Clang installation with a working WebAssembly target.
On macOS, the build probes Homebrew LLVM paths and explains how to install LLVM
when the system compiler cannot target `wasm32`.
