# Working with normfix as an AI agent

This repository contains the `normfix` implementation. If your task is to run
the installed tool on another 42 project, read
[`docs/guide/ai-agents.md`](docs/guide/ai-agents.md) first.

The safe default for automation is an explicit, read-only scope:

```sh
normfix check /absolute/path/to/project --format json --no-color
```

Never begin with bare `normfix`: the commandless form is a recursive formatting
run rooted at the current directory. Do not add `--unsafe`, `--force`, removal
flags, or `upgrade` unless the user explicitly authorized that capability.

When changing this repository:

- preserve the separation between analysis and filesystem writes;
- put new source transformations behind exact tests and the official
  before/after Norminette gate;
- keep diagnostics in English until the localization layer owns translation;
- update the command/reference documentation and `normfix explain` catalogue
  with every user-visible rule or flag;
- follow [`docs/LOCALIZATION.md`](docs/LOCALIZATION.md) for human text and
  never translate commands, flags, rule IDs, JSON keys, or configuration keys;
- run formatting, workspace tests, Clippy with warnings denied, rustdoc with
  warnings denied, the MSRV suite, and the site build before a release;
- do not split `crates/normfix-engine/src/pipeline.rs` before the 1.0.0 release.

Repository-wide contributor rules and proof requirements live in
[`CONTRIBUTING.md`](CONTRIBUTING.md) and
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
