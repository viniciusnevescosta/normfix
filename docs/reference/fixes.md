# What is fixed, and what is not

The native C formatter currently handles proven cases in these areas:

- UTF-8 BOM removal, CRLF normalization, trailing whitespace, blank-line runs,
  file-start whitespace, and one final newline;
- preprocessor indentation and spacing, excluding sensitive multiline forms;
- include block order: system headers before project headers, alphabetically
  inside each category;
- required and forbidden blank lines around declarations, preprocessors, and
  functions;
- braces and control bodies that need their own physical line;
- Allman control layout, conservative removal of redundant single-statement
  blocks, and a narrow redundant-`else` cleanup when both branches return;
- four-column tab-stop indentation and common space/tab diagnostics;
- indentation and the required following blank line for simple initial local
  declaration groups;
- spacing around operators, pointers, parentheses, keywords, and function
  declarators;
- group alignment for simple one-line variables and function prototypes,
  including pointer declarators when the group is unambiguous;
- a declaration separated from the value it was given: `int teste = 10;`
  becomes `int teste;` and an assignment below the declaration block, which is
  what the official `DECL_ASSIGN_LINE` asks for;
- deletion of a statement that is only a `;`, when it sits in a block or at
  file scope and no preprocessor directive precedes it;
- `return value;` to `return (value);`;
- empty parameter lists in function definitions to `(void)`;
- pointer-return `return (0);` to `return (NULL);` when the return type and a
  visible `NULL` provider are both proven;
- line wrapping at proven operators or commas;
- greedy rejoining of continuation lines while the result remains within 80
  display columns.

Long-line packing does not cross comments, preprocessing directives, line
splices, or unrelated instructions. Strings and comments are not split.
Preprocessor lines are not rewritten merely to satisfy width.

### Include order

A run of `#include` directives is reordered only while **every** line in it is
exactly one include directive. The first line that is anything else (a comment,
a blank line, a conditional, a macro definition, or trailing text after the
closing delimiter) ends the run, and the directives on each side are sorted
independently. No directive is ever moved across such a construct, because
crossing one can change declarations, feature macros, or conditional
compilation.

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

Sorting is by category first (`<system>` before `"project"`), then by the
header name, compared case-insensitively. Equal names keep their original
relative order. Use `--no-reorder-includes` to leave every block untouched; the
report then falls back to the `INCLUDE_ORDER_REVIEW` warning.

The formatter measures terminal display cells: tabs use four-column stops,
combining marks use zero cells, and wide Unicode characters use two.

### Proof gates

Formatting happens only in memory first. For every layout action:

- the source must parse without `ERROR`, `MISSING`, or unknown tape regions;
- the token tape must cover and reconstruct the complete input;
- the ordered token-and-comment fingerprint must remain identical;
- the candidate must reparse without recovery;
- edit ranges must be valid and non-overlapping.

After the complete candidate is produced, Norminette runs again. If any rule
count increases relative to the validated baseline, the native formatting
batch is reverted for that file. Operational failures never authorize a
partial write.

Narrow token-changing actions such as `return (...)` and `(void)` are separate
semantic actions with dedicated construction rules; they are not treated as
generic whitespace edits.

## Diagnostics that remain manual

The terminal report explains the rule, exact source span, origin, and a concrete
next step for work such as:

- functions over 25 body lines;
- more than 4 parameters, 5 local variables, or 5 functions per `.c` file;
- lines over 80 columns with no safe operator/comma break;
- forbidden control structures, ternaries, `goto`, labels, and assignments in
  conditions;
- declarations that appear after a statement;
- public or global identifiers that need project-wide renaming;
- type/include movement and project structure changes;
- ambiguous declarations, function pointers, attributes, bit-fields, and
  multiline declarators;
- malformed or parser-recovered C;
- header guards that fail the closed-worktree proof.

The semantic layer evaluates a conservative subset of C integer constant
expressions, including enum constants. This allows a known enum bound such as
`count[op_total]` to be reported as an informational Norminette compatibility
false positive instead of an actual variable-length array. Unsupported
expressions remain unknown; they are never guessed.

For a long function, the diagnostic suggests extracting a cohesive region and
reports the applicable budget. It never moves statements, invents parameters,
or creates a helper automatically: data flow, naming, visibility, and project
intent cannot be proven from formatting facts alone.
