# Fuzz targets

Two invariants that example tests cannot exhaust:

| Target | Asserts |
|---|---|
| `tape_round_trip` | The token tape reconstructs any input byte for byte |
| `actions_never_panic` | The action pipeline returns a value or a typed error, never a panic |

```sh
cargo install cargo-fuzz --locked
cargo +nightly fuzz run tape_round_trip
cargo +nightly fuzz run actions_never_panic
```

`cargo-fuzz` needs a nightly toolchain, which is why this is not part of the
pinned workspace or of CI: the project pins a stable toolchain and a 1.85 MSRV
on purpose. Run these when changing the parser, the tape, or the scheduler.

A crash writes its input to `fuzz/artifacts/`. Reproduce with
`cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<file>`, then add it
as a regression test in the crate it belongs to.
