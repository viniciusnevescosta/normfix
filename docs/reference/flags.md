# Every flag

Each entry says what the flag does, when you would reach for it, and shows it
being used. Flags are global: they work with the bare command and with every
subcommand.

Run `normfix --help` for the same list without the prose.

## Selecting what to process

### `PATH...`

Zero, one, or many files and directories. Zero means the current directory,
scanned recursively without following symbolic links.

```sh
normfix                                   # the whole project
normfix main.c                            # one file
normfix src includes                      # two directories
normfix src/parser.c includes/shell.h     # a mixture
```

An explicit file argument is always processed, even if an ignore file would
have excluded it.

### `--changed`

Process unstaged tracked changes plus untracked files Git does not ignore.

```sh
normfix --changed
```

Use it while working: it formats what you just touched instead of rewriting the
whole project. It deliberately excludes staged-only paths.

### `--staged`

Process only the paths recorded as changed in the Git index.

```sh
normfix check --staged
```

It reads the index to select *names*, then analyzes the current working-tree
bytes. It does not rewrite the index or stage the result, so `git diff --staged`
is unaffected.

Cannot be combined with `--changed` or with explicit paths. An empty scope is a
successful no-op, and it never falls back to scanning everything.

### `--use-gitignore`

Also honor `.gitignore` during recursive discovery.

```sh
normfix --use-gitignore
```

Off by default, deliberately: a C file you told Git to ignore still takes part
in project-wide proofs like the allowed-function check. `.normfixignore` is
always honored.

## Previewing instead of writing

### `--check`

Plan everything, write nothing.

```sh
normfix --check
normfix --check --format json > report.json
```

Exit code `1` means there is work to do, which makes it a one-line CI gate.

### `--diff`

Print a unified diff of every proposed change, and write nothing.

```sh
normfix --diff
normfix --diff src/parser.c
```

Tabs render as `\t` so indentation changes stay visible. Mutually exclusive
with `--check`.

### `--interactive`

Preview each changed file and choose which ones get written.

```sh
normfix format --interactive
```

Answer `y`, `n`, `a` (all), or `q` (cancel). The approval is bound to the exact
bytes you were shown; if a file changes underneath you, it is skipped rather
than written. Requires a terminal, and refuses to combine with `--check`,
`--diff`, JSON output, or destructive flags.

## Identity for official headers

### `--login LOGIN`

Supply or constrain the 42 login used in the official header.

```sh
normfix --login vneves-c
```

### `--email EMAIL`

Supply the verified 42 student email. The email is the source of truth; the
login is validated against it.

```sh
normfix --email vneves-c@student.42.fr
```

Without either flag, `normfix` resolves the identity from your environment and
Git configuration, and asks interactively when it cannot and the run needs one.
A valid explicitly supplied identity, or a valid answer to that prompt, is
atomically saved in the platform's private per-user configuration so later runs
do not ask again. See [official headers](/reference/headers) for paths and
permissions.

## Backups and recovery

### `--no-backup`

Skip retained backups for ordinary formatting writes.

```sh
normfix --no-backup
```

It does **not** skip recovery for a destructive removal. Those always require
external storage and fail closed without it. Skipping backups means
[`undo`](/commands/undo) has nothing to restore for that run.

### `--backup-dir PATH`

Use a specific external backup base instead of the default under
`$XDG_DATA_HOME`.

```sh
normfix --backup-dir ~/normfix-backups
```

The directory must not overlap the project. A path inside it, or above it, is
refused, before and after resolving symbolic links.

## Output

### `--format human|json`

Terminal output, or the versioned JSON report.

```sh
normfix --check --format json | jq '.summary'
```

Always branch on `schema_version` before reading JSON. The human layout is free
to improve between releases; the JSON structure is not.

### `--lang`

Choose the language of human output: `en`, `pt`, `es`, or `fr`.

```sh
normfix check --lang pt
```

```console
$ normfix check --lang pt
normfix · iniciando
  ação             check
  modo             somente leitura
  escopo           /home/student/demo (recursivo)
...
Resumo: 1 arquivo(s) | 1 proposto(s) | 0 gravado(s) | 1 correção(ões) | 0 pendente(s) | 0 informativo(s) | 0 com falha | 0 inesperado(s) | 0 em quarentena
Concluído em 219 ms.
```

Without the flag the process locale is used — `NORMFIX_LANG`, then `LC_ALL`,
`LC_MESSAGES`, and `LANG` — falling back to English. Only the primary subtag
matters, so `pt_BR.UTF-8` selects Portuguese. An unpublished `--lang` value
continues in English with one advisory rather than failing.

This changes explanations only. Command names, flag spellings, rule IDs, exit
codes, and every value in `--format json` stay identical in all four languages,
so a script never has to select a language to keep working.

Rule messages from the analysis backends are still English. A non-English run
says so in one line rather than presenting a partly translated report as a
complete one.

### `--no-color`

Disable ANSI colors even on a terminal.

```sh
normfix --no-color
```

Colors are already disabled when output is not a terminal, or when `NO_COLOR`
is set.

### `-v`, `--verbose`

List every accepted fix instead of only the count.

```sh
normfix --check -v
```

Useful when you want to know exactly which seventeen fixes a file received.

## Execution

### `--threads N`

Set the parallel worker count. Defaults to available hardware.

```sh
normfix --threads 1
```

Use `1` to make output ordering trivially reproducible while debugging. Results
and writes are sorted by path regardless, so worker count never changes the
report or the order files are written in.

