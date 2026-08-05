# Known boundaries

Every limit below is deliberate. Reading them is the fastest way to understand what the tool is for.

- Exact compatibility requires Norminette 3.3.59; other versions are rejected.
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
