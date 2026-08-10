# Makefiles, README documents, and project files

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
literal relative `.c` paths, it also checks whether each token exists and
whether the referenced regular file contains any C token beyond whitespace or
comments. Paths
are resolved from the directory containing that Makefile, including for
nested Makefiles, while every component must remain inside the canonical
project root and avoid symbolic links. A missing or trivia-only path is
reported by default. `--unsafe` may remove only the exact proven token and repack the
remaining list without reordering it. Expansions, patterns, quotes, comments,
recipes, `define` blocks, `.RECIPEPREFIX`, escaping paths, and uncertain
filesystem results are left unchanged.

Every filesystem-backed workflow compares non-static prototypes in project
headers with a complete lossless C/header source snapshot. Missing
implementations and matching trivia-only bodies are reported at the prototype
name. Unsafe removal is limited to missing implementations and requires the
complete project scope, scoped authorization, no other identifier use or
ambiguity, shadow reparse validation, and a transaction-time hash check of
every proof input. Existing trivia-only definitions are never removed: an
intentional no-op can be valid.

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

`--analyzer` additionally asks the chosen compiler for GCC `-fanalyzer` output
in ordinary workflows. Preflight performs this bounded analyzer pass
automatically. It can surface possible allocation leaks and invalid-access
paths, but it is
slower and intentionally informational. It is not a leak proof: path
exploration is incomplete, one translation unit is inspected at a time, and
ownership hidden behind external functions or stored in structs may escape the
analysis. A compiler without either supported analyzer interface is reported
and skipped.

### Pre-defense mode

```sh
normfix preflight
```

`preflight` is the read-only formatter/linter preview intended immediately
before evaluation. It aggregates official Norminette results, native Norm
limits and extraction suggestions, official headers and header guards,
allowed-function policy, Makefile structure and literal source references,
trivia-only Makefile sources, header prototypes without a project definition,
trivia-only implementation bodies, unexpected files, README findings, the
strict compiler pass, and the compiler analyzer. Compiler passes cannot be
disabled for this workflow.

The final `Pre-defense estimate` is intentionally non-conclusive. Unexpected
files, installed-Norminette findings, and Makefile diagnostics produce a hard
fail with exact source locations. The 0–100 score and letter band only
prioritize the remaining work; they are not an official grade.

The hard-fail evidence is based on the original on-disk Norminette and Makefile
diagnostics, plus any newly exposed finding that remains in the shadow. A safe
edit proposed by check mode does not retroactively pass the submitted bytes.

When `normfix.toml` is absent, preflight emits
`FUNCTION_POLICY_NOT_CONFIGURED` instead of pretending the authorized-function
check ran. It also emits `PREFLIGHT_MANUAL_STEPS`: the command deliberately
does not execute Make recipes, link or inspect the final binary, run the
program/tests, or invoke runtime leak tools. Run those project-specific steps
separately. It reports whether `clang-tidy` is on `PATH` and gives separate
debug-build sanitizer guidance, but executes neither. When no regular Makefile
is selected or found at the project root, `MAKEFILE_NOT_FOUND` reports an
incomplete check without hard-failing: only subject-specific policy can prove
that every project needs one.

## README and Markdown support

README files are parsed through Comrak and canonically reprinted by default:

```sh
normfix README.md
```

Canonical reprinting is idempotent but can create a broad first-run diff. Use
`--check` or `--diff` to preview it. `--no-format-markdown` keeps README files
read-only while still reporting heading-level jumps, trailing whitespace, and
a missing final newline.

When preflight discovers a README, `README_42_CRITERIA_REVIEW` reminds you to
compare it with the current subject and evaluation sheet. README absence emits
no diagnostic and never fails preflight.

## Unexpected project files

Recursive discovery reports regular files other than `.c`, `.h`, `Makefile`,
README variants, `.normfixignore`, and its legacy `.norminetteignore` alias.
Outside preflight this warning alone does not change the exit status. Preflight
uses it as an explicit hard-fail rule because the evaluated submission scope is
expected to contain only supported project files. It never implies that a file
is disposable.

Use `--remove-unexpected` only when you intend to move all eligible unexpected
regular files to external quarantine. Symbolic links, directories, paths
outside the project, changed snapshots, and overlapping recovery paths are
rejected.
