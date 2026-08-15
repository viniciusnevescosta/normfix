# Release roadmap

The roadmap separates changes by compatibility promise. Dates are deliberately
absent: a release moves only after its tests, real-project checks, native
archives, and browser build are green.

`1.0.0` is released. The command surface, rule IDs, exit codes, and the
`schema_version: 2` JSON report are stable for all of 1.x.

What shipped, and why each change was made, lives in the
[release notes](https://github.com/viniciusnevescosta/normfix/blob/main/CHANGELOG.md).
This page is only what is still ahead, so a reader never has to work out which
half they are looking at.

## 1.9 — Python projects

A separate Python pipeline on the same oracle model the Norminette uses, plus a
Python-capable playground. The C/Norminette contract stays available and
versioned instead of being silently generalized.

What matters is the result a student needs: strict type checking and lint
findings they can act on. mypy `--strict` and flake8 are the reference for that
result, and the decision of which tools produce it is open — Astral's `ruff` and
`ty` may reach the same answers faster, and being faster matters for a tool a
student runs before every push. Whether either can be embedded, so that nothing
has to be installed, is part of that comparison. The choice will be made by what
each reports on real 42 Python projects, and whichever is chosen becomes the
versioned oracle the way Norminette 3.3.59 is.

## 1.10 — starting a project

Create a project from explicit choices: its name and allowed function list, then
`main.c`, the header, the Makefile, a `README.md` carrying the student's login,
`src/`, `tests/`, and an initialized Git repository. For C and for Python.
