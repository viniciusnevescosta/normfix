# `normfix format`

Applies the edits that passed every proof gate, and writes them through one
recoverable transaction.

```sh
normfix format
normfix format src includes
normfix format src/parser.c includes/minishell.h
```

`normfix` with no subcommand does the same thing. Use `format` when the intent
should be obvious to whoever reads the script later.

## What a run looks like

```console
$ normfix format
normfix 1.0.0-rc.2
Safe automatic fixes for the 42 Norm v4.1

Files
STATUS      FIXES  REMAINING  INFO  FILE
FIXED        17          0     0  math_utils.c

Summary: 1 files | 1 proposed | 1 written | 17 fixes | 0 remaining | 0 info | 0 failed
Completed in 0.62 s.
```

The seventeen fixes include the official header, the include order, the brace
layout, tab indentation, declaration separation, and the parenthesized
returns.

## See the change before accepting it

`--diff` prints a unified diff and writes nothing:

```diff
--- a/math_utils.c
+++ b/math_utils.c
@@ -1,13 +1,27 @@
-# include "libft.h"
-# include <stdlib.h>
+/* *********************************************************************** */
+/*                                                                         */
+/*   math_utils.c                                       :+:      :+:       */
+/*   By: vneves-c <vneves-c@student.42.fr>          +#+  +:+       +#+     */
+/*   Created: 2026/08/05 14:29:44 by vneves-c          #+#    #+#          */
+/* *********************************************************************** */
+
+#include <stdlib.h>
+#include "libft.h"

-int add(int a,int b){
-return a+b;
+int\tadd(int a, int b)
+{
+\treturn (a + b);
 }
```

Tabs are rendered as `\t` so indentation changes stay visible in a terminal.

## Approve file by file

```sh
normfix format --interactive
```

The first pass is read-only and prints each proposed diff, accepting `y`, `n`,
`a` (all), or `q` (cancel). The run then analyzes the same scope again and
writes only the files whose second-pass plan still matches the bytes you
approved. If anything changed underneath you, that file is skipped and
reported.

Interactive mode needs a real terminal and refuses to combine with `--check`,
`--diff`, JSON output, or destructive flags.

## Format only what you touched

```sh
normfix format --changed
normfix format --staged
```

See [Git scopes](/guide/command-line#git-scopes) for exactly what each selects.

## Backups

Every write keeps the original bytes outside the project:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

`--no-backup` skips that for ordinary formatting. It does **not** skip it for a
destructive removal, which always requires recoverable storage and fails closed
without it. Restore with [`undo`](/commands/undo).
