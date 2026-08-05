# Command line

The commandless interface is the shortest way to format a project. Subcommands
make intent clearer in scripts and interactive review.

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

## Workflows

| Command | Writes files | What it does |
|---|---|---|
| `format` | yes | Applies accepted edits |
| `lint` | no | Reports diagnostics against the original bytes; proposes no formatting, header, Makefile, or Markdown replacement |
| `check` | no | Runs formatting and linting in a shadow buffer |
| `budget` | no | A lint run plus one informational line/variable/parameter row per parsed function |
| `preflight` | no | A check-oriented run with the strict compiler check enabled; it does not execute `make` or the program |
| `explain` | no | Prints the bundled English explanation for one stable rule ID, without scanning a project |
| `undo` | yes | Lists or restores an intact transaction backup |

`undo` refuses to overwrite bytes changed after the run it restores. With no
`--run` it selects the newest valid recovery point after interactive
confirmation; non-interactive restoration requires `--force`.

## Options

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
| `--timeout SECONDS` | Per-invocation Norminette timeout; default: 5 seconds |
| `--threads N` | Parallel worker count; default: available hardware |
| `--remove-invalid-comments` | Delete only comments rejected at exact official locations |
| `--remove-unused` | Remove only unreachable `static` functions proven in a complete project |
| `--remove-unexpected` | Move unexpected regular files to recoverable external quarantine |
| `--unsafe` | Enable the closed set of risky/destructive actions |
| `--force` | Confirm destructive capabilities non-interactively |
| `--no-reorder-includes` | Leave contiguous include blocks in their current order |
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

## Include order

A run of `#include` directives is reordered so system headers come first, then
project headers, alphabetically inside each category:

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

::: warning The block must be provably contiguous
A run is rewritten only while **every** line in it is exactly one include
directive. The first line that is anything else — a comment, a blank line, a
conditional, a macro definition, or trailing text after the closing delimiter —
ends the run, and each side is sorted independently. No directive crosses such a
construct, because doing so can change declarations, feature macros, or
conditional compilation.
:::

Names are compared case-insensitively and equal names keep their original
relative order. `--no-reorder-includes` leaves every block untouched; the report
then falls back to the `INCLUDE_ORDER_REVIEW` warning, which `normfix explain
INCLUDE_ORDER_REVIEW` describes offline.

## Git scopes

Git scope selection happens before normal discovery:

```sh
normfix check --changed
normfix format --staged
```

`--changed` means unstaged tracked changes plus untracked files not ignored by
Git; it deliberately does not include staged-only paths. `--staged` uses the
index diff to select names, then analyzes and formats their current
working-tree bytes — it does not rewrite the index or stage the result.

An empty scope is a successful no-op and never falls back to a full-directory
scan. Git is invoked directly, with NUL-delimited paths, a timeout, an output
limit, and path-confinement checks. Absolute or escaping names are rejected. A
candidate that is a symbolic link or not a regular file is safely omitted; a
metadata or Git failure rejects the entire scope instead of silently scanning
another set.

::: tip A scope is not a proof
Git scope is a review convenience, not a complete-project proof. Project-wide
findings that need a closed snapshot are disabled when the scope cannot
provide one.
:::

## Interactive review

```sh
normfix format --interactive
```

The first pass is read-only: `normfix` prints the report and each proposed file
diff, accepting `y`, `n`, `a` (all), or `q` (cancel). It then analyzes the same
selected scope again. Each approval is bound to hashes of the exact original
and proposed bytes shown in the first pass, and the transaction writes only
files whose second-pass plan still matches that snapshot-bound approval.

Interactive mode requires a human terminal and cannot be combined with preview,
JSON, lint/budget, or risky/destructive operations.

## Ignore behavior

Recursive scans respect `.normfixignore` by default, using the Git-ignore style
supported by the `ignore` crate. The legacy `.norminetteignore` filename
remains supported so existing projects do not silently regain ignored inputs.

`.gitignore` is deliberately opt-in through `--use-gitignore`, because ignored
C files can still affect project-wide proofs. Explicit file arguments remain
explicit and are not filtered by ignore files.

## Exit codes

| Code | Meaning |
|---:|---|
| `0` | Fix mode completed with no blocking diagnostic, or the input was already clean |
| `1` | Manual diagnostics remain, or preview mode found proposed changes/quarantine candidates |
| `2` | Discovery, configuration, tool, I/O, transaction, or quarantine failure |
| `130` | An interactive per-file review was cancelled |

Informational advisories do not make a run fail.
