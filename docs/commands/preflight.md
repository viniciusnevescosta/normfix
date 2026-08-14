# `normfix preflight`

The read-only checks worth running immediately before a 42 evaluation, with the
strict compiler pass enabled.

```sh
normfix preflight
```

It runs everything [`check`](/commands/check) runs, plus
`cc -fsyntax-only -Wall -Wextra -Werror` against the real translation units on
disk.

```console
$ normfix preflight
error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
  srcs/sort/sort.c:30:5           call to undeclared function 'sort_medium'
  srcs/sort/sort_adaptive.c:21:5  call to undeclared function 'sort_medium'
    note: Compiler diagnostics inspect the original on-disk translation unit
          and never authorize or reject formatter edits.
 = help: Fix this strict compiler diagnostic, then rerun normfix.
 = source: C compiler
```

That example is real: a header declared `sort_medium` but no file defined it,
so the project did not build. Norminette would never have told you.

## A complete run, before and after

Every output on this page comes from an actual run. The project below is four
files: `main.c` and `add.c` indented with spaces, a `demo.h` declaring an
`unused_api` nobody implements, and a Makefile whose `SRC` still lists a
`ghost.c` that was deleted.

Preflight states what it is about to do before it reads anything:

```console
$ normfix preflight
normfix · starting
  action       preflight
  mode         read-only check
  scope        /home/student/demo (recursive)
  working dir  /home/student/demo
  identity     student@student.42.fr (user config)
  workers      auto
  checks       Norminette + strict compiler
  norminette   automatic PATH discovery
  version rule advisory (other releases continue)
  timeout      5s per file
  cache        enabled
  gitignore    not applied
  backups      automatic external backup
  destructive  none
  force        no
```

Then it reports the estimate against the bytes currently on disk:

```console
Pre-defense estimate: HARD FAIL | grade FAIL | 31/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:1:1 [INVALID_HEADER] The official 42 Makefile header is missing or malformed
  add.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  demo.h:1:1 [INVALID_HEADER] Missing or invalid 42 header
  main.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  Makefile:2:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
  add.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:5:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:8 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:7:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:8:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:7:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:8:1 [TOO_FEW_TAB] Missing tabs for indent level
```

Most of that list is exactly what `normfix` repairs. Running the default fix
and asking again:

```console
$ normfix
$ normfix preflight
Pre-defense estimate: HARD FAIL | grade FAIL | 59/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:14:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
```

Thirteen hard failures are gone and one remains, which is the useful result:
the deleted `ghost.c` is still listed in the Makefile, and no tool should
decide on its own whether that file should come back or the line should go.
The verdict stays `HARD FAIL` while any hard failure remains — the score moves,
the verdict does not soften.

The evaluated bytes are the submitted bytes. In the first run, `normfix` had
already computed the fixes for every `INVALID_HEADER` and `SPACE_REPLACE_TAB`
above, and the estimate still failed on them, because a repair you have not
written is not part of what an evaluator will open.

Every filesystem-backed workflow, including the default check, compares
non-static prototypes in project headers with every project C or header file
that parsed cleanly. A missing implementation or a matching definition whose body is
only braces, whitespace, and comments is highlighted at the prototype name.
Generated sources and external libraries remain ambiguous. Explicitly
authorized `--unsafe` mode removes only a missing prototype when the complete
source set contains no definition, call, function-pointer/reference, macro,
string, conditional, attribute, or token-paste evidence. A trivia-only existing
definition is warning-only because an intentional no-op can be valid.

## Estimate and hard-fail rules

The report ends with a 0–100 estimate, letter band, and verdict. It is always
labelled **non-conclusive**. It is a prioritization aid, not a predicted 42
grade.

The verdict is `HARD FAIL` when any of these objective conditions is present:

- an unexpected project file in the evaluated scope;
- a Norm finding corroborated by the installed official Norminette;
- a static Makefile diagnostic or Makefile processing failure.

Each source hard-fail item repeats its exact `path:line:column`, rule ID, and
message. An operational Makefile failure names the file without inventing a
source coordinate.
Official Norm and Makefile findings are evaluated against the original bytes on
disk; a proposed read-only fix does not turn the current submission into a pass.
Findings that would still be there after the proposed edits count too.
README absence is not a hard fail. When a README is present, an informational
advisory asks you to compare it with the current subject/evaluation sheet.
If no regular Makefile is selected or found at the project root,
`MAKEFILE_NOT_FOUND` reports that build-target and source-list checks did not
run. It is an advisory and costs no score: a piscina exercise is expected to
contain only `.c` files, so a Makefile and project headers are both optional
there. Only the subject can say whether a Makefile is required, and normfix
does not read subjects.

## What it does not do

It does not run `make`, link a binary, execute your program or tests, or prove
the absence of leaks. Those remain yours, and the report says so.

For leaks specifically, [`normfix leaks`](/commands/leaks) runs a program you
already built under a leak checker. It is a separate command because it executes
your code, and it asks before it does.

Preflight reports whether `clang-tidy` is available on `PATH` and shows a
practical AddressSanitizer/UndefinedBehaviorSanitizer debug-build recipe. It
does not run `clang-tidy`, sanitizers, `make` (not even `make -n`, which can
evaluate `$(shell ...)`), or a project binary. Such execution needs separate,
explicit trust in the project's build and runtime behavior.

Preflight automatically adds a bounded deep static-analysis pass: `-fanalyzer`
on GCC, `--analyze` on Clang. Ordinary workflows still require `--analyzer`.
`normfix` chooses from the compiler's version banner, which matters because
`/usr/bin/gcc` on macOS is Clang wearing another name.

They can *suggest* a leak or an invalid access; they never prove correctness,
and they never authorize an edit. A compiler with no analyzer at all reports
`CC_ANALYZER_UNAVAILABLE` and the run continues.

`preflight` refuses to combine with `--no-compiler-preflight`, because the compiler
pass is the point of the command.
## Reading it from a script

Every field this command returns is documented in [the JSON API](/reference/api).
