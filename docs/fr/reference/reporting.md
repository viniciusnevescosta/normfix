# Rapports, codes de sortie et performance

## Lire un diagnostic

Chaque diagnostic est montré face au code qu'il concerne, pour aller droit à la
ligne au lieu de chercher une coordonnée :

```text
error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
  --> srcs/sort/sort.c:30:3
   |
30 |         sort_medium(ctx);
   |         ^^^^^^^^^^^ call to undeclared function 'sort_medium'
   |
  ::: srcs/sort/sort_adaptive.c:21:3
   |
21 |         sort_medium(ctx);
   |         ^^^^^^^^^^^ call to undeclared function 'sort_medium'
   |
   = help: Fix this strict -Wall/-Wextra/-Werror compiler diagnostic, then rerun normfix.
   = source: C compiler
   = explain: normfix explain CC_IMPLICIT_FUNCTION_DECLARATION
```

Les accents circonflexes couvrent les octets exacts que la règle concerne, pas
seulement leur premier caractère. Les occurrences d'une même règle sont groupées
sous un seul titre, chacune étiquetée de son propre message, et l'aide, les
notes, l'origine et l'indication `explain` communes sont indiquées une fois pour
le groupe au lieu d'être répétées sous chaque occurrence.

La vue par défaut montre les trois premières occurrences d'une règle et indique
combien elle en a retenu, car un projet peut porter des milliers d'un même
diagnostic. `--verbose` les montre toutes, chacune dans sa propre section avec
son propre extrait.

Quelques détails utiles à connaître :

- Les tabulations sont développées, pour que l'accent tombe sous le bon
  caractère.
- Les caractères de contrôle de votre source sont affichés en images visibles et
  n'atteignent jamais le terminal comme des contrôles.
- La colonne de la ligne `-->` est comptée en caractères, la convention d'un
  compilateur C. La Norminette officielle compte des colonnes d'affichage, si
  bien que sa sortie peut nommer une colonne plus grande pour le même caractère
  sur une ligne indentée par tabulations. L'accent est la réponse qui fait
  autorité sur *où*.
- Un diagnostic de compilateur qui appartient à un fichier sans position à
  l'intérieur, généralement parce que l'emplacement réel est dans un en-tête
  inclus, nomme le fichier et l'en-tête plutôt que de dessiner un accent sur du
  code sans rapport.

Le rendu utilise [`annotate-snippets`], la bibliothèque avec laquelle `rustc`
rend ses propres diagnostics.

[`annotate-snippets`]: https://crates.io/crates/annotate-snippets

## Le reste de la sortie

- un tableau d'état par fichier : `CLEAN`, `INFO`, `FIXED`, `WOULD FIX`,
  `REVIEW` ou `FAILED` ;
- des identifiants de règle stables, l'aide commune, les notes, l'origine du
  diagnostic et une indication `normfix explain RULE` ;
- des détails facultatifs des corrections acceptées avec `--verbose` ;
- des diffs unifiés avec `--diff` ;
- des décomptes agrégés et le temps écoulé.

La couleur n'est activée que pour un stdout interactif. `--no-color`,
`NO_COLOR`, la sortie JSON et la sortie redirigée sont sans couleur. Les extraits
sont rendus sur une largeur fixe, si bien qu'un rapport se lit de la même façon
sur deux machines.

Avant la découverte, le mode humain écrit un bloc compact
`normfix · starting` sur `stderr` avec l'action, la portée effective, le mode
écriture/vérification, l'origine de l'identité, la politique du vérificateur, les
processus, le cache, la vérification du compilateur, les sauvegardes et les
capacités destructives demandées. Cela rend évidente une exécution accidentelle à
la racine ou dans le répertoire personnel avant que le travail ne commence. Ces
portées protégées refusent sans `--force`.

Le mode JSON écrit à la place un objet d'événement `execution_start` sur
`stderr`. Le rapport final versionné reste l'unique document JSON sur `stdout`,
si bien que l'automatisation existante peut continuer à l'analyser comme une
seule valeur.

`--format json` émet un schéma déterministe et formaté avec
`schema_version: 2`. Il inclut les métadonnées d'identité, les résultats de
découverte et de quarantaine, les champs changement/écriture/échec par fichier,
les corrections, les diagnostics avant/après, les décomptes de résumé, le
`evaluation` facultatif du preflight et `duration_seconds`. Les tampons de source
et les diffs unifiés sont volontairement omis.

