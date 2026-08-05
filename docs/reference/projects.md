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
