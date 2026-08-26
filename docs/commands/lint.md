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

Because `lint` never plans edits, it rejects formatting, header-identity,
backup, diff, and removal flags with a command that does support the requested
operation. This prevents a green-looking invocation from silently ignoring a
setting. Use `normfix check --diff` when you want to see proposed changes.

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

Every field this command returns is documented in [the JSON API](/reference/api).
