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
WOULD FIX    17          0     0  math_utils.c

Summary: 1 files | 1 proposed | 0 written | 17 fixes | 0 remaining | 0 info
Completed in 0.62 s.
```

`WOULD FIX` and `1 proposed` are the difference from [`lint`](/commands/lint):
check plans the edits and tells you how many passed the proof gates, it just
does not commit them.

## Machine-readable

```console
$ normfix check --format json
{
  "schema_version": 1,
  "tool_version": "0.4.0-beta.4",
  "mode": "check",
  "summary": {
    "files": 1,
    "changed": 1,
    "written": 0,
    "fixes": 17,
    "remaining": 0,
    "advisories": 0,
    "failed": 0,
    "unexpected_files": 0,
    "quarantine_candidates": 0,
    "quarantined": 0
  }
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
