# `normfix format`

Aplica as edições que passaram por todas as provas e as grava através de uma
única transação recuperável.

```sh
normfix format
normfix format src includes
normfix format src/parser.c includes/minishell.h
```

`normfix` sem subcomando faz a mesma coisa. Use `format` quando a intenção
precisar ser óbvia para quem ler o script depois.

## Como é uma execução

```console
$ normfix format
normfix 1.6.0
Safe automatic fixes for the 42 Norm v4.1

Files
STATUS      FIXES  REMAINING  INFO  FILE
FIXED        17          0     0  math_utils.c

Summary: 1 files | 1 proposed | 1 written | 17 fixes | 0 remaining | 0 info | 0 failed
Completed in 0.62 s.
```

As dezessete correções incluem o cabeçalho oficial, a ordem dos includes, o
layout das chaves, a indentação com tabulações, a separação das declarações e os
`return` entre parênteses.

## Ver a mudança antes de aceitá-la

`--diff` imprime um diff unificado e não grava nada:

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

As tabulações são exibidas como `\t` para que mudanças de indentação continuem
visíveis em um terminal.

## Aprovar arquivo por arquivo

```sh
normfix format --interactive
```

A primeira passagem é somente leitura e imprime cada diff proposto, aceitando
`y`, `n`, `a` (todos) ou `q` (cancelar). A execução então analisa o mesmo escopo
de novo e grava apenas os arquivos cujo plano da segunda passagem ainda
corresponde aos bytes que você aprovou. Se algo mudou por baixo de você, esse
arquivo é pulado e relatado.

O modo interativo exige um terminal real e se recusa a combinar com `--check`,
`--diff`, saída JSON ou flags destrutivas.

## Formatar só o que você tocou

```sh
normfix format --changed
normfix format --staged
```

Veja [escopos do Git](/pt/guide/command-line#git-scopes) para saber exatamente o
que cada um seleciona.

## Backups

Toda gravação mantém os bytes originais fora do projeto:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

`--no-backup` pula isso para a formatação comum. Ele **não** pula para uma
remoção destrutiva, que sempre exige armazenamento recuperável e falha fechada
sem ele. Restaure com [`undo`](/pt/commands/undo).
