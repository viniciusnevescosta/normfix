# Reporting, exit codes, and performance

## Reading a diagnostic

Every diagnostic is shown against the code it is about, so you can go straight
to the line instead of looking a coordinate up:

```text
error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
  --> srcs/sort/sort.c:30:3
   |
30 |         sort_medium(ctx);
   |         ^^^^^^^^^^^ call to undeclared function 'sort_medium'
   |
  ::: srcs/sort/sort_adaptive.c:21:3
   |
21 |         sort_medium(ctx);
   |         ^^^^^^^^^^^ call to undeclared function 'sort_medium'
   |
   = help: Fix this strict -Wall/-Wextra/-Werror compiler diagnostic, then rerun normfix.
   = source: C compiler
   = explain: normfix explain CC_IMPLICIT_FUNCTION_DECLARATION
```

The carets span the exact bytes the rule is about, not only its first
character. Occurrences of one rule are grouped under one heading, each labelled
with its own message, and the shared help, notes, origin, and `explain` hint
are stated once for the group instead of repeated under every occurrence.

The default view shows the first three occurrences of a rule and says how many
it held back, because a project can carry thousands of one diagnostic.
`--verbose` shows every one, each in its own section with its own snippet.

A few details worth knowing:

- Tabs are expanded, so the caret lands under the right character.
- Control characters in your source are shown as visible pictures and never
  reach the terminal as controls.
- The column in the `-->` line is counted in characters, the convention a C
  compiler uses. The official Norminette counts display columns instead, so its
  own output can name a larger column for the same character on a tab-indented
  line. The caret is the authoritative answer to *where*.
- A compiler diagnostic that belongs to a file without a position inside it,
  usually because the real location is in an included header, names the file
  and the header rather than drawing a caret on unrelated code.

The rendering uses [`annotate-snippets`], the library `rustc` renders its own
diagnostics with.

[`annotate-snippets`]: https://crates.io/crates/annotate-snippets

## The rest of the output

- a per-file status table: `CLEAN`, `INFO`, `FIXED`, `WOULD FIX`, `REVIEW`, or
  `FAILED`;
- stable rule IDs, shared help, notes, diagnostic origin, and a
  `normfix explain RULE` hint;
- optional accepted-fix details with `--verbose`;
- unified diffs with `--diff`;
- aggregate counts and elapsed wall time.

Color is enabled only for an interactive stdout. `--no-color`, `NO_COLOR`, JSON
output, and redirected output are color-free. Snippets are rendered against a
fixed width, so one report reads the same way on two machines.

Before discovery, human mode writes a compact `normfix · starting` block to
`stderr` with the action, effective scope, write/check mode, identity source,
checker policy, workers, cache, compiler check, backups, and requested
destructive capabilities. This makes an accidental root or home-directory run
obvious before work begins. Those protected scopes refuse without `--force`.

JSON mode instead writes one `execution_start` event object to `stderr`. The
versioned final report remains the only JSON document on `stdout`, so existing
automation can continue to parse it as one value.

`--format json` emits a deterministic, pretty-printed schema with
`schema_version: 2`. It includes identity metadata, discovery and quarantine
outcomes, per-file change/write/failure fields, fixes, before/after diagnostics,
summary counts, optional preflight `evaluation`, and `duration_seconds`. Source
buffers and unified diffs are intentionally omitted.

`normfix preflight` adds a deterministic, explicitly non-conclusive estimate:
`score`, `grade`, `verdict`, and exactly located `hard_failures`. The verdict is
`hard_fail` when the evaluated scope contains an unexpected file, a finding
corroborated by the installed official Norminette, or a Makefile diagnostic.
Norminette and Makefile evidence comes from the original on-disk snapshot, plus
any additional failure exposed in the final shadow. Therefore an auto-fixable
problem remains a preflight hard fail until the proposed bytes are actually
written and checked again.
The numeric score is a bounded prioritization heuristic, not a 42 grade; it
cannot cover runtime behavior, project-specific tests, leaks, peer judgment,
or defense questions.

### Exit codes

| Code | Meaning |
|---:|---|
| `0` | Fix mode completed with no blocking diagnostic, or the input was already clean |
| `1` | Manual diagnostics remain, preview mode found proposed changes/quarantine candidates, or preflight matched a hard-fail rule |
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
