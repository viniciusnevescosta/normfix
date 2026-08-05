# normfix

[![CI](https://github.com/viniciusnevescosta/normfix/actions/workflows/ci.yml/badge.svg)](https://github.com/viniciusnevescosta/normfix/actions/workflows/ci.yml)
[![Release](https://github.com/viniciusnevescosta/normfix/actions/workflows/release.yml/badge.svg)](https://github.com/viniciusnevescosta/normfix/releases)

`normfix` is a native Rust formatter and diagnostic tool for the
[42 Norminette](https://github.com/42School/norminette). It applies only the C,
header, Makefile, and README changes it can validate conservatively,
then reports the remaining architectural or semantic work in clear English.

The command is designed to be used like Norminette:

```sh
cd path/to/a/42-project
normfix
```

With no path, it scans the current directory recursively. It also accepts one
or more files, directories, or a mixture of both:

```sh
normfix main.c
normfix src includes
normfix src/parser.c src/lexer.c includes/minishell.h
```

The current package version is `0.4.0-alpha.1`.

## What it does

For each selected project, `normfix`:

1. discovers `.c`, `.h`, `Makefile`, and README files without following
   symbolic links;
2. resolves a verified 42 identity for official headers;
3. runs the official Norminette 3.3.59 against immutable shadow buffers;
4. parses C into a lossless token-and-trivia representation;
5. applies narrow formatting actions until they reach a fixed point;
6. rejects an action batch if syntax recovery appears, significant tokens
   change unexpectedly, or a new Norminette rule is introduced;
7. runs a diagnostics-only strict C compiler preflight by default;
8. analyzes Makefiles, README documents, and an optional project policy with
   dedicated parsers;
9. commits all accepted source replacements through a recoverable transaction;
10. groups repeated diagnostics by rule while retaining every location, then
    prints a summary and total elapsed time.

It does not promise to rewrite every violation. Long functions receive an
extraction suggestion, not an automatic extraction. Control-flow redesign,
public API changes, project-wide renames, and other changes that require human
intent remain diagnostics.

## Requirements

- The official Norminette version `3.3.59` available on `PATH`, or supplied with
  `--norminette PATH`.
- Rust 1.85 or newer only when building from source. Release archives contain a
  native binary and do not require a Rust toolchain.

Norminette uses its own Python runtime, as provided by the official package.

Install the exact checker in an isolated Python environment when it is not
already available, then verify it:

```sh
pipx install norminette==3.3.59
norminette --version
```

Using a campus-managed Python environment is also supported; only the command
version and its availability on `PATH` matter to `normfix`.

## Install

### Prebuilt binaries

Tagged releases provide native archives for Linux x86-64 and ARM64, plus macOS
Intel and Apple Silicon. Download the archive matching your machine from the
[latest release](https://github.com/viniciusnevescosta/normfix/releases/latest),
verify it against `SHA256SUMS`, and place `normfix` on `PATH`.

| Platform | Release archive |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |

There is no native Windows archive. On Windows, run the Linux CLI and its
Norminette dependency inside WSL, or use the browser playground for the
in-memory formatter preview. Native PowerShell and Windows process behavior
are not part of the supported CLI contract yet.

For example, on Apple Silicon with release `0.4.0-alpha.1`:

```sh
version=0.4.0-alpha.1
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Create `$HOME/.local/bin` first if necessary and ensure it is on `PATH`.

### Build from source

Install directly from a local checkout:

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

Or build a release binary without installing it:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

Cargo normally installs the command into `~/.cargo/bin`. Ensure that directory
is on `PATH`.

## Safe first run

Preview a project before writing:

```sh
normfix --check
normfix --diff
```

Then apply the accepted changes:

```sh
normfix
```

Default fix mode writes in place, but keeps original files in an external
backup directory. No project file is written in `--check` or `--diff` mode.

## Focused workflows

The commandless interface remains the shortest way to format a project.
Subcommands make intent clearer in scripts and interactive review:

```sh
normfix format src includes
normfix lint
normfix check main.c
normfix budget src
normfix preflight
normfix explain TOO_MANY_LINES
normfix undo --list
normfix undo --run RUN_ID
```

`format` writes accepted edits. `lint` is read-only: it reports diagnostics
against the original bytes and does not propose or write formatting, header,
Makefile, or Markdown replacements. It still runs the configured diagnostic
oracles and project checks. `check` runs formatting and linting in a shadow
buffer but writes nothing. `budget` is a lint run with one informational
line/variable/parameter budget row per parsed function. `preflight` is a
read-only check-oriented run with the strict compiler check enabled; it does
not execute `make` or the program.

`explain` prints the bundled English explanation for one stable rule ID
without scanning a project. `undo` lists or restores an intact transaction
backup and refuses to overwrite bytes changed after that run. With no `--run`,
it selects the newest valid recovery point after interactive confirmation;
non-interactive restoration requires `--force`.

## Command-line options

| Option | Behavior |
|---|---|
| `PATH...` | Zero, one, or many files/directories; zero means the current directory |
| `--check` | Plan and report changes without writing |
| `--diff` | Print unified diffs in human output without writing |
| `--changed` | Select unstaged tracked changes plus untracked, non-ignored Git files |
| `--staged` | Select only paths recorded as changed in the Git index |
| `--interactive` | Preview, show each changed-file diff, and ask which files to write |
| `--use-gitignore` | Respect `.gitignore` during recursive directory discovery |
| `--login LOGIN` | Supply or constrain the 42 login used for identity validation |
| `--email EMAIL` | Supply the verified 42 student email used in official headers |
| `--no-backup` | Disable retained backups for ordinary safe formatting writes |
| `--backup-dir PATH` | Use a specific external backup base |
| `--format human\|json` | Select terminal output or the versioned JSON report |
| `--no-color` | Disable ANSI color |
| `-v`, `--verbose` | List every accepted fix in human output |
| `--timeout SECONDS` | Set the per-invocation Norminette timeout; default: 5 seconds |
| `--threads N` | Set the parallel worker count; default: available hardware |
| `--remove-invalid-comments` | Delete only comments rejected at exact official locations |
| `--remove-unused` | Remove only unreachable `static` functions proven in a complete project |
| `--remove-unexpected` | Move unexpected regular files to recoverable external quarantine |
| `--unsafe` | Enable the closed set of risky/destructive actions documented below |
| `--force` | Confirm destructive capabilities non-interactively |
| `--no-format-markdown` | Analyze README documents without canonical CommonMark reprinting |
| `--no-cache` | Disable the external persistent analysis cache |
| `--norminette PATH` | Use one exact Norminette executable |
| `--no-compiler-preflight` | Skip the default strict C compiler advisory pass |
| `--cc PATH` | Use one exact C compiler for preflight and analysis |
| `--analyzer` | Also request GCC `-fanalyzer` advisories, including possible leaks |
| `-h`, `--help` | Show built-in help |
| `-V`, `--version` | Show the native CLI version |

`--check` and `--diff` are mutually exclusive. `--changed` and `--staged` are
mutually exclusive and cannot be combined with explicit path arguments.
`--force` without `--unsafe`, `--remove-unused`, or `--remove-unexpected` is an
error.

### Git scopes and interactive review

Git scope selection happens before normal discovery:

```sh
normfix check --changed
normfix format --staged
```

`--changed` means unstaged tracked changes plus untracked files not ignored by
Git. It deliberately does not include staged-only paths. `--staged` uses the
index diff to select names, then analyzes and formats their current working-tree
bytes; it does not rewrite the index or stage the result. An empty scope is a
successful no-op and never falls back to a full-directory scan. Git is invoked
directly, with NUL-delimited paths, a timeout, an output limit, and
path-confinement checks. Absolute or escaping names are rejected. A candidate
that is a symbolic link or not a regular file is safely omitted; a metadata or
Git failure rejects the entire scope instead of silently scanning another set.
A symbolic-link scope root is itself a reported error. Git scope is therefore
a review convenience, not a complete-project proof.

Interactive formatting is a two-pass workflow:

```sh
normfix format --interactive
```

The first pass is read-only. `normfix` prints the report and each proposed
file diff, accepting `y`, `n`, `a` (all), or `q` (cancel). It then analyzes the
same selected scope again. Each approval is bound to hashes of the exact
original and proposed bytes shown in the first pass; the transaction writes
only files whose second-pass plan still matches that snapshot-bound approval.
Interactive mode requires a human terminal and cannot be combined with
preview, JSON, lint/budget, or risky/destructive operations.

### Ignore behavior

Recursive scans respect `.normfixignore` by default. Its syntax follows the
Git-ignore style supported by the `ignore` crate. The legacy
`.norminetteignore` filename remains supported so existing projects do not
silently regain ignored inputs. `.gitignore` is deliberately opt-in through
`--use-gitignore`, because ignored C files can still affect project-wide
proofs. Explicit file arguments remain explicit and are not filtered by ignore
files.

The `.git` metadata directory and symbolic-link entries are never traversed.

## Official 42 headers

Missing official headers are inserted into C sources, C headers, and Makefiles
when a validated identity is available. Identity resolution uses this order:

1. `--email`, with optional `--login` consistency checking;
2. `NORMFIX_EMAIL`, with an optional environment or CLI login;
3. an INI configuration file;
4. the effective Git `user.email`, if it is a supported 42 address;
5. the `MAIL` environment variable;
6. known Vim, Neovim, VS Code, Cursor, and VSCodium 42-header settings.

The email is the source of truth. The login is the local part before `@`; the
tool never invents an address or silently chooses between ambiguous saved
addresses.

When no valid email is found and both input and error output are interactive
terminals, human mode asks:

```text
No verified 42 student email was found.
Enter your 42 email (Enter, cancel, or q to skip the header):
```

Enter, `cancel`, `q`, or end-of-input skips header insertion while all other
safe fixes continue. JSON and non-interactive runs never prompt. Ctrl-C cancels
the command itself, following normal terminal behavior.

### Persistent identity configuration

The default configuration path is:

```text
$XDG_CONFIG_HOME/normfix/config.ini
```

When `XDG_CONFIG_HOME` is not set, it falls back to:

```text
~/.config/normfix/config.ini
```

Use `NORMFIX_CONFIG` to select another file. The supported format is:

```ini
[header]
login = your_login
email = your_login@student.42.fr
```

Environment configuration is also supported:

```sh
export NORMFIX_LOGIN='your_login'
export NORMFIX_EMAIL='your_login@student.42.fr'
```

One timestamp is captured for the complete run. `SOURCE_DATE_EPOCH` can provide
a reproducible UTC timestamp; an invalid value stops the run instead of
silently using the wall clock.

Valid existing headers retain the `By` and `Created` fields. The filename and
`Updated` line change only when the file has another accepted edit or its header
filename is stale, making a second clean run idempotent.

### Header guards

For ordinary headers, `normfix` can insert a missing filename-derived guard,
repair a mismatched `#ifndef`/`#define` pair, or rename a simple wrong guard.
Every operation requires a closed Git-worktree proof. The proof scans ignored
files too, verifies the expected macro is unused, rejects duplicate
filename-derived guards and dynamic build definitions, and binds approval to
the complete project and header hashes.

Insertion is refused for conditional preprocessing, `#pragma once`, `#undef`,
or another macro collision. A rename is refused when the old names have uses
beyond the canonical whole-file pair. Complex, referenced, repeated-inclusion,
non-Git, or ambiguous headers stay unchanged and receive an actionable warning.

## C fixes applied automatically

The native C formatter currently handles proven cases in these areas:

- UTF-8 BOM removal, CRLF normalization, trailing whitespace, blank-line runs,
  file-start whitespace, and one final newline;
- preprocessor indentation and spacing, excluding sensitive multiline forms;
- required and forbidden blank lines around declarations, preprocessors, and
  functions;
- braces and control bodies that need their own physical line;
- Allman control layout, conservative removal of redundant single-statement
  blocks, and a narrow redundant-`else` cleanup when both branches return;
- four-column tab-stop indentation and common space/tab diagnostics;
- indentation and the required following blank line for simple initial local
  declaration groups;
- spacing around operators, pointers, parentheses, keywords, and function
  declarators;
- group alignment for simple one-line variables and function prototypes,
  including pointer declarators when the group is unambiguous;
- `return value;` to `return (value);`;
- empty parameter lists in function definitions to `(void)`;
- pointer-return `return (0);` to `return (NULL);` when the return type and a
  visible `NULL` provider are both proven;
- line wrapping at proven operators or commas;
- greedy rejoining of continuation lines while the result remains within 80
  display columns.

Long-line packing does not cross comments, preprocessing directives, line
splices, or unrelated instructions. Strings and comments are not split.
Preprocessor lines are not rewritten merely to satisfy width.

The formatter measures terminal display cells: tabs use four-column stops,
combining marks use zero cells, and wide Unicode characters use two.

### Proof gates

Formatting happens only in memory first. For every layout action:

- the source must parse without `ERROR`, `MISSING`, or unknown tape regions;
- the token tape must cover and reconstruct the complete input;
- the ordered token-and-comment fingerprint must remain identical;
- the candidate must reparse without recovery;
- edit ranges must be valid and non-overlapping.

After the complete candidate is produced, Norminette runs again. If any rule
count increases relative to the validated baseline, the native formatting
batch is reverted for that file. Operational failures never authorize a
partial write.

Narrow token-changing actions such as `return (...)` and `(void)` are separate
semantic actions with dedicated construction rules; they are not treated as
generic whitespace edits.

## Diagnostics that remain manual

The terminal report explains the rule, exact source span, origin, and a concrete
next step for work such as:

- functions over 25 body lines;
- more than 4 parameters, 5 local variables, or 5 functions per `.c` file;
- lines over 80 columns with no safe operator/comma break;
- forbidden control structures, ternaries, `goto`, labels, and assignments in
  conditions;
- declaration/assignment separation and declarations after statements;
- public or global identifiers that need project-wide renaming;
- type/include movement and project structure changes;
- ambiguous declarations, function pointers, attributes, bit-fields, and
  multiline declarators;
- malformed or parser-recovered C;
- header guards that fail the closed-worktree proof.

The semantic layer evaluates a conservative subset of C integer constant
expressions, including enum constants. This allows a known enum bound such as
`count[op_total]` to be reported as an informational Norminette compatibility
false positive instead of an actual variable-length array. Unsupported
expressions remain unknown; they are never guessed.

For a long function, the diagnostic suggests extracting a cohesive region and
reports the applicable budget. It never moves statements, invents parameters,
or creates a helper automatically: data flow, naming, visibility, and project
intent cannot be proven from formatting facts alone.

## Allowed-function policy

Projects with a subject-specific function allowlist can add `normfix.toml` at
the project root:

```toml
[project]
name = "get_next_line"
allowed = ["read", "malloc", "free"]
```

The bounded parser intentionally interprets only the relevant quoted `name`
and quoted-identifier `allowed` array. When a C/header scope is selected,
`normfix` independently discovers the complete project C/header set from the
project root, considering regular files without following symbolic links and
with `.gitignore`, `.normfixignore`, and `.norminetteignore` filtering disabled.
Every discovered file must be readable UTF-8 and parse losslessly. Non-`static`
definitions from that closed snapshot authorize calls across translation
units; same-file definitions are handled locally, while a `static` definition
in another file never authorizes the call.

Call candidates are recomputed against the final shadow source so reported
ranges remain correct after header insertion and formatting. Parameters,
function-pointer calls, macro/preprocessor ambiguity, and uppercase macro-like
identifiers fail closed instead of producing a guess. If discovery, reading,
parsing, losslessness, or snapshot revalidation is incomplete, all allowlist
findings are disabled and `FUNCTION_POLICY_PROOF_INCOMPLETE` explains why.
`normfix.toml` itself must be a bounded, non-symlink regular file. The policy
still does not replace the project subject or evaluator.

## Comments and destructive capabilities

Comments rejected as `WRONG_SCOPE_COMMENT` or `COMMENT_ON_INSTR` are reported
by default. `--remove-invalid-comments` deletes only a comment found at the
exact line and display column reported by the official checker. It never
removes the official header, and the remaining code-token fingerprint must be
unchanged.

`--remove-unused` and `--remove-unexpected` request stronger destructive
capabilities:

- unused-function removal considers only `static` definitions;
- it requires the selected inputs to equal the complete `.c`/`.h` project set;
- parser recovery, unknown bytes, preprocessor ambiguity, token pasting,
  attributes, string-based references, duplicate definitions, or an uncertain
  reference graph preserve the function;
- unexpected-file removal is a recoverable quarantine operation, never an
  extension-based permanent deletion.

In an interactive human run, these capabilities require a `y/N` confirmation
before analysis. The prompt grants only the requested capability; each
candidate must still pass its parser, hash, scope, and transaction proofs.
Answering yes does not weaken any proof.

JSON and other non-interactive runs require `--force`:

```sh
normfix --remove-unused --force
normfix --remove-unexpected --force
normfix --unsafe --force
```

`--unsafe` is a closed shorthand for five implemented operations:

- exact-location invalid-comment removal;
- compacting simple `NULL` comparisons only when the dedicated C shape is
  proven;
- removal of proven-missing tokens from simple literal Makefile source lists;
- unreachable-`static` removal under a closed-source proof;
- unexpected-file quarantine.

It does not enable arbitrary edits. Comment removal can also be requested
alone with `--remove-invalid-comments`; the other destructive plans still
require capability authorization.

Use preview mode before a destructive run:

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

Preview modes require the same interactive authorization because the
closed-world planners themselves are capability-gated, but they do not write,
delete, or move project files.

## Backups, transactions, and recovery

Default source backups are external to the scanned project:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

On Unix without `XDG_DATA_HOME`, the fallback is:

```text
~/.local/share/normfix/backups/<run-id>/
```

Each backed-up transaction includes exact original bytes and `journal.json`.
Before the first target changes, the writer:

- canonicalizes the project boundary;
- rejects duplicate, external, symbolic-link, and non-regular targets;
- confirms every current file still matches the analyzed bytes;
- writes external backups;
- stages and synchronizes every replacement.

Targets are committed in sorted path order. A mid-commit error triggers
best-effort rollback from the captured original bytes; an incomplete rollback
is reported with the recovery journal path.

`--no-backup` applies only to ordinary safe formatting. A source deletion
planned by `--remove-invalid-comments` or `--remove-unused` requires external
recovery storage and fails closed if it is unavailable.

Quarantine always retains a recoverable external copy, including when
`--no-backup` was supplied:

```text
<backup-base>/quarantine/<run-id>/<original-relative-path>
```

The source type, byte length, and BLAKE3 hash are rechecked immediately before
the move. Existing recovery destinations are never overwritten. A partial
quarantine failure attempts to restore files already moved.

## Makefile support

Makefiles use a dedicated conservative formatter because Norminette does not
parse GNU Make syntax. The formatter can:

- remove a UTF-8 BOM and normalize line endings;
- insert or update the official `#`-style 42 header;
- ensure one final newline;
- greedily pack plain explicit `.c` assignments up to 80 display columns while
  retaining order and assignment semantics.

It deliberately preserves recipes, `.RECIPEPREFIX` projects, `define` blocks,
shell assignments, variable/function expansion, patterns, comments, quotes,
command separators, and other ambiguous Make constructs.

The analyzer reports:

- a missing `NAME` assignment;
- missing `all`, `clean`, `fclean`, `re`, or `$(NAME)` rules;
- `all` not being the default concrete target;
- wildcard source/object discovery;
- long lines that cannot be safely packed;
- whitespace after a continuation backslash.

For a simple `SRC`/`SRCS`-style assignment whose complete value is made of
literal relative `.c` paths, it also checks whether each token exists. Paths
are resolved from the directory containing that Makefile, including for
nested Makefiles, while every component must remain inside the canonical
project root and avoid symbolic links. A missing path is reported by default.
`--unsafe` may remove only the exact proven-missing token and repack the
remaining list without reordering it. Expansions, patterns, quotes, comments,
recipes, `define` blocks, `.RECIPEPREFIX`, escaping paths, and uncertain
filesystem results are left unchanged.

The tool does not automatically add every `.c` file found on disk to a source
variable. Target membership is a build-design decision.

## Compiler preflight and leak advisories

For each selected `.c` file, the normal pipeline runs a read-only compiler pass
equivalent to:

```text
cc -fsyntax-only -Wall -Wextra -Werror
```

It adds stable `-I` paths for directories containing discovered project
headers, but it does not guess subject-specific defines, language modes,
generated headers, target flags, or linker inputs. Use `--cc PATH` to select an
exact compiler or `--no-compiler-preflight` to skip the pass. Compiler findings
are diagnostics only: they never authorize or reject formatter edits. An
unavailable compiler or visibly incomplete compile context produces a clear,
fail-open advisory.

`--analyzer` additionally asks the chosen compiler for GCC `-fanalyzer` output.
It can surface possible allocation leaks and invalid-access paths, but it is
slower and intentionally informational. It is not a leak proof: path
exploration is incomplete, one translation unit is inspected at a time, and
ownership hidden behind external functions or stored in structs may escape the
analysis. A compiler without `-fanalyzer` support is reported and skipped.

### Pre-defense mode

```sh
normfix preflight
normfix preflight --analyzer
```

`preflight` is the read-only formatter/linter preview intended immediately
before evaluation. It aggregates official Norminette results, native Norm
limits and extraction suggestions, official headers and header guards,
allowed-function policy, Makefile structure and literal source references,
unexpected files, README findings, and the strict compiler pass. The compiler
pass cannot be disabled for this workflow.

When `normfix.toml` is absent, preflight emits
`FUNCTION_POLICY_NOT_CONFIGURED` instead of pretending the authorized-function
check ran. It also emits `PREFLIGHT_MANUAL_STEPS`: the command deliberately
does not execute Make recipes, link or inspect the final binary, run the
program/tests, or invoke runtime leak tools. Run those project-specific steps
separately. `--analyzer` remains opt-in.

## README and Markdown support

README files are parsed through Comrak and canonically reprinted by default:

```sh
normfix README.md
```

Canonical reprinting is idempotent but can create a broad first-run diff. Use
`--check` or `--diff` to preview it. `--no-format-markdown` keeps README files
read-only while still reporting heading-level jumps, trailing whitespace, and
a missing final newline.

## Unexpected project files

Recursive discovery reports regular files other than `.c`, `.h`, `Makefile`,
README variants, `.normfixignore`, and its legacy `.norminetteignore` alias.
This warning alone does not change the exit status and never implies that a
file is disposable.

Use `--remove-unexpected` only when you intend to move all eligible unexpected
regular files to external quarantine. Symbolic links, directories, paths
outside the project, changed snapshots, and overlapping recovery paths are
rejected.

## Terminal and JSON reporting

Human output includes:

- a per-file status table: `CLEAN`, `INFO`, `FIXED`, `WOULD FIX`, `REVIEW`, or
  `FAILED`;
- exact `path:line:display-column` locations;
- grouped rule/severity/source sections, with every affected location and
  message retained;
- source snippets and carets in `--verbose` output, with tabs expanded to
  four-column stops;
- stable rule IDs, shared help, notes, diagnostic origin, and an
  `normfix explain RULE` hint;
- optional accepted-fix details with `--verbose`;
- unified diffs with `--diff`;
- aggregate counts and elapsed wall time.

Color is enabled only for an interactive stdout. `--no-color`, `NO_COLOR`, JSON
output, and redirected output are color-free.

`--format json` emits a deterministic, pretty-printed schema with
`schema_version: 1`. It includes identity metadata, discovery and quarantine
outcomes, per-file change/write/failure fields, fixes, before/after diagnostics,
summary counts, and `duration_seconds`. Source buffers and unified diffs are
intentionally omitted.

### Exit codes

| Code | Meaning |
|---:|---|
| `0` | Fix mode completed with no blocking diagnostic, or the input was already clean |
| `1` | Manual diagnostics remain, or preview mode found proposed changes/quarantine candidates |
| `2` | Discovery, configuration, tool, I/O, transaction, or quarantine failure |
| `130` | An interactive per-file review was cancelled |

Informational advisories do not make a run fail.

## Cache and performance

File analysis runs in parallel through Rayon. `--threads N` creates a local
pool with an exact worker count; without it, Rayon uses the available hardware.
Results and commits are sorted by path, so worker completion order does not
change report order or write order.

Official Norminette reports use both an in-memory run cache and a persistent
redb database outside the project. On Unix:

```text
$XDG_CACHE_HOME/normfix/<project-id>/cache-v1.redb
```

or:

```text
~/.cache/normfix/<project-id>/cache-v1.redb
```

Keys include the schema, analysis namespace, project-relative path when the
input is inside the run root (absolute-path fallback for an explicit external
input), source bytes, Norm configuration, and the verified executable
fingerprint. Cache lock, I/O, decoding, or corruption failures fail open as
misses; they never change diagnostics or exit status. A corrupt database is
preserved under a `.corrupt-N` name before recreation.

Use `--no-cache` for a fully uncached run.

## Browser playground (WebAssembly)

The repository includes a deliberately simple, old-school browser workbench
backed by `normfix-wasm` and a static Vite 8.1.5 frontend. It accepts in-memory
`.c` and `.h` buffers and returns native format proposals, English diagnostics,
unified diffs, and function budgets. All source stays inside the browser
process; the playground has no filesystem or source-upload path.

The browser sandbox cannot execute the external Norminette binary, a C
compiler, Git, Make, transactions, undo, identity discovery, or official-header
updates. Its result is therefore a convenient native formatter preview, not an
official evaluation. See [the playground instructions](web/README.md) to build
and serve it locally or publish the static bundle on Vercel. The playground is
also the no-install preview available to Windows users; the full Windows CLI
workflow remains supported only through WSL.

## Known boundaries

- Exact compatibility requires Norminette 3.3.59; other versions are rejected.
- C files must be valid UTF-8 and contain no NUL bytes.
- Tree-sitter recovery or unclassified tape bytes disable syntax-aware edits
  for that file.
- The default strict compiler pass uses a conservative inferred include
  context; project-specific defines, language mode, generated files, target
  flags, linking, and runtime behavior remain the project's responsibility.
- GCC `-fanalyzer` can suggest possible leaks but cannot prove leak freedom.
- The formatter does not infer project architecture, hidden evaluator
  contracts, public API intent, or target membership.
- Long-function extraction is suggested, never performed automatically.
- A hard 80-column result is guaranteed only when a safe break exists. Long
  literals, comments, directives, and ambiguous expressions remain warnings.
- The source transaction is recoverable and ordered, but a filesystem does not
  provide a single atomic rename spanning multiple files; rollback is the
  cross-file failure strategy.

## Architecture

The implementation separates syntax, semantic facts, actions, official-tool
compatibility, reporting, and filesystem writes so uncertainty cannot leak
into authorization. See:

- [Architecture: decisions, rationale, and invariants](docs/ARCHITECTURE.md)
- [Compatibility and supported toolchain policy](docs/COMPATIBILITY.md)
- [Release process and binary artifacts](docs/RELEASING.md)

The architecture document explains what each crate owns, why the project uses
a lossless token tape beside Tree-sitter, how the shadow-buffer proof pipeline
works, why reports remain deterministic under parallelism, and where the
safety boundary is drawn for destructive capabilities.

## Development

Run the complete native validation suite:

```sh
cargo fmt --all --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
```

Build and exercise the actual command:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --help
./target/release/normfix --check path/to/fixture
```

The workspace forbids project-authored `unsafe` Rust and treats Clippy
`all`/`pedantic` findings as warnings in the manifest; CI and release
validation promote warnings to errors.

## License

MIT
