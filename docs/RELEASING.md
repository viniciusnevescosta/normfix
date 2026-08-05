# Release process

`normfix` releases are built by GitHub Actions from an annotated version tag.
The workflow is deliberately tag-gated so an ordinary push cannot publish
external artifacts.

## Preconditions

Before tagging:

1. the working tree is clean and the intended commit is on `main`;
2. the workspace version in `Cargo.toml` is the intended release version;
3. `Cargo.lock`, README examples, and compatibility documentation agree with
   that version;
4. the complete CI workflow is green, including MSRV, official Norminette, and
   system-compiler smoke tests;
5. the browser/WASM crate and local playground smoke test are green;
6. real-project fixture changes have been reviewed as source diffs, not only as
   exit codes.

Run the same local quality gate used by CI:

```sh
cargo fmt --all --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
```

## Tag and publish

Create a tag whose text is exactly the workspace version prefixed by `v`:

```sh
git tag -a v0.4.0-beta.1 -m 'normfix 0.4.0-beta.1'
git push origin v0.4.0-beta.1
```

`.github/workflows/release.yml` then:

1. repeats the locked quality gate;
2. rejects a tag/version mismatch;
3. builds and executes `normfix --version` on four native runners;
4. archives `normfix`, `README.md`, and `LICENSE` per target;
5. attaches provenance for each archive;
6. creates `SHA256SUMS` and a GitHub release with generated notes.

The expected assets are:

```text
normfix-x86_64-linux-gnu.tar.gz
normfix-aarch64-linux-gnu.tar.gz
normfix-x86_64-macos.tar.gz
normfix-aarch64-macos.tar.gz
SHA256SUMS
```

These are intentionally public platform names, not raw Rust target triples.
They omit placeholder/vendor components while still distinguishing operating
system, architecture, and the Linux GNU ABI.

## Verification after publication

Download at least one archive on each operating-system family, verify its
checksum, and run:

```sh
normfix --version
normfix --check path/to/a/known-clean-project
```

Confirm that the release page identifies the source tag and exposes provenance
for every archive. Release binaries still require the supported official
Norminette on `PATH`.

## Why GitHub binaries first

The CLI depends transitively on the complete internal workspace. All workspace
packages currently inherit `publish = false` while their Rust APIs are alpha,
and path-only dependencies cannot be satisfied by publishing only the
top-level CLI crate. GitHub release archives solve the student installation
problem without pretending those internal APIs have a stable crates.io
contract.

The WASM playground is source-distributed with the repository and is not one
of the four native archives. If a hosted static build is introduced later, it
needs a separate content-integrity and browser-deployment contract; the native
release workflow must not silently start uploading user source to a service.

Crates.io distribution is outside the current release contract because it
would require registry versions for every internal dependency, publication in
topological order, ownership of every package name, and semver support for
public library APIs.

## Recovery and failed releases

Do not replace an existing tag with different source. If a workflow or artifact
is wrong, mark that release as a prerelease or remove its downloadable assets,
fix the repository, increment the version, and publish a new tag. Checksums and
provenance are meaningful only when one tag identifies one immutable source
state.
