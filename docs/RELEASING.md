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
6. `npm audit --audit-level=moderate` reports no known site dependency issue;
7. real-project fixture changes have been reviewed as source diffs, not only as
   exit codes.

Run the same local quality gate used by CI:

```sh
cargo fmt --all --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
npm ci
npm audit --audit-level=moderate
npm run build
```

## Tag and publish

Create an annotated tag whose text is exactly the workspace version prefixed by
`v`. The tagged commit must already be contained in `main`:

```sh
git tag -a v1.3.0 -m 'normfix 1.3.0'
git push origin v1.3.0
```

`.github/workflows/release.yml` then:

1. proves the tag is annotated, belongs to `main`, and matches the workspace
   version;
2. repeats the locked Rust, npm audit, WASM, playground, and documentation
   quality gates;
3. builds and executes `normfix --version` on four native runners;
4. archives `normfix`, `README.md`, and `LICENSE` per target;
5. attaches provenance for each archive;
6. creates `SHA256SUMS` and one immutable GitHub release with generated notes.

A rerun refuses to replace an existing release or asset. Publish a new version
instead of using the same tag for different bytes.

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

The WASM playground is source-distributed with the repository, deployed as a
static Vercel site, and is not one of the four native archives. Formatting runs
inside the browser; source text is not submitted to a normfix application
server. Its deployment and CSP are separate from the native archive contract.

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

## After the release

The Homebrew tap is a separate repository,
[`viniciusnevescosta/homebrew-normfix`](https://github.com/viniciusnevescosta/homebrew-normfix),
and it is not updated by the release workflow. Once the archives are published:

1. update `version`, the four URLs, and the four `sha256` values in
   `packaging/homebrew/normfix.rb` from the published `SHA256SUMS`;
2. copy it to `Formula/normfix.rb` in the tap and push;
3. verify with `brew fetch viniciusnevescosta/normfix/normfix`, which fails
   loudly on a wrong checksum.

The one-line installer needs no release step. It resolves `/releases/latest`
for the stable channel and reads checksums from the manifest. Until a stable
release exists, it scans the public release feed and falls back to the newest
pre-release; once a stable exists, normal installs never select a pre-release.
