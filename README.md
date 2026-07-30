# norminette-fix

`norminette-fix` is a native Rust formatter and diagnostic tool for the
[42 Norminette](https://github.com/42School/norminette). It applies only the C,
header, Makefile, and opt-in README changes it can validate conservatively,
then reports the remaining architectural or semantic work in clear English.

The command is designed to be used like Norminette:

```sh
cd path/to/a/42-project
norminette-fix
```

With no path, it scans the current directory recursively. It also accepts one
or more files, directories, or a mixture of both:

```sh
norminette-fix main.c
norminette-fix src includes
norminette-fix src/parser.c src/lexer.c includes/minishell.h
```

The current package version is `0.4.0-alpha.1`. The native fixer is implemented,
but the alpha label remains while compatibility fixtures and real-project
validation are expanded.

## What it does

For each selected project, `norminette-fix`:

1. discovers `.c`, `.h`, `Makefile`, and README files without following
   symbolic links;
2. resolves a verified 42 identity for official headers;
3. runs the official Norminette 3.3.59 against immutable shadow buffers;
4. parses C into a lossless token-and-trivia representation;
5. applies narrow formatting actions until they reach a fixed point;
6. rejects an action batch if syntax recovery appears, significant tokens
   change unexpectedly, or a new Norminette rule is introduced;
7. analyzes Makefiles and README documents with dedicated parsers;
8. commits all accepted source replacements through a recoverable transaction;
9. prints source snippets, exact locations, help, a summary, and total duration.

It does not promise to rewrite every violation. Function extraction, control
flow redesign, public API changes, project-wide renames, and other changes that
require human intent remain diagnostics.

## Requirements

- Rust 1.85 or newer to build. This repository pins Rust 1.97.1 for development.
- The official Norminette version `3.3.59` available on `PATH`, or supplied with
  `--norminette PATH`.

The native binary does not use the legacy Python fixer. Norminette itself still
uses its own Python runtime, as provided by the official package.

Verify the required checker before installation:

```sh
norminette --version
```

## Install

Install directly from a local checkout:

```sh
git clone https://github.com/viniciusnevescosta/norminette-fix.git
cd norminette-fix
cargo install --path crates/normfix-cli --locked
```

Or build a release binary without installing it:

```sh
cargo build --release --locked -p normfix-cli
./target/release/norminette-fix --version
```

Cargo normally installs the command into `~/.cargo/bin`. Ensure that directory
is on `PATH`.

## Safe first run

Preview a project before writing:

```sh
norminette-fix --check
norminette-fix --diff
```

Then apply the accepted changes:

```sh
norminette-fix
```

Default fix mode writes in place, but keeps original files in an external
backup directory. No project file is written in `--check` or `--diff` mode.

## Command-line options

| Option | Behavior |
|---|---|
| `PATH...` | Zero, one, or many files/directories; zero means the current directory |
| `--check` | Plan and report changes without writing |
| `--diff` | Print unified diffs in human output without writing |
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
| `--unsafe` | Enable all three removal capabilities above |
| `--force` | Confirm destructive capabilities non-interactively |
| `--format-markdown` | Canonically reprint README documents through a CommonMark AST |
| `--no-cache` | Disable the external persistent analysis cache |
| `--norminette PATH` | Use one exact Norminette executable |
| `-h`, `--help` | Show built-in help |
| `-V`, `--version` | Show the native CLI version |

`--check` and `--diff` are mutually exclusive. `--force` without
`--unsafe`, `--remove-unused`, or `--remove-unexpected` is an error.

### Ignore behavior

Recursive scans respect `.norminetteignore` by default. Its syntax follows the
Git-ignore style supported by the `ignore` crate. `.gitignore` is deliberately
opt-in through `--use-gitignore`, because ignored C files can still affect
project-wide proofs. Explicit file arguments remain explicit and are not
filtered by ignore files.

The `.git` metadata directory and symbolic-link entries are never traversed.

## Official 42 headers

Missing official headers are inserted into C sources, C headers, and Makefiles
when a validated identity is available. Identity resolution uses this order:

1. `--email`, with optional `--login` consistency checking;
2. `NORMINETTE_FIX_EMAIL`, with an optional environment or CLI login;
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
$XDG_CONFIG_HOME/norminette-fix/config.ini
```

When `XDG_CONFIG_HOME` is not set, it falls back to:

```text
~/.config/norminette-fix/config.ini
```

Use `NORMINETTE_FIX_CONFIG` to select another file. The supported format is:

```ini
[header]
login = your_login
email = your_login@student.42.fr
```

Environment configuration is also supported:

```sh
export NORMINETTE_FIX_LOGIN='your_login'
export NORMINETTE_FIX_EMAIL='your_login@student.42.fr'
```

One timestamp is captured for the complete run. `SOURCE_DATE_EPOCH` can provide
a reproducible UTC timestamp; an invalid value stops the run instead of
silently using the wall clock.

Valid existing headers retain the `By` and `Created` fields. The filename and
`Updated` line change only when the file has another accepted edit or its header
filename is stale, making a second clean run idempotent.

### Header guards

A simple filename-derived inclusion guard may be renamed automatically, but
only after a closed Git-worktree proof. The proof scans ignored files too,
checks that the old macro appears only in the canonical `#ifndef`/`#define`
pair, verifies the expected name is unused, rejects duplicate filename-derived
guards and dynamic build definitions, and binds approval to content hashes.

Complex, referenced, conditional, repeated-inclusion, non-Git, or ambiguous
guards stay unchanged and receive an actionable warning.

## C fixes applied automatically

The native C formatter currently handles proven cases in these areas:

- UTF-8 BOM removal, CRLF normalization, trailing whitespace, blank-line runs,
  file-start whitespace, and one final newline;
- preprocessor indentation and spacing, excluding sensitive multiline forms;
- required and forbidden blank lines around declarations, preprocessors, and
  functions;
- braces and control bodies that need their own physical line;
- four-column tab-stop indentation and common space/tab diagnostics;
- spacing around operators, pointers, parentheses, keywords, and function
  declarators;
- group alignment for simple one-line variables and function prototypes,
  including pointer declarators when the group is unambiguous;
- `return value;` to `return (value);`;
- empty parameter lists in function definitions to `(void)`;
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
norminette-fix --remove-unused --force
norminette-fix --remove-unexpected --force
norminette-fix --unsafe --force
```

`--unsafe` is a closed shorthand for comment removal, unreachable-static
removal, and unexpected-file quarantine. It does not enable arbitrary edits.

Use preview mode before a destructive run:

```sh
norminette-fix --diff --remove-unused
norminette-fix --check --remove-unexpected
```

Preview modes require the same interactive authorization because the
closed-world planners themselves are capability-gated, but they do not write,
delete, or move project files.

## Backups, transactions, and recovery

Default source backups are external to the scanned project:

```text
$XDG_DATA_HOME/norminette-fix/backups/<run-id>/
```

On Unix without `XDG_DATA_HOME`, the fallback is:

```text
~/.local/share/norminette-fix/backups/<run-id>/
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

The tool does not automatically add every `.c` file found on disk to a source
variable. Target membership is a build-design decision.

## README and Markdown support

README files are parsed through Comrak. By default they are read-only and
receive informational diagnostics for heading-level jumps, trailing
whitespace, and a missing final newline.

`--format-markdown` opts into canonical CommonMark reprinting:

```sh
norminette-fix --format-markdown README.md
```

Canonical reprinting is idempotent but may create a broad diff, so it is never
enabled implicitly.

## Unexpected project files

Recursive discovery reports regular files other than `.c`, `.h`, `Makefile`,
README variants, and `.norminetteignore`. This warning alone does not change
the exit status and never implies that a file is disposable.

Use `--remove-unexpected` only when you intend to move all eligible unexpected
regular files to external quarantine. Symbolic links, directories, paths
outside the project, changed snapshots, and overlapping recovery paths are
rejected.

## Terminal and JSON reporting

Human output includes:

- a per-file status table: `CLEAN`, `INFO`, `FIXED`, `WOULD FIX`, `REVIEW`, or
  `FAILED`;
- exact `path:line:display-column` locations;
- source snippets with tabs expanded to four-column stops;
- carets over the affected range;
- severity, stable rule ID, help, notes, and diagnostic origin;
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

Informational advisories do not make a run fail.

## Cache and performance

File analysis runs in parallel through Rayon. `--threads N` creates a local
pool with an exact worker count; without it, Rayon uses the available hardware.
Results and commits are sorted by path, so worker completion order does not
change report order or write order.

Official Norminette reports use both an in-memory run cache and a persistent
redb database outside the project. On Unix:

```text
$XDG_CACHE_HOME/norminette-fix/<project-id>/cache-v1.redb
```

or:

```text
~/.cache/norminette-fix/<project-id>/cache-v1.redb
```

Keys include the schema, analysis namespace, project-relative path when the
input is inside the run root (absolute-path fallback for an explicit external
input), source bytes, Norm configuration, and the verified executable
fingerprint. Cache lock, I/O, decoding, or corruption failures fail open as
misses; they never change diagnostics or exit status. A corrupt database is
preserved under a `.corrupt-N` name before recreation.

Use `--no-cache` for a fully uncached run.

## Known boundaries

- Exact compatibility requires Norminette 3.3.59; other versions are rejected.
- C files must be valid UTF-8 and contain no NUL bytes.
- Tree-sitter recovery or unclassified tape bytes disable syntax-aware edits
  for that file.
- The default pipeline does not currently invoke the optional C compiler
  adapter, because correct validation requires project-specific include paths,
  defines, language mode, and target flags.
- The formatter does not infer project architecture, hidden evaluator
  contracts, public API intent, or target membership.
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
- [Native Rust migration status](docs/MIGRACAO_RUST_STATUS.md)
- [Documentation index](docs/README.md)

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
cargo build --release --locked -p normfix-cli
./target/release/norminette-fix --help
./target/release/norminette-fix --check path/to/fixture
```

The workspace forbids project-authored `unsafe` Rust and treats Clippy
`all`/`pedantic` findings as warnings in the manifest; CI and release
validation promote warnings to errors.

## License

MIT
