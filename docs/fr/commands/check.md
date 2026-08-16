# `normfix check`

Exécute tout le pipeline de correction en mémoire et signale le résultat sans
toucher un seul fichier.

```sh
normfix check
normfix check main.c
```

`normfix --check` fait la même chose.

```console
$ normfix check
Files
STATUS      FIXES  REMAINING  INFO  FILE
REVIEW        1          1     0  Makefile
WOULD FIX     2          0     0  add.c
REVIEW        3          1     0  demo.h
WOULD FIX     6          0     0  main.c

Summary: fichiers : 4 | proposés : 4 | écrits : 0 | corrections : 12 | restants : 2 | informatifs : 0 | en échec : 0 | inattendus : 0 | en quarantaine : 0
Completed in 578 ms.
```

`WOULD FIX` et `4 proposed` font la différence avec [`lint`](/fr/commands/lint) :
`check` planifie les modifications et dit combien ont franchi les preuves, il ne
les valide simplement pas.

Les deux statuts répondent à des questions différentes. `WOULD FIX` signifie que
tout ce qui a été trouvé dans ce fichier a une correction prouvée qui attend —
`add.c` et `main.c` n'ont besoin de rien de votre part. `REVIEW` signifie qu'il
reste quelque chose une fois toute correction sûre appliquée, et la colonne
`REMAINING` le compte : ici le Makefile liste une source qui n'existe pas et
`demo.h` déclare une fonction que personne n'implémente. Aucune des deux n'a de
réponse automatique sûre, donc les deux sont signalées plutôt que devinées.

En lisant le résumé de gauche à droite : 4 fichiers ont été analysés, 4 ont des
changements proposés, aucun n'a été écrit puisque c'est `check`, 12 corrections
individuelles ont franchi leurs preuves et 2 constats ont encore besoin d'une
personne.

## Lisible par une machine

```console
$ normfix check --format json
{
  "schema_version": 2,
  "tool_version": "1.9.0",
  "mode": "check",
  "summary": {
    "files": 4,
    "changed": 4,
    "written": 0,
    "fixes": 12,
    "remaining": 2,
    "advisories": 0,
    "failed": 0,
    "unexpected_files": 0,
    "quarantine_candidates": 0,
    "quarantined": 0
  },
  "evaluation": null
}
```

Branchez toujours sur `schema_version` avant de lire le reste. La sortie humaine
peut s'améliorer d'une version à l'autre ; la structure du JSON, non.

## S'en servir comme garde-fou

```sh
normfix check --format json > report.json || exit 1
```

Le code de sortie `1` ici signifie « il reste du travail », ce qui est exactement
ce que veut une vérification avant fusion.
