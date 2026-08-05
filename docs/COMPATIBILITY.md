# Compatibility policy

This document defines what `normfix` considers supported. It is intentionally
narrow: compatibility claims participate in the safety model and must be
backed by automated evidence.

## Official Norminette

The supported checker is exactly Norminette `3.3.59`.

`normfix` verifies the executable version before analysis and refuses a
different release. This is stricter than a minimum-version check because the
official diagnostic names, locations, parser behavior, and accepted layouts
are inputs to the before/after regression proof. Silently accepting a newer
checker could authorize an edit under rules the formatter has never tested.

Norminette remains an external dependency. Release archives contain the native
`normfix` binary, not Python or the official checker.

### Adopting another checker release

A Norminette update requires one reviewed change that:

1. records upstream release notes and rule-name changes;
2. runs the complete native suite against the candidate version;
3. refreshes official-output fixtures only after explaining every difference;
4. verifies safe-fix idempotence and no-regression behavior on representative
   42 projects;
5. updates the exact version constant, CI installation, README, and this file;
6. ships as a new `normfix` version.

Supporting a range is appropriate only after CI proves every version in that
range and the oracle has an explicit adapter for any protocol difference.

## Rust toolchain

- Minimum supported Rust version (MSRV): `1.85`.
- Repository and release toolchain: `1.97.1`, pinned in
  `rust-toolchain.toml`.

CI checks the MSRV independently from the pinned development toolchain. Raising
the MSRV requires a documented release change rather than an incidental
dependency update.

## Operating systems and release targets

Prebuilt releases cover the Unix environments used by 42 students:

| Operating system | Architecture | Public release archive |
|---|---|---|
| Linux | x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux | ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS | Intel | `normfix-x86_64-macos.tar.gz` |
| macOS | Apple Silicon | `normfix-aarch64-macos.tar.gz` |

Public archive names deliberately omit Rust vendor placeholders and
machine-vendor labels. Toolchain target identifiers remain internal build
inputs, not release or product names.

Windows has no native release target. The full CLI is supported on Windows
through WSL using the Linux archive and a Norminette installation inside the
same WSL environment. The browser playground is the no-install alternative
for formatter previews. Native PowerShell/CMD execution is unsupported: the
bounded subprocess termination, symlink/path behavior, and transaction proofs
currently have Unix-specific implementation and integration evidence, so a
native Windows binary would overstate the contract.

## C and build diagnostics

The official Norminette is the style compatibility authority. A system C
compiler runs by default as a separate diagnostics-only oracle for
`-fsyntax-only -Wall -Wextra -Werror`. Inferred header-directory include paths
do not replace a project's own Makefile flags, defines, generated inputs,
language mode, linker inputs, or runtime tests.

GCC `-fanalyzer` is opt-in. Its allocation-lifetime and control-flow findings
can suggest a possible leak or invalid access, but they are not proof that
arbitrary C behavior is correct or that a project is leak-free.

`normfix preflight` does not execute Make recipes, link a binary, run the
program/tests, or invoke a runtime leak checker. It reports these remaining
manual steps explicitly.

## Browser compatibility

The playground targets modern browsers with standard WebAssembly and ES module
support. Its deliberately small, old-school HTML/CSS/TypeScript interface is
built as a static site with pinned Vite 8.1.5 and can be served locally or by
Vercel. Its compatibility contract is the in-memory native
formatter/diagnostic subset described in [`web/README.md`](../web/README.md).
It does not bundle or emulate Norminette, a compiler, Git, filesystem
transactions, or official-header identity logic.

## Report compatibility

The human interface groups diagnostics for readability and may improve between
releases. Automation should use `--format json` and check `schema_version`;
JSON retains individual findings. An incompatible JSON structure requires a
schema-version increment and compatibility notes.

## What versioning covers

`normfix` follows Semantic Versioning. The version number describes the
following surfaces, and only these:

| Surface | Covered | What a breaking change means |
|---|---|---|
| Command-line flags and subcommands | yes | Removing or renaming one, or changing what an existing one does |
| Exit codes | yes | Changing the meaning of `0`, `1`, `2`, or `130` |
| JSON report structure | yes, through `schema_version` | Removing or retyping a field |
| Configuration files (`normfix.toml`, `.normfixignore`) | yes | Changing how an existing key or pattern is interpreted |
| Backup, journal, and quarantine layout | yes | Making an older recovery point unreadable by `undo` |
| Which sources are edited automatically | no | New proven edits arrive in minor releases |
| Diagnostic wording, grouping, and help text | no | Improved continuously |
| Rust crate APIs | no | Every crate sets `publish = false` and is internal |
| The supported Norminette version | separate | Changing it is a documented release change, never incidental |

A new automatic edit is a minor release, because a formatter whose output never
changed would not be worth running. A run that produces a *worse* official
result is a bug in any version, and the differential test exists to catch
exactly that.

The minimum supported Rust version is a release decision, not a build detail.
Raising it requires a documented change; a dependency that needs a newer
compiler is held back instead.
