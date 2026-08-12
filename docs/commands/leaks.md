# `normfix leaks`

Runs a program you already built under a leak checker, and reports what it
observed.

```sh
normfix leaks ./libft_test
normfix leaks ./push_swap -- 3 1 2
```

Everything else normfix does reads your source. This one executes it, so it
asks first:

```console
$ normfix leaks ./push_swap
normfix is about to run ./push_swap under the leak checker. This executes your program. Continue? [y/N] y
Lost 1024 bytes outright, and 96 more reachable only through them.
This is what one run observed with the arguments it was given. It is not a proof that the program never leaks.
```

Arguments after `--` go to your program, not to the checker, so you can exercise
the path that matters:

```sh
normfix leaks ./push_swap -- 5 2 9 1
```

## What it does not do

`normfix` never builds your program. Building means running your Makefile's
recipes, which is a second and much larger category of running code that you
wrote — and "you built it, I ran it" is a far smaller promise than "I built and
ran it". Build it the way you normally do, then point this command at the
result.

## A clean result is not a proof

The checker sees the one path your program took with the arguments you gave it.
A run that loses nothing tells you that path is clean; it says nothing about the
paths you did not take. That line is printed with every result for the same
reason the rest of this tool reports what it cannot prove instead of asserting
it.

Memory still reachable at exit is not counted as lost. 42 evaluates memory
nobody can reach any more, and an arena your program holds until it exits is not
that.

If the checker produces output normfix cannot read as a leak summary, that is an
error, not a clean result. A checker that was killed and a checker that found
nothing produce the same silence, and the difference matters too much to guess
at.

## Exit codes

| Code | Meaning |
|---|---:|
| `0` | Nothing was lost on the path this run took |
| `1` | Something was lost |
| `2` | The checker is unavailable, was refused, or could not be read |

Outside an interactive terminal — in CI, or with `--format json` — the
confirmation cannot be answered, so `--force` is required:

```sh
normfix leaks --force ./libft_test
```

## Installing a checker

| System | How |
|---|---|
| Linux, FreeBSD | Valgrind, from your package manager |
| macOS | [`LouisBrunner/valgrind-macos`](https://github.com/LouisBrunner/valgrind-macos), since upstream Valgrind does not build for macOS. Its Apple Silicon support is limited |
| Windows | Run normfix inside [WSL](https://learn.microsoft.com/windows/wsl/install), where the Linux checker works normally |

normfix locates `valgrind` on `PATH` and verifies it by its own `--version`, so
any working build satisfies it. When none is found, it says so and names the
route for the system you are on.