### `--timeout SECONDS`

Per-file Norminette timeout. Default `5`.

```sh
normfix --timeout 15
```

Raise it on a slow machine or a very large file. A timeout is an operational
failure for that file, not a diagnostic.

### `--no-cache`

Disable the external analysis cache.

```sh
normfix --no-cache
```

The cache stores official checker results outside the project, keyed by the
source bytes and the verified checker fingerprint. Disable it to force a full
re-run; a cache failure already fails open as a miss.

### `--norminette PATH`

Use one exact Norminette executable instead of searching `PATH`.

```sh
normfix --norminette ~/.local/pipx/venvs/norminette/bin/norminette
```

The version is fingerprinted. Release `3.3.59` is tested; another parseable
release continues with a prominent `NORMINETTE_VERSION_UNTESTED` advisory.

## Compiler checks

### `--strict-norminette-version`

Refuse a Norminette release this version has not been verified against.

```sh
normfix --strict-norminette-version
```

The default keeps working when a campus installs a newer official release while
still naming the compatibility gap. Strict mode is useful for reproducible CI
that deliberately pins `3.3.59`. The former
`--allow-untested-norminette` spelling remains as a hidden no-op during the
release-candidate transition.

### `--no-compiler-preflight`

Skip the strict `cc -fsyntax-only -Wall -Wextra -Werror` pass.

```sh
normfix --no-compiler-preflight
```

The pass is on by default and is diagnostics-only: it never authorizes or
rejects a formatter edit. Skip it when your project needs build flags the
inferred context cannot supply, and the noise is not useful.

### `--cc PATH`

Use one exact compiler for the strict syntax pass and the deep analyzer. The
analyzer is automatic in `preflight`; ordinary workflows require `--analyzer`.

```sh
normfix --cc /usr/bin/gcc-14
```

The compiler is identified by its version banner, so a command named `gcc` that
is really Clang is treated as Clang.

### `--analyzer`

Also run the deep static analyzer your compiler ships during an ordinary
workflow. `preflight` already enables this bounded pass automatically.

```sh
normfix --analyzer
```

`normfix` picks the flags from the compiler's own version banner, not from the
command name:

| Compiler | What runs |
|---|---|
| GCC | `-fanalyzer` |
| Clang | `--analyze -Xclang -analyzer-output=text` |
| Anything else | Nothing; the run reports `CC_ANALYZER_UNAVAILABLE` and continues |

::: warning `/usr/bin/gcc` on macOS is Clang
Apple ships a `gcc` command that answers `Apple clang version ...`. Choosing it
with `--cc` does not get you `-fanalyzer`. `normfix` detects this and uses the
Clang analyzer instead, so the flag does what you meant either way.
:::

Both analyzers are slower and informational. They are automatic in `preflight`
and opt-in elsewhere. They can suggest a leak or an invalid access along a
path; neither is proof of either, and neither is ever proof of their absence. A
missing analyzer never changes the exit status.

For a real GCC on macOS, install one and point at it explicitly:

```sh
brew install gcc
normfix preflight --cc "$(brew --prefix)/bin/gcc-14"
```

## Content that is rewritten

### `--no-reorder-includes`

Leave contiguous `#include` blocks in their current order.

```sh
normfix --no-reorder-includes
```

By default a run of include directives is sorted with system headers first,
then project headers, alphabetically inside each. A block is only rewritten
while every line in it is exactly one include directive, so a comment or a
conditional ends the run and nothing crosses it.

### `--no-format-markdown`

Leave README documents unchanged.

```sh
normfix --no-format-markdown
```

README files are reprinted as canonical CommonMark by default. That can produce
a large first-run diff, which is the usual reason to turn it off.

## Destructive operations

Each of these deletes or moves something. All of them keep recoverable external
storage, and all of them require confirmation.

### `--remove-invalid-comments`

Delete only the comments the official checker rejected at exact locations.

```sh
normfix --remove-invalid-comments
```

Nothing else is touched: a comment the checker accepts is never removed.

### `--remove-unused`

Remove `static` functions proven unreachable in the complete project.

```sh
normfix --remove-unused
```

The proof needs every project source to be readable and unambiguous. One
unreadable file disables the whole analysis rather than producing a partial
answer.

### `--remove-unexpected`

Move unexpected regular files into external quarantine.

```sh
normfix --remove-unexpected
```

Nothing is deleted: files are moved to recovery storage with their relative
path preserved, and an existing destination is never overwritten.

### `--unsafe`

Enable the closed set above, plus NULL-check compaction and stale Makefile
source removal.

```sh
normfix --unsafe
```

It is a named set, not an open switch. It cannot enable an operation that does
not already exist as its own flag.

### `--force`

Confirm destructive operations without a prompt, or explicitly acknowledge a
protected system/broad scope.

```sh
normfix --unsafe --force
```

For CI and scripts. `--force` on its own, with no destructive flag, is an
error unless the selected scope is protected. A protected-scope acknowledgement
does not create any destructive capability; those still require their own
flags.

## Environment

### `NORMFIX_NO_UPDATE_CHECK`

Disables the once-a-day release notice.

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

The notice only ever appears for interactive human output and is silent on
failure. See [`upgrade`](/commands/upgrade) for exactly what it sends.

## Information

### `-h`, `--help`

```sh
normfix --help
normfix undo --help
```

### `-V`, `--version`

```sh
normfix --version
```
