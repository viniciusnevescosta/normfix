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

## What it does not do

It does not run `make`, link a binary, execute your program or tests, or prove
the absence of leaks. Those remain yours, and the report says so.

```sh
normfix preflight --analyzer
```

adds deep static-analysis findings: `-fanalyzer` on GCC, `--analyze` on Clang.
`normfix` chooses from the compiler's version banner, which matters because
`/usr/bin/gcc` on macOS is Clang wearing another name.

They can *suggest* a leak or an invalid access; they never prove correctness,
and they never authorize an edit. A compiler with no analyzer at all reports
`CC_ANALYZER_UNAVAILABLE` and the run continues.

`preflight` refuses to combine with `--no-compiler-preflight`, because the compiler
pass is the point of the command.
