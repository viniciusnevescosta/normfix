# What normfix is, and why

## The point

A 42 student's scarcest resource is time. Not skill, not effort. Hours. And a
meaningful share of those hours goes into whitespace: fixing indentation,
moving declarations, splitting lines at 80 columns, pasting headers. Across a
cursus that is thousands of files, in project after project, none of which
teaches you anything the second time you do it.

`normfix` exists to give those hours back. It corrects, in one command and
across an entire project, the mistakes that are mechanical, and refuses to
touch the ones that are actually about your program, because those are the ones
worth your time.

## In one paragraph

You write C for a 42 project. The
[official Norminette](https://github.com/42School/norminette) tells you that
line 47 has the wrong indentation, that a function is too long, that a
declaration is in the wrong place, and then stops, because reporting is all it
does.
`normfix` reads the same project, fixes the mistakes it can prove are safe to
fix, and explains the rest in English instead of a rule name. It is one command
that leaves your project closer to passing than it found it, or tells you
exactly why it could not.

```sh
cd path/to/a/42-project
normfix
```

That is the whole interface. No configuration file is required, nothing is
uploaded, and every file it rewrites is backed up outside the project first.

## The problem

The 42 Norm is a layout standard: real tabs, 80 columns, one declaration per
line, a blank line after the declaration block, 25 lines per function, five
functions per file, an official header at the top of every file. None of it is
hard. All of it is tedious, and all of it is checked by a tool that only says
*no*.

So the day before a defense you are doing one of two things: hand-editing
whitespace across forty files, or running a general-purpose formatter and
hoping. Both go badly. The first is slow and you will miss something. The
second is worse, because a formatter that does not know the Norm will
confidently produce code the Norminette rejects, and it rewrites your whole
file to do it, so you cannot tell what it changed from what you wrote.

## What normfix does differently

**It uses the official checker as the authority.** The installed Norminette
runs before and after every batch of edits. If a batch introduces a rule
violation that was not there before, the whole batch is reverted and your
original bytes stay. Version 3.3.59 is the tested compatibility baseline; a
different installed release remains usable, but is named in a prominent
warning because the native rules have not received the same validation.
`normfix` never argues with the tool you are actually graded by.

**It edits narrow byte ranges, not whole files.** A change touches the range it
proved something about and nothing else, so the diff is reviewable and the rest
of your file is byte-identical. This is why you can run it on work in progress.

**It refuses more than it accepts.** Reordering includes across a `#ifdef`
could change which declarations exist, so it stops at the conditional.
Extracting a function from a 40-line body requires naming the new function,
which is a design decision, so it reports the length and lets you decide.
Every refusal comes with the reason and the next step.

**Everything it writes is recoverable.** Writes go through one transaction with
external backups and a journal. `normfix undo` restores a run, and refuses to
do so if you have edited those files since.

## What it will not do

This is the honest list, and it is the point of the tool rather than a
limitation of the current version:

- It will not extract a long function for you.
- It will not redesign control flow, rename across a project, or change a
  public signature.
- It will not prove your program is leak-free. The analyzer pass can
  suggest a leak; it cannot prove its absence.
- It will not call an untested Norminette release "supported." It continues
  with a visible compatibility warning so a 42 upgrade does not make the tool
  unusable, while `--strict-norminette-version` restores fail-closed behavior.
- It will not guarantee 80 columns when no safe break exists. A long string or
  a macro stays long and is reported instead.

## Where it fits

| Moment | Command |
|---|---|
| While writing | `normfix --changed` on what you just touched |
| Before committing | `normfix --check` as a gate; exit code `1` means work remains |
| In review | `normfix lint --format json` for a diagnosis with no edits |
| Before a defense | [`normfix preflight`](/commands/preflight), which adds the strict compiler pass |
| After a bad run | [`normfix undo`](/commands/undo) |

## The rule it is built on

> Change what can be proven, explain what cannot, and never turn uncertainty
> into permission.

Every design decision in [the architecture](/ARCHITECTURE) follows from that
sentence, including the ones that make the tool do less than it could.

## Next

- [Getting started](/guide/getting-started): install it and make a first,
  reversible run.
- [Commands](/commands/): a page per subcommand, with real output.
- [Every flag](/reference/flags): what each one does, with an example.
- [Browser playground](/guide/playground): try the formatter without
  installing anything.
