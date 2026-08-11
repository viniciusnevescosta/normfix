# Contributing

The governing rule of this project is:

> Change what can be proven, explain what cannot, and never turn uncertainty
> into permission.

A change that makes `normfix` rewrite more code is only welcome together with
the proof that makes the rewrite safe. A change that turns an unprovable case
into a clear diagnostic is welcome on its own.

## Before you start

Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). It records what each crate
owns and, more usefully, the tradeoff accepted for every boundary. Most review
disagreements turn out to be disagreements with a decision recorded there.

For a new language or translated string, also read
[`docs/LOCALIZATION.md`](docs/LOCALIZATION.md). It defines which human surfaces
are translated and which command, rule, JSON, and configuration tokens must
remain stable.

## Setup

Building from source needs [Rust](https://www.rust-lang.org/tools/install)
1.85 or newer; the pinned toolchain in `rust-toolchain.toml` is installed for
you by the first command. Nobody
*using* normfix needs a toolchain, because the release archives ship a native
binary.

The [official Norminette](https://github.com/42School/norminette) remains the
compatibility authority used by the differential gate.

```sh
rustup show active-toolchain     # installs the pinned Rust toolchain
npm ci                           # the playground and documentation workspace
```

## The gates

Every one of these runs in CI and must pass locally before you open a pull
request:

```sh
cargo fmt --all --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
cargo +1.85.0 test --workspace --all-targets --locked
cargo deny --all-features check
cargo test --locked -p normfix-engine --test differential -- --ignored
npm audit --audit-level=moderate
npm run build
```

The differential gate needs the official Norminette on `PATH`. It is the gate
that matters most, because it asserts the claim the whole project rests on: a
run never leaves a file with more official diagnostics than it started with. It
also asserts that a second run changes nothing, and that a file that compiled
still compiles.

Benchmarks are not a gate, but run them when you touch the parser, the tape, or
the scheduler:

```sh
cargo bench -p normfix-c-actions
```

## The site

The playground and the documentation are one npm workspace, built together:

```sh
npm ci && npm run build   # playground into web/dist, documentation into web/dist/docs
npm run dev               # the playground
npm run docs:dev          # the documentation
```

The documentation build fails on a dead internal link, which is what actually
keeps the four locale trees honest: a page that links to a translation that
does not exist yet stops the build rather than shipping a 404.

## Adding a transformation

A new automatic edit needs all of:

1. **A containment argument.** State exactly what makes the edit safe, and what
   ends the region it may touch. Include ordering, for example, stops at the
   first line that is not exactly one include directive.
2. **A phase**, gated by an option when the edit is not universally desirable.
3. **The right applicability.** `SafeLayout` preserves the token-and-comment
   fingerprint, `SafeSemantic` changes tokens deliberately, and
   `UnsafeDestructive` requires an explicit capability grant.
4. **A fixpoint test.** Applying twice must equal applying once.
5. **A refusal test.** Prove the edit does *not* happen in the ambiguous case.

Prefer a property over an example when the invariant can be generated. The
tape and include-ordering tests show the pattern.

## Style

- English in code, comments, commit messages, and documentation.
- Explain *why* in comments; the code already says what.
- No `unsafe`. The workspace forbids it.
- Prefer failing closed. When a proof is incomplete, report and change nothing.

## Commits and pull requests

Commit messages are conventional-commit prefixed and describe the change in
terms of behavior, not files. Keep a pull request to one reviewable idea.

Anything that changes the CLI surface, the JSON report schema, the supported
Norminette version, or the MSRV also changes
[`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) and
[`CHANGELOG.md`](CHANGELOG.md) in the same pull request.
