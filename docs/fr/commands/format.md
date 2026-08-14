# `normfix format`

Applique les modifications qui ont franchi toutes les preuves, et les écrit via
une seule transaction récupérable.

```sh
normfix format
normfix format src includes
normfix format src/parser.c includes/minishell.h
```

`normfix` sans sous-commande fait la même chose. Utilisez `format` quand
l'intention doit être évidente pour qui lira le script plus tard.

## À quoi ressemble une exécution

```console
$ normfix format
normfix 1.6.3
Safe automatic fixes for the 42 Norm v4.1

Files
STATUS      FIXES  REMAINING  INFO  FILE
FIXED        17          0     0  math_utils.c

Summary: fichiers : 1 | proposés : 1 | écrits : 1 | corrections : 17 | restants : 0 | informatifs : 0 | en échec : 0
Completed in 0.62 s.
```

Les dix-sept corrections comprennent l'en-tête officiel, l'ordre des includes,
la disposition des accolades, l'indentation par tabulations, la séparation des
déclarations et les `return` entre parenthèses.

## Voir le changement avant de l'accepter

`--diff` affiche un diff unifié et n'écrit rien :

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

Les tabulations sont rendues par `\t` pour que les changements d'indentation
restent visibles dans un terminal.

## Approuver fichier par fichier

```sh
normfix format --interactive
```

La première passe est en lecture seule et affiche chaque diff proposé, en
acceptant `y`, `n`, `a` (tous) ou `q` (annuler). L'exécution analyse ensuite à
nouveau la même portée et n'écrit que les fichiers dont le plan de la deuxième
passe correspond encore aux octets que vous avez approuvés. Si quelque chose a
changé sous vos pieds, ce fichier est ignoré et signalé.

Le mode interactif exige un vrai terminal et refuse de se combiner avec
`--check`, `--diff`, la sortie JSON ou les options destructives.

## Formater seulement ce que vous avez touché

```sh
normfix format --changed
normfix format --staged
```

Voir [portées Git](/fr/guide/command-line#git-scopes) pour savoir exactement ce
que chacune sélectionne.

## Sauvegardes

Chaque écriture conserve les octets d'origine hors du projet :

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

`--no-backup` saute cela pour la mise en forme ordinaire. Il ne le saute
**pas** pour une suppression destructive, qui exige toujours un stockage
récupérable et échoue fermée sans lui. Restaurez avec
[`undo`](/fr/commands/undo).
