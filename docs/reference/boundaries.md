# Known boundaries

Every limit below is deliberate. Reading them is the fastest way to understand what the tool is for.

- Exact compatibility is tested against Norminette 3.3.59; other parseable
  versions run with a prominent advisory unless strict version mode is enabled.
- C files must be valid UTF-8 and contain no NUL bytes.
- Tree-sitter recovery or unclassified tape bytes disable syntax-aware edits
  for that file.
- The default strict compiler pass uses a conservative inferred include
  context; project-specific defines, language mode, generated files, target
  flags, linking, and runtime behavior remain the project's responsibility.
- GCC `-fanalyzer` can suggest possible leaks but cannot prove leak freedom.
- The formatter does not infer project architecture, hidden evaluator
  contracts, public API intent, or target membership.
- Long-function extraction is suggested, never performed automatically.
- A hard 80-column result is guaranteed only when a safe break exists. Long
  literals, comments, directives, and ambiguous expressions remain warnings.
- The source transaction is recoverable and ordered, but a filesystem does not
  provide a single atomic rename spanning multiple files; rollback is the
  cross-file failure strategy.

## Analyzers that are not wired in

`--analyzer` uses what the compiler already ships: `-fanalyzer` on GCC, the
Clang static analyzer otherwise. Other tools are deliberately left to you,
because each needs a build or a run that `normfix` refuses to perform:

| Tool | Why it is not run |
|---|---|
| `valgrind`, `leaks` | Runtime tools. They need a linked binary and a workload, and `normfix` never builds or executes your program. |
| [AddressSanitizer](https://clang.llvm.org/docs/AddressSanitizer.html), [LeakSanitizer](https://clang.llvm.org/docs/LeakSanitizer.html), UBSan | Instrumented builds, for the same reason. `preflight` gives a separate debug-build recipe without changing the submitted Makefile. |
| [clang-tidy](https://clang.llvm.org/extra/clang-tidy/index.html) | It needs the project's real compilation database, include paths, defines, and target flags. `preflight` reports whether it is available, but does not guess a command. |
| `cppcheck`, `scan-build` | Separate installs with their own project configuration; wiring them would mean guessing your build. |

The rule behind all four rows is the same one behind everything else: a result
this tool cannot reproduce and explain is not a result it will report.
