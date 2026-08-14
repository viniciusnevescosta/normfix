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
  - title: It only changes what it can prove
    details: >-
      An edit touches the exact piece it proved something about, and nothing
      else. Whatever it cannot prove, it reports and leaves alone — so you can
      run it on work in progress and still read the diff.
  - title: The Norminette has the final word
    details: >-
      normfix never argues with the tool you are graded by. It runs the
      official checker before and after its edits, and throws away any batch
      that made things worse.
  - title: Nothing gets lost
    details: >-
      Every file it rewrites is copied outside your project first. `normfix
      undo` puts a run back, and refuses if you have edited those files since.
  - title: Try it without installing
    details: >-
      The playground runs in your browser tab. Nothing is uploaded, there is no
      account, and there is nothing watching what you paste.
---

## What normfix is

`normfix` formats and checks the C files, headers, Makefiles, and READMEs of a
42 project. It is not a general-purpose C rewriter: it works within the Norm's
layout rules, treats the
[official Norminette](https://github.com/42School/norminette) as the authority
on what those rules mean, and refuses to guess whenever the C syntax alone
cannot show that a change is safe.

## What it will not do

Each of these is a decision, not a gap. The
[compatibility policy](/COMPATIBILITY) and the
[architecture record](/ARCHITECTURE) explain the reasoning:

- it will not call an untested Norminette release supported — it names the one
  it found and carries on with a warning;
- it will not split a long function for you, because choosing where to cut it
  changes how your program is built;
- it will not prove your program is leak-free; the analyzer can point at a
  likely leak, never at the absence of one;
- it will not force 80 columns when there is no safe place to break the line;
- it will not delete anything unless you ask for it, and never without a copy
  you can restore from.
