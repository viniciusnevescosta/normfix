# `normfix budget`

Une exécution en lecture seule qui ajoute une ligne informative par fonction
analysée, montrant combien de marge il reste avant les limites de la Norme :
25 lignes, 5 variables locales et 4 paramètres.

```sh
normfix budget
normfix budget src
```

```console
$ normfix budget
info[NORM_BUDGET]: 2 occurrences in 1 file
  math_utils.c:4:1   add(): lines 1/25 (24 left), variables 0/5 (5 left),
                     parameters 2/4 (2 left).
  math_utils.c:8:1   scale(): lines 3/25 (22 left), variables 1/5 (4 left),
                     parameters 2/4 (2 left).
 = help: Keep headroom for defense-day changes; limits already exceeded are
         also reported as warnings.
 = source: Norm v4.1 native rule

Summary: fichiers : 1 | proposés : 0 | écrits : 0 | corrections : 0 | restants : 14 | informatifs : 2
```

Les lignes de budget sont informatives et ne changent jamais à elles seules le
code de sortie.

`budget` diagnostique les octets déjà présents sur le disque et ne planifie
jamais de modification. Il refuse donc les options de mise en forme, d'identité
d'en-tête, de sauvegarde, de diff et de suppression au lieu de les ignorer
silencieusement. Utilisez `normfix check` pour prévisualiser les corrections.

## Pourquoi la marge compte

Une fonction à 24 lignes sur 25 respecte la Norme et se trouve à une question du
jour de la soutenance de ne plus la respecter. `budget` existe pour rendre cela
visible avant qu'un évaluateur vous demande d'ajouter une vérification.

`normfix` indique le nombre ; il n'extrait jamais une fonction à votre place.
Choisir la frontière d'une fonction change la structure du programme, et c'est
une décision qui a besoin d'un nom et d'un responsable. Voir
[`normfix explain TOO_MANY_LINES`](/fr/commands/explain).
