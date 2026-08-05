# Reporting, exit codes, and performance

Human output includes:

- a per-file status table: `CLEAN`, `INFO`, `FIXED`, `WOULD FIX`, `REVIEW`, or
  `FAILED`;
- exact `path:line:display-column` locations;
- grouped rule/severity/source sections, with every affected location and
  message retained;
- source snippets and carets in `--verbose` output, with tabs expanded to
  four-column stops;
- stable rule IDs, shared help, notes, diagnostic origin, and an
  `normfix explain RULE` hint;
- optional accepted-fix details with `--verbose`;
- unified diffs with `--diff`;
- aggregate counts and elapsed wall time.

Color is enabled only for an interactive stdout. `--no-color`, `NO_COLOR`, JSON
output, and redirected output are color-free.

`--format json` emits a deterministic, pretty-printed schema with
`schema_version: 1`. It includes identity metadata, discovery and quarantine
outcomes, per-file change/write/failure fields, fixes, before/after diagnostics,
summary counts, and `duration_seconds`. Source buffers and unified diffs are
intentionally omitted.

### Exit codes

| Code | Meaning |
|---:|---|
| `0` | Fix mode completed with no blocking diagnostic, or the input was already clean |
| `1` | Manual diagnostics remain, or preview mode found proposed changes/quarantine candidates |
| `2` | Discovery, configuration, tool, I/O, transaction, or quarantine failure |
| `130` | An interactive per-file review was cancelled |

Informational advisories do not make a run fail.

## Cache and performance

File analysis runs in parallel through Rayon. `--threads N` creates a local
pool with an exact worker count; without it, Rayon uses the available hardware.
Results and commits are sorted by path, so worker completion order does not
change report order or write order.

Official Norminette reports use both an in-memory run cache and a persistent
redb database outside the project. On Unix:

```text
$XDG_CACHE_HOME/normfix/<project-id>/cache-v1.redb
```

or:

```text
~/.cache/normfix/<project-id>/cache-v1.redb
```

Keys include the schema, analysis namespace, project-relative path when the
input is inside the run root (absolute-path fallback for an explicit external
input), source bytes, Norm configuration, and the verified executable
fingerprint. Cache lock, I/O, decoding, or corruption failures fail open as
misses; they never change diagnostics or exit status. A corrupt database is
preserved under a `.corrupt-N` name before recreation.

Use `--no-cache` for a fully uncached run.
