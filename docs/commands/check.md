# `normfix check`

Runs the complete fixing pipeline in memory and reports the result without
touching a single file.

```sh
normfix check
normfix check main.c
```

`normfix --check` is the same thing.

```console
$ normfix check
Files
STATUS      FIXES  REMAINING  INFO  FILE
REVIEW        1          1     0  Makefile
WOULD FIX     2          0     0  add.c
REVIEW        3          1     0  demo.h
WOULD FIX     6          0     0  main.c

Summary: 4 files | 4 proposed | 0 written | 12 fixes | 2 remaining | 0 info | 0 failed | 0 unexpected | 0 quarantined
Completed in 578 ms.
```

`WOULD FIX` and `4 proposed` are the difference from [`lint`](/commands/lint):
check plans the edits and tells you how many passed the proof gates, it just
does not commit them.

The two statuses answer different questions. `WOULD FIX` means everything found
in that file has a proven repair waiting — `add.c` and `main.c` need nothing
from you. `REVIEW` means something is left after every safe fix is applied, and
the `REMAINING` column counts it: here the Makefile lists a source that does not
exist and `demo.h` declares a function nobody implements. Neither has a safe
automatic answer, so both are reported instead of guessed at.

Reading the summary left to right: 4 files were analyzed, 4 have proposed
changes, none were written because this is `check`, 12 individual fixes passed
their proof gates, and 2 findings still need a human.

## Machine-readable

```console
$ normfix check --format json
{
  "schema_version": 2,
  "tool_version": "1.3.1",
  "mode": "check",
  "summary": {
    "files": 4,
    "changed": 4,
    "written": 0,
    "fixes": 12,
    "remaining": 2,
    "advisories": 0,
    "failed": 0,
    "unexpected_files": 0,
    "quarantine_candidates": 0,
    "quarantined": 0
  },
  "evaluation": null
}
```

Always branch on `schema_version` before reading the rest. The human output is
free to improve between releases; the JSON structure is not.

## Use it as a gate

```sh
normfix check --format json > report.json || exit 1
```

Exit code `1` here means "there is work to do", which is exactly what a
pre-merge check wants.
