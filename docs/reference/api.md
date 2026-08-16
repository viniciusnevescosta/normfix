# The JSON API

Everything `normfix` can be asked, and exactly what comes back. This page is
for whoever is reading the answer with a program — a script in CI, an agent
deciding what to do next — rather than watching a terminal.

Every command accepts `--format json`, and every one of them answers on
standard output. Prose belongs on standard error so a person can follow a run;
a result you have to fish out of the diagnostic stream is not an interface.

## The two shapes

Commands that inspect a project — `format`, `lint`, `check`, `budget`,
`preflight` — answer with the **run report**. Everything else answers with a
**command envelope**. Both open with `schema_version`, and both name the
command that produced them.

Branch on `schema_version` first. Everything below can gain fields within a
version; nothing is removed or retyped without incrementing it.

## The run report

```sh
normfix --format json check src
```

```json
{
  "schema_version": 2,
  "tool_version": "1.9.0",
  "command": "check",
  "mode": "check",
  "scope": { "selection": "explicit_paths", "respects_gitignore": false },
  "granted_capabilities": [],
  "identity": { "login": "vneves-c", "email": "vneves-c@student.42.fr", "source": "user config" },
  "discovery_errors": [],
  "unexpected_files": [],
  "quarantine_candidates": [],
  "quarantined_files": [],
  "files": [
    {
      "path": "src/main.c",
      "changed": true,
      "written": false,
      "backup": null,
      "failure": null,
      "fixes": [
        { "rule_id": "SPLIT_DECLARATION_ASSIGNMENT", "description": "separated a declaration from its value", "line": 5, "count": 1 }
      ],
      "before": [],
      "after": [
        {
          "rule_id": "TOO_MANY_LINES",
          "path": "src/main.c",
          "range": { "start": 120, "end": 126 },
          "severity": "warning",
          "message": "process() has 31 body line(s); the limit is 25.",
          "source": "Norm v4.1 native rule",
          "notes": [],
          "help": "Extract a cohesive region into a helper you name."
        }
      ]
    }
  ],
  "summary": {
    "files": 1, "changed": 1, "written": 0, "fixes": 1, "remaining": 1,
    "advisories": 0, "failed": 0, "unexpected_files": 0,
    "quarantine_candidates": 0, "quarantined": 0
  },
  "duration_seconds": 0.31
}
```

`files` holds **every** file the run analysed, whatever the command — one entry
per file, under the path the run used. A `budget` run adds its own array to
each entry; a `preflight` run adds `evaluation` at the top level.

| Field | What it is |
|---|---|
| `command` | which command produced this: `format`, `lint`, `check`, `budget`, `preflight`, or `diff` |
| `mode` | whether it wrote, checked, or diffed |
| `scope.selection` | `git_changed`, `git_staged`, `explicit_paths`, or `working_directory` |
| `scope.respects_gitignore` | whether discovery honoured `.gitignore` |
| `granted_capabilities` | what the run was allowed to do; present and empty on an ordinary run |
| `files[].changed` | the proposed bytes differ from the original |
| `files[].written` | those bytes reached the disk |
| `files[].failure` | an operational failure, distinct from a finding |
| `files[].before` / `after` | diagnostics against the original and the proposed bytes |
| `files[].diff` | present only with `--diff` |
| `files[].budget` | present only for `budget` |

Source buffers are never included. They would multiply the payload by the size
of the project, and a caller that wants the bytes already has the file.

### `budget` adds each function's headroom

```sh
normfix --format json budget src
```

```json
{
  "command": "budget",
  "files": [
    {
      "path": "src/main.c",
      "budget": [
        {
          "function": "process",
          "line": 12,
          "lines": 31, "line_limit": 25,
          "variables": 3, "variable_limit": 5,
          "parameters": 2, "parameter_limit": 4
        }
      ]
    }
  ]
}
```

The same numbers appear in the `NORM_BUDGET` sentence a person reads. They are
here as fields so nothing has to take that sentence apart.

### `preflight` adds a deliberately non-conclusive estimate

```json
{
  "command": "preflight",
  "evaluation": {
    "conclusive": false,
    "score": 59,
    "grade": "fail",
    "verdict": "hard_fail",
    "hard_failures": [
      { "rule_id": "MAKEFILE_SOURCE_NOT_FOUND", "path": "Makefile", "line": 14, "column": 20,
        "message": "The literal Makefile source `ghost.c` does not exist below the project root." }
    ],
    "notes": []
  }
}
```

