# Commands

The commandless interface is the shortest way to format a project, and it is
what most runs use:

```sh
cd path/to/a/42-project
normfix
```

Subcommands make intent explicit, which matters in scripts, in CI, and during
review.

| Command | Writes | Use it when |
|---|---|---|
| [`format`](/commands/format) | yes | You want the accepted edits applied |
| [`lint`](/commands/lint) | no | You want diagnostics about the bytes on disk, with nothing proposed |
| [`check`](/commands/check) | no | You want to see what a fixing run *would* do |
| [`budget`](/commands/budget) | no | You want line/variable/parameter headroom per function |
| [`preflight`](/commands/preflight) | no | You are about to defend and want the read-only checks |
| [`explain`](/commands/explain) | no | You want a rule explained without scanning anything |
| [`undo`](/commands/undo) | yes | You want a previous run restored |
| [`upgrade`](/commands/upgrade) | yes | You want the newest release, verified |
| [`uninstall`](/commands/uninstall) | yes | You want normfix removed from this machine |

## Every example on these pages is real

The output shown was produced by `normfix 1.1.0` against this file:

```c
# include "libft.h"
# include <stdlib.h>

int add(int a,int b){
return a+b;
}

int	scale(int value, int factor)
{
	int result;
	result = value * factor;
	return result;
}
```

It is deliberately messy in ordinary ways: unordered includes, a collapsed
function definition, missing spaces, a declaration not separated from the
instructions, and bare `return` values.

## Exit codes

Every command shares them:

| Code | Meaning |
|---:|---|
| `0` | Nothing blocking: the run was clean, or fix mode completed |
| `1` | Manual diagnostics remain, or a preview found proposed changes |
| `2` | Discovery, configuration, tool, I/O, transaction, or quarantine failure |
| `130` | An interactive review was cancelled |

Informational advisories never change the exit code. This makes the codes
usable directly in CI:

```sh
normfix --check || echo "this project is not Norm-clean yet"
```

## Flags every command accepts

`--format json` and `--no-color` change output; `--threads`, `--timeout`,
`--no-cache`, and `--norminette PATH` change how the run executes. The full
table lives in [command line](/guide/command-line).
