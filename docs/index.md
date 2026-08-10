---
layout: home

hero:
  name: normfix
  text: Safe fixes for the 42 Norm
  tagline: >-
    One command fixes the mechanical mistakes across a whole 42 project, and
    explains the ones worth your time. Your hours are the scarce resource.
  actions:
    - theme: brand
      text: Why normfix
      link: /why
    - theme: alt
      text: Getting started
      link: /guide/getting-started
    - theme: alt
      text: Try it in the browser
      link: /guide/playground

features:
  - title: Proven edits only
    details: >-
      Rules propose narrow byte-range replacements against immutable shadow
      buffers. A failed proof cannot partially mutate a source file, and
      anything ambiguous is reported instead of rewritten.
  - title: The official checker decides
    details: >-
      The installed official Norminette is the oracle. Version 3.3.59 is the
      verified baseline; another release remains usable with a visible
      compatibility warning.
  - title: Recoverable by construction
    details: >-
      Writes run through one auditable transaction with external backups, a
      journal, ordered commits, rollback, and an undo that fails closed on any
      changed target.
  - title: Private in the browser
    details: >-
      The WebAssembly playground reuses the same native parser and C actions in
      your tab. No upload, account, analytics, or backend.
---

## What normfix is

`normfix` formats and diagnoses C sources, headers, Makefiles, and README
documents for 42 projects. It is not a general C rewriter: it operates under
the Norm's physical layout rules, keeps the
[official Norminette](https://github.com/42School/norminette) as the
compatibility authority, and refuses to guess where C syntax alone cannot prove
a change is safe.

## What it will not do

Every boundary below is deliberate and documented in the
[compatibility policy](/COMPATIBILITY) and the
[architecture record](/ARCHITECTURE):

- it does not claim tested native-rule compatibility for a Norminette release
  other than 3.3.59; it identifies that release before continuing;
- it does not extract long functions for you, because choosing a function
  boundary changes program structure;
- it does not prove leak freedom, and analyzer output stays informational;
- it does not guarantee a hard 80-column result when no safe break exists;
- it does not delete anything without an explicit capability grant and
  recoverable external storage.