`conclusive` is always `false`. The score is a bounded prioritisation
heuristic, not a 42 grade: it cannot see runtime behaviour, subject-specific
tests, leaks, peer judgement, or defence questions.

## The command envelope

```sh
normfix --format json upgrade --check
```

```json
{
  "schema_version": 2,
  "command": "upgrade",
  "outcome": "success",
  "result": {
    "state": "available",
    "current_version": "1.9.0",
    "latest_version": "1.9.0",
    "installed": false
  }
}
```

`outcome` is `success`, `planned` for a dry run, `findings` when a check found
something, or `failure`. Read it rather than inferring from which fields are
present.

| Command | `result` holds |
|---|---|
| `explain` | `rule_id` — the canonical name, which an alias resolves to — and `explanation` |
| `undo` | `recovery_points` in run order, and their `count` |
| `upgrade` | `state` (`current`, `available`, `installed`), both versions, and `installed` |
| `uninstall` | `dry_run`, `purge`, `removes_recovery_data`, and the `plan` as text |
| `leaks` | the checker's totals, its allocation `sites`, and its `errors` |

### `leaks`

```sh
normfix --format json leaks --force ./push_swap
```

```json
{
  "schema_version": 2,
  "command": "leaks",
  "outcome": "findings",
  "result": {
    "program_exit_code": 0,
    "definitely_lost_bytes": 1024,
    "indirectly_lost_bytes": 96,
    "still_reachable_bytes": 0,
    "error_count": 2,
    "sites": [
      { "bytes": 1024, "indirect": false, "function": "create_stack",
        "location": { "file": "stack.c", "line": 23 } }
    ],
    "errors": [
      { "kind": "Invalid read of size 4", "function": "sort_stack",
        "location": { "file": "sort.c", "line": 41 } }
    ]
  }
}
```

`sites` says where lost memory was allocated; `errors` says which line touched
memory the program did not own. `location` is absent when the binary carries no
debug information. `outcome` is `findings` when anything was lost, so nothing
has to compare byte counts to decide what happened.

## Failure

```json
{
  "schema_version": 2,
  "outcome": "failure",
  "error": { "code": "run_error", "message": "…" }
}
```

A refused flag combination, an unreachable checker, an unreadable file: all of
them arrive here. One shape, one field to read.

## Which flag shows up where

| Flag | Field |
|---|---|
| `--check`, `--diff` | `mode` |
| `--diff` | `files[].diff` |
| `--changed`, `--staged` | `scope.selection` |
| `--use-gitignore` | `scope.respects_gitignore` |
| `--unsafe`, `--force`, `--remove-unused`, `--remove-unexpected`, `--remove-invalid-comments` | `granted_capabilities` |

### Rule identifiers carry their source

A `rule_id` says which authority produced the finding, so a caller can weigh it
without reading the prose.

| Prefix | Who said it |
|---|---|
| no prefix | the official Norminette, or a native Norm rule |
| `CC_` | the C compiler under `-Wall -Wextra -Werror` |
| `CC_ANALYZER_` | the compiler's own deep analyzer |
| `TIDY_` | `clang-tidy`, when the machine running normfix has one |
| `MAKEFILE_`, `HEADER_`, `PREFLIGHT_` | normfix's own project checks |

Everything from a lens — `CC_ANALYZER_` and `TIDY_` — arrives with severity
`info`. A lens reports a judgement about how a program behaves rather than a
fact about its text, so it counts under `advisories`, never under `remaining`,
and it never authorizes an edit. Branch on `severity` rather than on the prefix.

`--threads`, `--timeout`, `--no-cache`, `--norminette`, `--cc`, and
`--clang-tidy` leave no
trace, on purpose. They change how a run reaches its answer, not what the
answer covers, and a caller reading a result should not have to care which were
used to produce it.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | nothing left to report |
| `1` | findings remain, or a leak was observed |
| `2` | the run could not be completed |
| `130` | interrupted |

An agent should branch on the exit code first and read the payload second: a
`2` means the answer describes a failure, not a project.

Running normfix without a person watching has its own page:
[AI agent guide](/guide/ai-agents).
