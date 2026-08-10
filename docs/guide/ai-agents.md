# Using normfix from an AI agent

This page is the operational contract for coding agents, editor agents, CI
bots, and other non-human callers. It keeps an agent from accidentally turning
a status check into a recursive write.

## The one rule to remember

The bare command formats the current directory recursively:

```sh
normfix
```

An agent should therefore start with an explicit path and a read-only command:

```sh
normfix check /absolute/path/to/project --format json --no-color
```

Use an absolute project path. Do not rely on an inherited working directory,
especially when the agent may have started in a home directory, checkout
parent, mounted workspace root, or system directory.

## Capability check

Before the first project run, record the tool and checker versions:

```sh
normfix --version
norminette --version
normfix --help
```

`normfix` fingerprints every checker. When 42 publishes a different release,
the default run continues and emits `NORMINETTE_VERSION_UNTESTED`; an agent must
surface that reduced assurance. Use `--strict-norminette-version` only when the
user or CI policy explicitly requires the tested checker release.

At startup, human mode writes a color-free action/configuration block to
`stderr`. JSON mode writes one `execution_start` JSON event to `stderr` and
keeps the versioned final report as the only JSON document on `stdout`. Neither
mode prompts when stdin is non-interactive.

Read the scope out of that event before doing anything with the result. It is
the run's own statement of what it was about to touch, so an agent can abort a
run whose scope does not match the task it was given, instead of discovering
the mismatch in the summary.

A broad or operating-system-sensitive scope is refused before any file is read:

```console
$ normfix check /
normfix
error: refusing to scan or modify protected scope `/` because it is a filesystem root; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.
```

That is exit `2` with no JSON report on `stdout`. Filesystem roots, complete
home directories, operating-system trees, and broad multi-project directories
all refuse this way, and the check resolves symbolic links and `..` first. Do
not add `--force` to make the message go away: the refusal almost always means
the scope was computed wrong, and `--force` is a decision for the user to make
about a path they have inspected.

The regular formatter does not need Rust. A compiler is used only for advisory
preflight checks; its findings never authorize an edit.

## Recommended agent workflow

1. Inspect repository state and resolve any merge conflict before formatting.
2. Run a machine-readable preview against an explicit scope.
3. Read `schema_version` before consuming fields from the JSON report.
4. Show the user the proposed files, remaining diagnostics, and any operational
   failures.
5. If writes are already authorized, run the same explicit scope with
   `normfix format`.
6. Inspect the resulting diff and run the project's own build/tests.
7. Run `normfix check` again. A successful second pass should propose no edit.

```sh
project=/absolute/path/to/project
normfix check "$project" --format json --no-color > normfix-report.json
normfix format "$project" --no-color
git -C "$project" diff --check
normfix check "$project" --format json --no-color
```

Do not create `normfix-report.json` inside a 42 submission directory unless the
user wants it there: an unexpected file is itself an evaluation finding. Use a
temporary or agent-owned output directory instead.

## Reading the JSON contract

The stable report currently uses `schema_version: 2`. Useful fields are:

| Field | Agent decision |
|---|---|
| `summary.changed` | A preview found byte changes it can prove safe |
| `summary.remaining` | Manual/blocking findings remain |
| `summary.failed` | A tool, discovery, I/O, or transaction operation failed |
| `summary.unexpected_files` | Files outside the accepted project file set were found |
| `files[].failure` | This file was not completed; do not describe it as fixed |
| `files[].after` | Diagnostics against the final shadow buffer |
| `files[].fixes` | Proven edits proposed or written for that file |
| `identity.available` | An official 42 header can be created or updated |
| `evaluation.conclusive` | Always `false`; never present the estimate as an official grade |
| `evaluation.verdict` | `hard_fail` means an objective preflight rejection rule matched |
| `evaluation.hard_failures` | Exact path/line/column/rule evidence to surface first |

Source buffers and diffs are intentionally absent from JSON. Use `normfix
--diff /absolute/path` when a human-readable patch is needed.

Exit status is part of the API:

| Code | Meaning |
|---:|---|
| `0` | Clean, or a write completed with no blocking issue |
| `1` | A preview found work or a manual finding remains |
| `2` | The run itself failed |
| `130` | A human cancelled interactive review |

Exit `1` is not an operational crash. Exit `2` must never be hidden behind a
claim that the project passed.

## Choosing a command

| Goal | Command |
|---|---|
| Exact preview | `normfix --diff PATH` |
| Machine gate | `normfix check PATH --format json --no-color` |
| Diagnose bytes without edits | `normfix lint PATH --format json --no-color` |
| Pre-defense review | `normfix preflight PATH --format json --no-color` |
| Function headroom | `normfix budget PATH --format json --no-color` |
| Explain a rule offline | `normfix explain RULE` |
| Format an authorized scope | `normfix format PATH --no-color` |
| Restore a normfix transaction | `normfix undo --list`, then `normfix undo --run ID` |

`--changed` and `--staged` are convenient for a developer's own worktree, but
they select names through Git and analyze working-tree bytes. Use an explicit
path for a complete evaluation and a Git scope for a focused edit.

## Authority and destructive flags

These options request materially different capabilities:

- `--remove-invalid-comments` deletes only comments rejected at exact official
  locations;
- `--remove-unused` removes only unreachable `static` functions under a closed
  project proof;
- `--remove-unexpected` moves files to external recoverable quarantine;
- `--unsafe` enables the documented closed set of destructive cleanups;
- `--force` supplies non-interactive confirmation for those capabilities.

An agent must not infer permission for them from a request to check, format,
evaluate, or "fix Norm errors." Previewing a destructive plan also requires
the capability because the analysis is intentionally authorization-gated.

Never delete backup or quarantine data to make a report look clean. Use
`normfix undo` for recovery, and report the journal path if rollback needs
manual review.

## Evaluation limits

`preflight` combines the official Norm result, project-file checks, strict
compiler diagnostics, policy checks, and an automatic bounded compiler-analyzer
pass. It is a
strong review aid, not a conclusive 42 grade. It does not know the subject PDF,
execute a defense checklist, prove algorithmic correctness, or prove the
absence of leaks. It does not execute Make recipes, a produced binary,
`clang-tidy`, or sanitizers. Run the project's own Makefile, tests, sanitizer
build, and subject-specific tester separately only when the user authorizes
execution of that project.

Do not treat the presence or absence of a README as a universal pass/fail rule.
When one exists, verify it against the current subject's required sections.
Likewise, `MAKEFILE_NOT_FOUND` is advisory until subject policy proves that a
Makefile is required. Do not report a proposed shadow fix as a preflight pass:
the evaluation hard-fails original on-disk Norminette and Makefile findings.

## Terminal and CI hygiene

- Prefer `--format json --no-color` for parsers and redirected output.
- Never parse the decorative human table when JSON is available.
- Set `NORMFIX_NO_UPDATE_CHECK=1` in hermetic or network-free CI.
- Keep the official checker and `normfix` versions in CI logs.
- Do not pipe a write command through a filter that hides its exit status.
- Do not run against `/`, `/System`, `/usr`, `/etc`, a home directory, or a
  workspace containing several projects. Select the actual submission root.

For every option and proof boundary, continue with [Every flag](/reference/flags),
[Safety and recovery](/reference/safety), [Reporting](/reference/reporting), and
[Architecture](/ARCHITECTURE).
