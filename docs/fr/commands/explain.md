# `normfix explain`

Affiche l'explication intégrée d'une règle. Il n'analyse aucun projet, ne lit
aucun fichier et n'utilise pas le réseau.

```sh
normfix explain TOO_MANY_LINES
normfix explain INCLUDE_ORDER_REVIEW
normfix explain VLA_COMPAT_FALSE_POSITIVE
```

Chaque diagnostic d'un rapport normal se termine par la commande exacte de sa
propre règle, si bien que vous avez rarement à taper l'identifiant de mémoire :

```text
 = explain: normfix explain TOO_MANY_WS
```

## La forme d'une réponse

```console
$ normfix explain TOO_MANY_LINES
TOO_MANY_LINES: Function body exceeds 25 lines

Why
  The 42 Norm limits each function body to 25 physical lines so
  responsibilities stay small and reviewable.

Next
  Extract one coherent responsibility. Keep live inputs to four parameters or
  fewer and verify that the file still contains at most five functions.

Safety
  normfix reports this as a suggestion because choosing a function boundary
  changes program structure.
```

Quatre parties, toujours : ce qu'est la règle, pourquoi elle existe, quoi faire
ensuite, et pourquoi l'outil a agi ou non de lui-même.

## Familles de règles

Les identifiants préfixés `CC_` viennent du compilateur C et `CC_ANALYZER_` de
`-fanalyzer` ; les deux sont expliqués de façon générique, car le message qui
fait autorité est celui du compilateur lui-même. Tout le reste est soit un nom
de règle officiel de Norminette, soit une règle native de `normfix`.

Un identifiant inconnu reçoit quand même une réponse utile plutôt qu'une erreur.
L'ensemble d'articles intégré est un confort, pas la source de vérité.
