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

Every filesystem-backed workflow, including the default check, compares
non-static prototypes in project headers with every losslessly parsed project
C/header file. A missing implementation or a matching definition whose body is
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
New findings that remain in the final shadow are included too.
README absence is not a hard fail. When a README is present, an informational
advisory asks you to compare it with the current subject/evaluation sheet.
If no regular Makefile is selected or found at the project root,
`MAKEFILE_NOT_FOUND` says build verification is incomplete. It remains an
advisory because not every subject requires a Makefile and no subject policy was
proven.

## What it does not do

It does not run `make`, link a binary, execute your program or tests, or prove
the absence of leaks. Those remain yours, and the report says so.

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
