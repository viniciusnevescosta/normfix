# Safety, recovery, and destructive operations

Projects with a subject-specific function allowlist can add `normfix.toml` at
the project root:

```toml
[project]
name = "get_next_line"
allowed = ["read", "malloc", "free"]
```

The bounded parser intentionally interprets only the relevant quoted `name`
and quoted-identifier `allowed` array. When a C/header scope is selected,
`normfix` independently discovers the complete project C/header set from the
project root, considering regular files without following symbolic links and
with `.gitignore`, `.normfixignore`, and `.norminetteignore` filtering disabled.
Every discovered file must be readable UTF-8 and parse losslessly. Non-`static`
definitions from that closed snapshot authorize calls across translation
units; same-file definitions are handled locally, while a `static` definition
in another file never authorizes the call.

Call candidates are recomputed against the final shadow source so reported
ranges remain correct after header insertion and formatting. Parameters,
function-pointer calls, macro/preprocessor ambiguity, and uppercase macro-like
identifiers fail closed instead of producing a guess. If discovery, reading,
parsing, losslessness, or snapshot revalidation is incomplete, all allowlist
findings are disabled and `FUNCTION_POLICY_PROOF_INCOMPLETE` explains why.
`normfix.toml` itself must be a bounded, non-symlink regular file. The policy
still does not replace the project subject or evaluator.

## Comments and destructive capabilities

Comments rejected as `WRONG_SCOPE_COMMENT` or `COMMENT_ON_INSTR` are reported
by default. `--remove-invalid-comments` deletes only a comment found at the
exact line and display column reported by the official checker. It never
removes the official header, and the remaining code-token fingerprint must be
unchanged.

`--remove-unused` and `--remove-unexpected` request stronger destructive
capabilities:

- unused-function removal considers only `static` definitions;
- it requires the selected inputs to equal the complete `.c`/`.h` project set;
- parser recovery, unknown bytes, preprocessor ambiguity, token pasting,
  attributes, string-based references, duplicate definitions, or an uncertain
  reference graph preserve the function;
- unexpected-file removal is a recoverable quarantine operation, never an
  extension-based permanent deletion.

In an interactive human run, these capabilities require a `y/N` confirmation
before analysis. The prompt grants only the requested capability; each
candidate must still pass its parser, hash, scope, and transaction proofs.
Answering yes does not weaken any proof.

JSON and other non-interactive runs require `--force`:

```sh
normfix --remove-unused --force
normfix --remove-unexpected --force
normfix --unsafe --force
```

`--unsafe` is a closed shorthand for six implemented operations:

- exact-location invalid-comment removal;
- compacting simple `NULL` comparisons only when the dedicated C shape is
  proven;
- removal of proven-missing or trivia-only tokens from simple literal Makefile
  source lists;
- removal of project-local header prototypes only when a complete lossless
  source proof finds neither an implementation nor any use/ambiguity;
- unreachable-`static` removal under a closed-source proof;
- unexpected-file quarantine.

Prototype implementation warnings themselves are enabled in normal runs.
Unsafe mode can remove a missing, unused declaration after the complete proof;
it never removes an existing trivia-only definition or its prototype because a
no-op body may be intentional.

It does not enable arbitrary edits. Comment removal can also be requested
alone with `--remove-invalid-comments`; the other destructive plans still
require capability authorization.

Use preview mode before a destructive run:

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

Preview modes require the same interactive authorization because the
closed-world planners themselves are capability-gated, but they do not write,
delete, or move project files.

## Backups, transactions, and recovery

Default source backups are external to the scanned project:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

On Unix without `XDG_DATA_HOME`, the fallback is:

```text
~/.local/share/normfix/backups/<run-id>/
```

Each backed-up transaction includes exact original bytes and `journal.json`.
Before the first target changes, the writer:

- canonicalizes the project boundary;
- rejects duplicate, external, symbolic-link, and non-regular targets;
- confirms every current file still matches the analyzed bytes;
- writes external backups;
- stages and synchronizes every replacement.

Targets are committed in sorted path order. A mid-commit error triggers
best-effort rollback from the captured original bytes; an incomplete rollback
is reported with the recovery journal path.

`--no-backup` applies only to ordinary safe formatting. A source deletion
planned by invalid-comment removal, Makefile source reconciliation, orphan
prototype removal, or unreachable-`static` removal requires external recovery
storage and fails closed if it is unavailable.

Quarantine always retains a recoverable external copy, including when
`--no-backup` was supplied:

```text
<backup-base>/quarantine/<run-id>/<original-relative-path>
```

The source type, byte length, and BLAKE3 hash are rechecked immediately before
the move. Existing recovery destinations are never overwritten. A partial
quarantine failure attempts to restore files already moved.
