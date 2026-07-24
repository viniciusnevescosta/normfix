# norminette-fix

`norminette-fix` fixes formatting problems that can be changed safely, inserts
the official 42 header, runs the official Norminette again, and prints
actionable English warnings for anything that needs a human refactor.

It follows the 42 Norm v4.1 and pins the official
[Norminette 3.3.59](https://github.com/42School/norminette/releases/tag/3.3.59).

## Install

Python 3.10 or newer is required. `pipx` is recommended because it exposes the
command globally without mixing dependencies with your projects:

```sh
pipx install /Users/viniciuscosta/projects/norminette-fix
```

For development:

```sh
cd /Users/viniciuscosta/projects/norminette-fix
python3.12 -m venv .venv
.venv/bin/python -m pip install -e '.[dev]'
```

## Use it like Norminette

With no path, every `.c` and `.h` below the current directory is processed
recursively:

```sh
cd ~/projects/42-school/my_project
norminette-fix
```

Pass one or more files, directories, or a mixture of both:

```sh
norminette-fix main.c
norminette-fix src include
norminette-fix src/parser.c src/lexer.c include/minishell.h
```

Useful modes:

```sh
norminette-fix --check
norminette-fix --diff src
norminette-fix --format json --check
norminette-fix --use-gitignore
norminette-fix --verbose
norminette-fix --timeout 10
```

- Default mode edits files in place.
- `--check` reports what would change without writing.
- `--diff` previews a unified diff without writing.
- `--use-gitignore` skips ignored files found while scanning directories.
- `--timeout` isolates a file when the official parser gets stuck (5 seconds
  per file by default), so the rest of the directory is still processed.
- Exit `0` means clean, `1` means fixes or review items remain, and `2` means an
  input, I/O, or tool failure occurred.

## Official 42 header

The header is inserted automatically when absent. Identity is resolved in this
order:

1. `--login` and `--email`
2. `NORMINETTE_FIX_LOGIN` and `NORMINETTE_FIX_EMAIL`
3. Git `user.name` and a Git email only when it is a 42 student address
4. the operating-system username and a derived student address

For predictable metadata, configure it once in your shell:

```sh
export NORMINETTE_FIX_LOGIN='your_login'
export NORMINETTE_FIX_EMAIL='your_login@student.42sp.org'
```

Or pass it for one run:

```sh
norminette-fix --login your_login --email your_login@student.42sp.org
```

Valid existing headers keep their author and creation date. The filename and
`Updated` line are refreshed only when another accepted edit changes the file,
so a second run is idempotent.

Header files also receive a filename-derived inclusion guard, for example
`ft_string.h` becomes `FT_STRING_H`.

## What is fixed automatically

The default fixer targets transformations that preserve the meaningful C token
sequence whenever possible:

- trailing whitespace, blank-line rules, final newline, and CRLF normalization;
- required blank lines between declarations/body, preprocessors, and functions;
- real-tab indentation and common declaration alignment;
- braces that need their own line;
- spacing around operators, parentheses, keywords, pointers, and function names;
- empty argument lists in definitions changed to `(void)`; old-style prototypes
  are reported because changing `f()` to `f(void)` changes their C type;
- `return value;` changed to `return (value);`;
- independent same-line instructions split at proven statement boundaries;
- lines over 80 columns wrapped at token-safe logical operators, binary
  operators, comparisons, or commas, with Norm-compliant continuation tabs;
- nested preprocessor indentation and directive spacing;
- official 42 headers and header inclusion guards.

Every layout-only batch is checked against Norminette's lexer. If significant
tokens would change unexpectedly, that batch is rejected.

Original files are backed up outside the scanned project before writing:

```text
~/.local/share/norminette-fix/backups/<run-id>/
```

Use `--no-backup` to opt out or `--backup-dir PATH` to select another external
location.

## What is reported for manual work

The tool deliberately does not guess at semantic or architectural changes. It
prints the location, official error code, measured limit when available, why it
was left alone, and a concrete next step for cases such as:

- function over 25 lines;
- more than 4 parameters, 5 local variables, or 5 functions per `.c` file;
- line over 80 display columns when no safe automatic break point exists
  (notably long literals, comments, or preprocessor lines);
- `for`, `do/while`, `switch`, `goto`, ternaries, VLAs, and assignments in
  conditions;
- declarations mixed with initialization or placed after statements;
- project-wide identifier, macro, global, typedef, struct, enum, or union
  renames;
- moving types/includes between files or changing build structure;
- comments inside functions and semantic preprocessor errors;
- malformed C that the official parser cannot understand.

This boundary is intentional: splitting a function or rewriting control flow
without understanding the project can make code pass Norminette while silently
breaking it.

## Develop and test

```sh
.venv/bin/pytest
```

The test suite covers official header shape, header guards, recursive and
multi-target discovery, safe formatting, structured warnings, preview modes,
JSON output, and second-run idempotency.

## Scope

The formatter handles `.c` and `.h`, matching the official Norminette command.
Makefile rules and subjective rules marked `(*)` in the Norm v4.1 still require
evaluation by a person.
