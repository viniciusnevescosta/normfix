# `normfix undo`

Restores a previous run from its external backup, and refuses to overwrite
anything that changed since.

```sh
normfix undo --list
normfix undo
normfix undo --run run-1785950998077000000-53423
```

## Find a recovery point

```console
$ normfix undo --list
normfix undo: 1 recovery point(s)
  run-1785950998077000000-53423  1 file(s)
```

Each run keeps the exact original bytes and a `journal.json` proving which
files it wrote and what it wrote to them.

## Reading it from a script

```sh
normfix --format json undo --list
```

```json
{
  "schema_version": 2,
  "command": "undo",
  "outcome": "success",
  "result": {
    "count": 2,
    "recovery_points": []
  }
}
```

`count` of zero means there is nothing to restore. That is a different answer
from a failure, which arrives with `outcome: "failure"` and an `error` object,
and the two need opposite responses from whatever is calling.

## Restore

With no `--run`, `undo` selects the newest intact recovery point and asks for
confirmation. Non-interactive restoration requires `--force`:

```sh
normfix undo --force
```

## When it refuses

`undo` fails closed. It will not restore when:

- a target file no longer matches the bytes that run wrote, because someone edited it
  afterwards, and restoring would silently discard that work;
- a backup file is missing or its hash does not match the journal;
- any path in the backup or the project resolves through a symbolic link;
- the journal is unreadable or its schema is unknown.

A refusal names the file and the reason. That is deliberate: a recovery tool
that guesses is worse than one that stops.

## What is not covered

`--no-backup` runs leave nothing to restore, which is the tradeoff for skipping
backups. Destructive operations always keep recovery storage regardless, so a
quarantined file or a removed comment can be recovered even when `--no-backup`
was passed.
