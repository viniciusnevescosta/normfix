# Performance

Every number here was measured, and every one is reproducible with a command
you can run yourself. Where a number is not impressive, it says so and says
why.

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
