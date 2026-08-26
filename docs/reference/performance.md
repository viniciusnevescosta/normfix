# Performance

Every benchmark number here was measured, and the repeatable commands are
shown. The acceptance record also describes one deliberately temporary field
fixture instead of pretending that fixture is a stable benchmark.

::: tip There is no `normfix bench`
Benchmarks are a development tool, not part of the command surface. They run
through `cargo bench` from a checkout.
:::

## What a run actually costs

On a real project, `libft` with 44 sources and headers:

| Run | Time |
|---|---:|
| Cold cache, everything enabled | 1.82 s |
| Warm cache, everything enabled | 0.19 s |
| Warm cache, without the compiler preflight | 0.17 s |

The cache is worth about **ten times**, which matters because the common case
is running the tool repeatedly on a project you are working on, not once on a
project you have never seen.

### Why a cold run costs what it does

One invocation of the official Norminette costs **107 ms** on this machine, and
that is a Python interpreter starting, not anything this project controls. For
44 files that is roughly 4.7 s of serial work, which parallelism brings down to
1.82 s.

So the honest summary of a cold run is: it is dominated by a subprocess per
file. Optimizing the Rust in this repository moves that number by single-digit
percentages. The cache exists precisely because the fix for the dominant cost
is not to do the work twice.

## Acceptance result: an intentionally messy Libft

The 1.9.1 release candidate was also run against a temporary adversarial Libft:
11 analyzed files, one `normfix.toml`, and one unexpected text file. It mixed a
wrong header guard, missing official headers, a nonexistent Makefile source,
spaces where tabs were required, packed instructions, long lines, invalid
comments, a `for` loop, a ternary, misaligned declarations, and functions over
the Norm budgets.

| Operation | Result | Time |
|---|---|---:|
| Read-only pass, cache disabled | 351 safe fixes proposed in 10 files | 1.06 s |
| Authorized write pass, cache disabled | 356 fixes written to 10 files; 1 unexpected file quarantined | 1.30 s |
| Check with a fresh cache after formatting | 0 changes; 7 manual findings | 0.472 s |
| Same check, warm cache | median of five runs | 0.121 s |

The warm cache was **3.9 times faster** on this small fixture. More important
than the timing, every result boundary held:

- `make` built `libft.a` with `cc -Wall -Wextra -Werror` and `ar`;
- the same assertion driver passed before and after formatting;
- all eight optimized C object files were byte-identical before and after;
- every C, header, and Makefile line fit within 80 display columns with
  four-column tabs;
- the official Norminette then reported only the six deliberately structural
  findings: two excessive-argument locations, two excess-function locations,
  one long function, and one function with too many variables;
- normfix added one project-specific allowlist warning for the deliberate
  `puts` call, so the final report contained seven manual findings;
- a second pass proposed zero changes, and `normfix undo` restored all ten
  written files exactly while the unexpected note remained recoverable in
  quarantine.

Measured on 2026-08-26 on an Apple M1 MacBook Pro with 8 cores and 8 GB RAM,
macOS 26.5.2, Norminette 3.3.59, and the Rust 1.85 MSRV. Wall-clock times vary
with storage, Python startup, CPU load, and project shape; the correctness
checks above are the acceptance criteria, not a timing threshold.

## What this project's own code costs

These exclude every external tool, so they measure what a change to this
repository can actually regress:

| Case | Time |
|---|---:|
| Already-correct 50-line file | 0.95 ms |
| Messy 40-line file, every layout action | 1.89 ms |
| Messy 800-line file | 38.2 ms |
| Constructing a parser | 0.34 µs |

Measured on an Apple M1, 8 cores, macOS 26.5, with the toolchain pinned in
`rust-toolchain.toml`.

```sh
cargo bench -p normfix-c-actions
```

CI runs the same benchmarks on every push as an informational job. A shared
runner is too noisy to gate on, but a benchmark that never runs is a benchmark
that quietly stops compiling.

## What benchmarking found

The benchmarks were added after weeks of hand timing, and the first run
contradicted two assumptions in a few minutes.

An already-correct 50-line file took **4.5 ms** to decide that nothing needed
doing. The suspected cause was parser construction; measuring it showed
**340 nanoseconds**, so that was not it. The real cause was that the source was
re-parsed once per formatting phase, when it cannot change while the phase loop
runs: accepting a batch is the only thing that rewrites it, and that leaves the
loop immediately.

Parsing once per pass instead:

| Case | Before | After |
|---|---:|---:|
| Already-correct 50-line file | 4.49 ms | 0.95 ms |
| Messy 800-line file | 108 ms | 38.2 ms |

End to end on a real project that is a 29 percent improvement warm and 5
percent cold, for the reason above: a cold run is waiting on Python.

The lesson is worth more than the numbers. Two plausible explanations were
wrong, and only measurement said so.

## What is not optimized

- **The per-file subprocess.** Norminette accepts several files in one
  invocation, which would replace 44 process launches with one. Doing that
  means the pipeline can no longer check one file's proposed bytes at a time,
  which is how the before/after proof is built today. It is the largest
  remaining win and the one with the most architectural cost.
- **Very large single files.** Above a few thousand lines the cost is
  dominated by something other than the line index, and that has not been
  chased. Real 42 sources are far below that.
- **Token allocation.** Each parse copies every token's text into an owned
  string. Borrowing from the source instead is a contained change that has not
  been measured yet.
