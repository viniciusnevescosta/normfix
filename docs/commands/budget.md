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

## Why headroom matters

A function at 24 of 25 lines is Norm-compliant and one defense-day question
away from not being. Budget exists to make that visible before an evaluator
asks you to add a check.

`normfix` reports the number; it never extracts a function for you. Choosing a
function boundary changes program structure, which is a decision that needs a
name and an owner. See
[`normfix explain TOO_MANY_LINES`](/commands/explain).

## Reading it from a script

```sh
normfix --format json budget src
```

```json
{
  "schema_version": 2,
  "tool_version": "1.6.2",
  "mode": "budget",
  "scope": { "selection": "explicit_paths", "respects_gitignore": false },
  "granted_capabilities": [],
  "files": [
    {
      "path": "main.c",
      "changed": true,
      "written": false,
      "fixes": [],
      "before": [],
      "after": []
    }
  ],
  "summary": { "files": 1, "changed": 1, "written": 0, "fixes": 1, "remaining": 0, "failed": 0 },
  "duration_seconds": 0.31
}
```

Branch on `schema_version` first. `scope` says how the files were chosen and
`granted_capabilities` what the run was allowed to do; both are present and
plain on an ordinary run, so their absence never has to be interpreted. The
full field list is in [reporting](/reference/reporting).
