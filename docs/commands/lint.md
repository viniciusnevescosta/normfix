# `normfix lint`

Reports what is wrong with the bytes currently on disk. It proposes nothing and
writes nothing: not formatting, not the official header, not Makefile or
README changes.

```sh
normfix lint
normfix lint src
```

Use it when you want the diagnosis without the treatment: in CI, in a review,
or when you intend to fix something by hand and do not want the tool to move
under you.

## What it reports

```console
$ normfix lint
warning[TOO_MANY_WS]: 2 occurrences in 1 file
  math_utils.c:1:1                     Extra whitespaces for indent level
  math_utils.c:2:1                     Extra whitespaces for indent level
 = help: Review this location and apply the named Norm rule manually; no
         semantics-preserving automatic edit was proven.
 = source: official Norminette 3.3.59 compatibility
 = explain: normfix explain TOO_MANY_WS

Summary: files: 1 | proposed: 0 | written: 0 | fixes: 0 | remaining: 14 | info: 0
```

Note `0 proposed`: lint never plans an edit. The same project under
[`check`](/commands/check) reports seventeen proposed fixes instead, because
check is allowed to plan them.

Diagnostics are grouped by rule and every location is kept. Each group names
its origin (the official Norminette, the C compiler, the native parser, or a
project rule) so you know which authority you are arguing with.

## In CI

```sh
normfix lint --format json > report.json
```

The JSON keeps individual findings and carries `schema_version`. Exit code `1`
means diagnostics remain, `0` means clean, `2` means the run itself failed.

## Reading it from a script

```sh
normfix --format json lint
```

```json
{
  "schema_version": 2,
  "tool_version": "1.6.2",
  "mode": "lint",
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