`normfix preflight` ajoute une estimation déterministe et explicitement non
concluante : `score`, `grade`, `verdict` et `hard_failures` situés exactement. Le
verdict est `hard_fail` quand la portée évaluée contient un fichier inattendu, un
constat corroboré par la Norminette officielle installée ou un diagnostic de
Makefile. Les indices de Norminette et de Makefile viennent de l'instantané
d'origine sur le disque, plus tout échec supplémentaire exposé dans l'ombre
finale. Un problème corrigible automatiquement reste donc un échec du preflight
tant que les octets proposés ne sont pas réellement écrits puis revérifiés.
La note numérique est une heuristique bornée de priorisation, pas une note 42 ;
elle ne couvre ni le comportement à l'exécution, ni les tests propres au projet,
ni les fuites, ni le jugement des pairs, ni les questions de soutenance.

Voici l'objet `evaluation` d'une exécution réelle, sur un projet dont le seul
problème restant est un Makefile listant une source supprimée :

```json
{
  "schema_version": 2,
  "evaluation": {
    "conclusive": false,
    "score": 59,
    "grade": "fail",
    "verdict": "hard_fail",
    "hard_failures": [
      {
        "rule_id": "MAKEFILE_SOURCE_NOT_FOUND",
        "path": "Makefile",
        "line": 14,
        "column": 20,
        "message": "The literal Makefile source `ghost.c` does not exist below the project root."
      }
    ],
    "notes": [
      "Incomplete means discovery or file analysis failed, or no processable file was covered; no grade can be inferred from that run.",
      "Hard fail: an unexpected project file, a finding corroborated by the installed official Norminette, or a Makefile finding was present.",
      "The score deducts bounded category weights for those findings, other warnings, operational failures, and pending edits; it is not a 42 grade.",
      "Runtime behavior, subject-specific tests, peer judgment, leaks, and defense questions remain outside this estimate."
    ]
  }
}
```

`conclusive` vaut `false` dans tout rapport que cet outil peut produire ; il
existe pour qu'un consommateur n'ait jamais à déduire cette limite de la prose.
`notes` fait partie du document plutôt que de la décoration du terminal, si bien
qu'un agent qui relaie le résultat emporte les réserves avec lui. Lisez
`verdict` pour la décision et `score` uniquement pour ordonner le travail : le
verdict reste `hard_fail` tant qu'il subsiste un échec, aussi haute que grimpe la
note.

### Codes de sortie

| Code | Signification |
|---:|---|
| `0` | Le mode correction est allé au bout sans diagnostic bloquant, ou l'entrée était déjà propre |
| `1` | Des diagnostics manuels subsistent, le mode aperçu a trouvé des changements proposés/candidats à la quarantaine, ou le preflight a déclenché une règle d'échec |
| `2` | Échec de découverte, de configuration, d'outil, d'E/S, de transaction ou de quarantaine |
| `130` | Une relecture interactive fichier par fichier a été annulée |

Les avis informatifs ne font pas échouer une exécution.

## Cache et performance

L'analyse des fichiers s'exécute en parallèle via Rayon. `--threads N` crée un
pool local avec un nombre exact de processus ; sans cela, Rayon utilise le
matériel disponible. Les résultats et les validations sont triés par chemin, si
bien que l'ordre d'achèvement des processus ne change ni l'ordre du rapport ni
l'ordre d'écriture.

Les rapports de la Norminette officielle utilisent à la fois un cache d'exécution
en mémoire et une base redb persistante hors du projet. Sous Unix :

```text
$XDG_CACHE_HOME/normfix/<project-id>/cache-v1.redb
```

ou :

```text
~/.cache/normfix/<project-id>/cache-v1.redb
```

Les clés incluent le schéma, l'espace de noms de l'analyse, le chemin relatif au
projet quand l'entrée est dans la racine de l'exécution (avec repli sur le chemin
absolu pour une entrée externe explicite), les octets de la source, la
configuration de la Norme et l'empreinte vérifiée de l'exécutable. Les échecs de
verrou, d'E/S, de décodage ou de corruption du cache échouent ouverts comme des
absences ; ils ne changent jamais les diagnostics ni le code de sortie. Une base
corrompue est conservée sous un nom `.corrupt-N` avant d'être recréée.

Utilisez `--no-cache` pour une exécution entièrement sans cache.
