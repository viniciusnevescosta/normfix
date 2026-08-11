# Browser playground

The [playground](/) runs the safe `normfix` core in WebAssembly. It accepts C,
header, Makefile, and Markdown project files and returns formatted source,
native diagnostics, function budgets, and unified diffs without uploading the
project.

Desktop browsers use Monaco, with line numbers, search, multiple cursors,
bracket matching, and syntax highlighting for every supported file type.
Mobile and coarse-pointer devices use a lightweight textarea because Monaco
does not officially support mobile browsers.

## Adding your project

Drag files onto the page, or drag the project folder itself. A dropped folder
keeps its structure, so `libft/src/ft_strlen.c` arrives under that path rather
than flattened into a pile of names.

A real project directory holds more than source. Object files, the compiled
binary, `.git`, and editor settings are skipped rather than treated as errors,
and the count of what was skipped is always shown — the import never discards
anything quietly, and never refuses the whole drop because one file is not
something normfix formats. **Choose files** does the same thing for a picker.

## Official 42 headers

Enter a valid student email in **42 identity**. **Remember on this device** is
off by default. When explicitly enabled, the address is stored only in this
browser's same-origin local storage and can be removed at any time with
**Forget**. Otherwise it lasts for the current tab only.

The address is passed to WebAssembly in the tab to generate the official 42
header. It is never sent to a formatting server. Without a valid identity, the
source stays without a generated header and the result includes a diagnostic.

## Getting the result out

A run always covers the whole project, because a header and the file that
includes it are only judged correctly together. The choice is what to do with
the answer: apply every proven fix at once, or only the one in front of you.
Either way a fix stops being applicable once its file has been edited since the
run, since it was proven against the source normfix read, not against whatever
is in the buffer now.

- **Fix all files** writes every proven result back into the project at once.
- **Fix this file** does the same for the file you are looking at.
- **Copy file** copies the selected stable result. If clipboard access is
  denied, the browser selects the text for a keyboard copy.
- **Download file** saves the selected result.
- **Download all (.zip)** saves every stable result in one archive that
  every desktop platform opens without installing anything.
- **Use as new input** feeds a result back into the editor for another run.

## Privacy and network behavior

Source and identity stay in the tab. There is no source upload, account,
analytics dependency, or formatting backend. The only external request is an
unauthenticated, no-referrer fetch of the official repository's public GitHub
star count; the UI uses a bundled fallback when that request is unavailable.

## Working offline

The playground installs itself the first time you open it. After that the page,
the WebAssembly formatter, and the interface need no network at all: open the
same address on a plane, on school wifi at its worst, or while the site itself
is down, and formatting runs exactly as it did before. Nothing was ever
uploaded, so working offline changes how you reach the tool, not what it does.

Your browser can also install it as an app from its address bar or menu. It
then opens in its own window, under the name of the language you selected.

Two things are worth knowing:

- The desktop editor is not part of the install. Monaco is a large download
  that buys syntax highlighting and search, so it is fetched only when you have
  a connection, and kept once you do. Opening the playground offline before it
  has ever loaded gives you the plain text area, which formats identically.
- Only the playground is stored. The documentation you are reading now is a
  separate site and still needs a network.

A new release never replaces the page while you are working in it. It is
downloaded in the background, and the header offers **New version ready** with
a **Reload** button. Until you press it, you keep the version you started with.

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
Paths must be NFC-normalized portable relative paths of at most 240 UTF-8
bytes. It rejects case-insensitive duplicates,
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
