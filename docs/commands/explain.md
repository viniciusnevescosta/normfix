# `normfix explain`

Prints a bundled explanation for one rule. It scans no project, reads no file,
and uses no network.

```sh
normfix explain TOO_MANY_LINES
normfix explain INCLUDE_ORDER_REVIEW
normfix explain VLA_COMPAT_FALSE_POSITIVE
```

Every diagnostic in a normal report ends with the exact command for its own
rule, so you rarely have to type the identifier from memory:

```text
 = explain: normfix explain TOO_MANY_WS
```

## The shape of an answer

```console
$ normfix explain TOO_MANY_LINES
TOO_MANY_LINES: Function body exceeds 25 lines

Why
  The 42 Norm limits each function body to 25 physical lines so
  responsibilities stay small and reviewable.

Next
  Extract one coherent responsibility. Keep live inputs to four parameters or
  fewer and verify that the file still contains at most five functions.

Safety
  normfix reports this as a suggestion because choosing a function boundary
  changes program structure.
```

Four parts, always: what the rule is, why it exists, what to do next, and why
the tool did or did not act on it by itself.

## Rule families

Identifiers prefixed `CC_` come from the C compiler and `CC_ANALYZER_` from
`-fanalyzer`; both are explained generically, because the authoritative message
is the compiler's own. Everything else is either an official Norminette rule
name or a native `normfix` rule.

An unknown identifier still gets a useful answer rather than an error. The
bundled article set is a convenience, not the source of truth.
## Reading it from a script

Every field this command returns is documented in [the JSON API](/reference/api).
