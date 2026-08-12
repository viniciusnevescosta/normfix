# `normfix format`

Aplica las ediciones que superaron todas las pruebas y las escribe mediante una
única transacción recuperable.

```sh
normfix format
normfix format src includes
normfix format src/parser.c includes/minishell.h
```

`normfix` sin subcomando hace lo mismo. Usa `format` cuando la intención deba
ser obvia para quien lea el script más adelante.

## Cómo es una ejecución

```console
$ normfix format
normfix 1.4.0
Safe automatic fixes for the 42 Norm v4.1

Files
STATUS      FIXES  REMAINING  INFO  FILE
FIXED        17          0     0  math_utils.c

Summary: 1 files | 1 proposed | 1 written | 17 fixes | 0 remaining | 0 info | 0 failed
Completed in 0.62 s.
```

Las diecisiete correcciones incluyen la cabecera oficial, el orden de los
includes, la disposición de las llaves, la indentación con tabulaciones, la
separación de declaraciones y los `return` entre paréntesis.

## Ver el cambio antes de aceptarlo

`--diff` imprime un diff unificado y no escribe nada:

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

Las tabulaciones se muestran como `\t` para que los cambios de indentación sigan
siendo visibles en un terminal.

## Aprobar archivo por archivo

```sh
normfix format --interactive
```

La primera pasada es de solo lectura e imprime cada diff propuesto, aceptando
`y`, `n`, `a` (todos) o `q` (cancelar). Luego la ejecución analiza otra vez el
mismo alcance y escribe solo los archivos cuyo plan de la segunda pasada aún
coincide con los bytes que aprobaste. Si algo cambió debajo de ti, ese archivo se
omite y se informa.

El modo interactivo necesita un terminal real y se niega a combinarse con
`--check`, `--diff`, salida JSON o flags destructivas.

## Formatear solo lo que tocaste

```sh
normfix format --changed
normfix format --staged
```

Consulta [alcances de Git](/es/guide/command-line#git-scopes) para saber
exactamente qué selecciona cada uno.

## Copias de seguridad

Cada escritura conserva los bytes originales fuera del proyecto:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

`--no-backup` omite eso para el formateo corriente. **No** lo omite para una
eliminación destructiva, que siempre exige almacenamiento recuperable y falla
cerrada sin él. Restaura con [`undo`](/es/commands/undo).
