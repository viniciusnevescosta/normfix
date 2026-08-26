# `normfix budget`

A read-only run that adds one informational row per parsed function, showing
how much room is left before the Norm's 25 lines, 5 locals, and 4 parameters.

```sh
normfix budget
normfix budget src
```

```console
$ normfix budget
info[NORM_BUDGET]: 2 occurrences in 1 file
  math_utils.c:4:1   add(): lines 1/25 (24 left), variables 0/5 (5 left),
                     parameters 2/4 (2 left).
  math_utils.c:8:1   scale(): lines 3/25 (22 left), variables 1/5 (4 left),
                     parameters 2/4 (2 left).
 = help: Keep headroom for defense-day changes; limits already exceeded are
         also reported as warnings.
 = source: Norm v4.1 native rule

Summary: files: 1 | proposed: 0 | written: 0 | fixes: 0 | remaining: 14 | info: 2
```

Budget rows are informational and never change the exit code on their own.

`budget` diagnoses the bytes already on disk and never plans edits. Formatting,
header-identity, backup, diff, and removal flags are therefore rejected instead
of being silently ignored. Use `normfix check` to preview repairs.

## Why headroom matters

A function at 24 of 25 lines is Norm-compliant and one defense-day question
away from not being. Budget exists to make that visible before an evaluator
asks you to add a check.

`normfix` reports the number; it never extracts a function for you. Choosing a
function boundary changes program structure, which is a decision that needs a
name and an owner. See
[`normfix explain TOO_MANY_LINES`](/commands/explain).
## Reading it from a script

Every field this command returns is documented in [the JSON API](/reference/api).
