# Compatibility policy

This document defines what `normfix` considers supported. It is intentionally
narrow: compatibility claims participate in the safety model and must be
backed by automated evidence.

## Official Norminette

The tested checker is the
[official Norminette](https://github.com/42School/norminette) `3.3.59`.

`normfix` fingerprints the executable version before analysis. A different
release continues with a prominent `NORMINETTE_VERSION_UNTESTED` advisory by
default; `--strict-norminette-version` refuses it for pinned CI. This is not a
minimum-version compatibility claim because official diagnostic names,
locations, parser behavior, and accepted layouts are inputs to the native
compatibility layer. The warning makes that reduced assurance explicit.

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

### When 42 moves first

A tool that refuses every release but one stops working for everyone on the day
the school upgrades. The default therefore continues and reports
`NORMINETTE_VERSION_UNTESTED`; pinned CI can opt into refusal:

```sh
normfix --strict-norminette-version
```

The default behavior is defensible rather than a hole in the argument,
because the property the tool actually promises does not depend on knowing the
version: the before/after regression proof compares two answers from **the same
executable**, so a run still cannot leave a file with more official diagnostics
than it started with. What an unverified release costs is the guarantee that
the native rules agree with it, which is exactly what the warning says.

## Rust toolchain

- Minimum supported [Rust](https://www.rust-lang.org/tools/install) version
  (MSRV): `1.85`.
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
| Windows | x86-64 | `normfix-x86_64-windows.zip` |
| Windows | ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD | x86-64 | `normfix-x86_64-freebsd.tar.gz` |

Public archive names deliberately omit Rust vendor placeholders and
machine-vendor labels. Toolchain target identifiers remain internal build
inputs, not release or product names.

Windows is supported natively as of 1.4.0, on the evidence CI produces for it
rather than on the assumption that portable code ports. Both Windows targets
run the complete test suite, drive the real official Norminette, and prove the
differential property — that a run never leaves a file with more official
diagnostics than it started with — on the platform itself.

Two differences from Unix are real, and are stated here rather than smoothed
over:

- **Process containment has a narrow window.** Unix places a tool in its own
  process group between fork and exec, so no descendant can ever escape.
  Windows has no pre-start hook: the tool is placed in a job object immediately
  after spawn, and anything it spawns in the microseconds before that could
  break away. The job kills the rest of the tree when it closes.
- **A rename is not written through.** POSIX requires flushing the parent
  directory for a create or rename to survive a crash, which the transaction
  does. Windows has no directory-flush counterpart; the file's own contents are
  flushed and NTFS journals the metadata, but a machine that loses power
  between a commit and the metadata reaching disk has a weaker guarantee than
  the same moment on Unix. The backup and journal are unaffected — recovery
  reads them by content, not by ordering.

Windows archives are `.zip`, which the platform opens by itself. The one-line
installer works from any POSIX shell there — Git Bash, MSYS2, Cygwin, or WSL.
Running the Linux build inside WSL remains supported and is unchanged.

FreeBSD x86-64 is supported on the same terms. It is a Unix, so it shares the
process-group containment and the directory flush rather than needing Windows'
substitutes, and CI runs the complete suite, the official checker, and the
differential proof inside a FreeBSD virtual machine — GitHub has no FreeBSD
runner, and cross-compiling would publish a binary that had never run on the
system it targets. Its release archive is built in that same virtual machine
for the same reason.

FreeBSD on ARM64 is not published. `aarch64-unknown-freebsd` has no prebuilt
standard library on the pinned toolchain, so building it would require an
unpinned nightly compiler, and there is no way to run the suite on it. Either
one alone would be enough to make the claim unsupportable.

## C and build diagnostics

The official Norminette is the style compatibility authority. A system C
compiler runs by default as a separate diagnostics-only oracle for
`-fsyntax-only -Wall -Wextra -Werror`. Inferred header-directory include paths
do not replace a project's own Makefile flags, defines, generated inputs,
language mode, linker inputs, or runtime tests.

GCC `-fanalyzer` is automatic in `preflight` and opt-in in ordinary workflows.
Its allocation-lifetime and control-flow findings
can suggest a possible leak or invalid access, but they are not proof that
arbitrary C behavior is correct or that a project is leak-free.

`normfix preflight` does not execute Make recipes, link a binary, or run the
program and its tests. It reports these remaining manual steps explicitly.

`normfix leaks` does run a program, and is the only command that does. It never
builds one — it runs a binary it is pointed at, under a leak checker located on
`PATH` and verified by its own `--version`. What it reports is what one run
observed on one path, never a proof that a program does not leak, and output it
cannot read as a leak summary is an error rather than a clean result. Valgrind
covers Linux and FreeBSD directly, macOS through a community port with limited
Apple Silicon support, and Windows through WSL.

## Browser compatibility

The playground targets modern browsers with standard WebAssembly and ES module
support. Its deliberately small, old-school HTML/CSS/TypeScript interface is
built as a static site with pinned
[Vite 8.2.1](https://vite.dev/releases) and can be served locally or by
Vercel. Its compatibility contract is the in-memory native
formatter/diagnostic subset described in [`web/README.md`](../web/README.md).
It can build an official header from an identity supplied to that browser tab,
and can preview C, headers, Makefiles, and Markdown. It does not bundle or
emulate Norminette, a compiler, Git, project-wide header-guard proofs, or
filesystem transactions.

## Report compatibility

The human interface groups diagnostics for readability and may improve between
releases. Automation should use `--format json` and check `schema_version`;
JSON retains individual findings. An incompatible JSON structure requires a
schema-version increment and compatibility notes.

One consequence is worth stating plainly: the line and column printed beside a
snippet follow the C compiler convention and count characters, while the
official Norminette counts display columns. The two disagree on a tab-indented
line. Neither number is part of the versioned surface, and the caret under the
source is what locates the finding. See
[Reporting](reference/reporting.md#reading-a-diagnostic).

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
