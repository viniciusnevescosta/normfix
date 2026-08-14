# `normfix preflight`

Les vérifications en lecture seule qu'il vaut la peine de lancer juste avant une
évaluation 42, avec la passe stricte du compilateur activée.

```sh
normfix preflight
```

Il exécute tout ce qu'exécute [`check`](/fr/commands/check), plus
`cc -fsyntax-only -Wall -Wextra -Werror` sur les vraies unités de traduction
présentes sur le disque.

```console
$ normfix preflight
error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
  srcs/sort/sort.c:30:5           call to undeclared function 'sort_medium'
  srcs/sort/sort_adaptive.c:21:5  call to undeclared function 'sort_medium'
    note: Compiler diagnostics inspect the original on-disk translation unit
          and never authorize or reject formatter edits.
 = help: Fix this strict compiler diagnostic, then rerun normfix.
 = source: C compiler
```

Cet exemple est réel : un en-tête déclarait `sort_medium` mais aucun fichier ne
le définissait, donc le projet ne compilait pas. Norminette ne vous l'aurait
jamais dit.

## Une exécution complète, avant et après

Toute la sortie de cette page vient d'une exécution réelle. Le projet ci-dessous
compte quatre fichiers : `main.c` et `add.c` indentés avec des espaces, un
`demo.h` déclarant un `unused_api` que personne n'implémente, et un Makefile dont
`SRC` liste encore un `ghost.c` supprimé.

Preflight annonce ce qu'il va faire avant de lire quoi que ce soit :

```console
$ normfix preflight
normfix · starting
  action       preflight
  mode         read-only check
  scope        /home/student/demo (recursive)
  working dir  /home/student/demo
  identity     student@student.42.fr (user config)
  workers      auto
  checks       Norminette + strict compiler
  norminette   automatic PATH discovery
  version rule advisory (other releases continue)
  timeout      5s per file
  cache        enabled
  gitignore    not applied
  backups      automatic external backup
  destructive  none
  force        no
```

Puis il signale l'estimation face aux octets actuellement sur le disque :

```console
Pre-defense estimate: HARD FAIL | grade FAIL | 31/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:1:1 [INVALID_HEADER] The official 42 Makefile header is missing or malformed
  add.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  demo.h:1:1 [INVALID_HEADER] Missing or invalid 42 header
  main.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  Makefile:2:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
  add.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:5:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:8 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:7:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:8:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:7:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:8:1 [TOO_FEW_TAB] Missing tabs for indent level
```

L'essentiel de cette liste est exactement ce que `normfix` répare. En lançant la
correction par défaut puis en redemandant :

```console
$ normfix
$ normfix preflight
Pre-defense estimate: HARD FAIL | grade FAIL | 59/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:14:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
```

Treize échecs ont disparu et un subsiste, et c'est le résultat utile : le
`ghost.c` supprimé est toujours listé dans le Makefile, et aucun outil ne devrait
décider seul si ce fichier doit revenir ou si la ligne doit partir. Le verdict
reste `HARD FAIL` tant qu'il subsiste un échec — la note bouge, le verdict ne
s'adoucit pas.

Les octets évalués sont les octets rendus. À la première exécution, `normfix`
avait déjà calculé les corrections de tous les `INVALID_HEADER` et
`SPACE_REPLACE_TAB` ci-dessus, et l'estimation a quand même échoué à cause d'eux,
parce qu'une réparation que vous n'avez pas écrite ne fait pas partie de ce
qu'ouvrira un évaluateur.

Tout flux adossé au système de fichiers, y compris la vérification par défaut,
compare les prototypes non statiques des en-têtes du projet à chaque fichier
C ou en-tête du projet qui a pu être lu sans erreur. Une implémentation absente, ou une
définition correspondante dont le corps n'est qu'accolades, espaces et
commentaires, est signalée sur le nom du prototype. Les sources générées et les
bibliothèques externes restent ambiguës. Le mode `--unsafe` explicitement
autorisé ne supprime qu'un prototype sans implémentation lorsque l'ensemble
complet des sources ne contient aucune définition, aucun appel, aucun
pointeur/référence de fonction, aucune macro, chaîne, condition, attribut ni
collage de jetons comme indice. Une définition existante réduite à des broutilles
n'est qu'un avertissement, car un no-op intentionnel peut être valide.

## Estimation et règles d'échec

Le rapport se termine par une estimation de 0 à 100, une lettre et un verdict. Il
est toujours étiqueté **non concluant**. C'est une aide à la priorisation, pas une
note 42 prédite.

Le verdict est `HARD FAIL` dès que l'une de ces conditions objectives est
présente :

- un fichier inattendu dans la portée évaluée ;
- un constat de Norme corroboré par la Norminette officielle installée ;
- un diagnostic statique de Makefile ou un échec de traitement du Makefile.

Chaque élément d'échec de source répète son `chemin:ligne:colonne` exact,
l'identifiant de règle et le message. Un échec opérationnel de Makefile nomme le
fichier sans inventer de coordonnée de source.
Les constats officiels de Norme et de Makefile sont évalués sur les octets
d'origine présents sur le disque ; une correction proposée en lecture seule ne
transforme pas le rendu actuel en réussite. Les nouveaux constats qui subsistent
dans l'ombre finale sont également inclus.
L'absence de README n'est pas un échec. Quand un README est présent, un avis
informatif vous demande de le comparer à la fiche de sujet/évaluation actuelle.
Si aucun Makefile ordinaire n'est sélectionné ni trouvé à la racine du projet,
`MAKEFILE_NOT_FOUND` signale que les vérifications de cibles de compilation et
de liste de sources n'ont pas eu lieu. C'est un avis et cela ne coûte aucun
point : un exercice de piscine est censé ne contenir que des fichiers `.c`, donc
le Makefile comme les en-têtes du projet y sont facultatifs. Seul le sujet peut
dire si un Makefile est exigé, et normfix ne lit pas les sujets.

## Ce qu'il ne fait pas

Il n'exécute pas `make`, ne lie pas de binaire, n'exécute ni votre programme ni
vos tests, et ne prouve pas l'absence de fuites. Cela reste à vous, et le rapport
le dit.

Preflight indique si `clang-tidy` est disponible dans le `PATH` et montre une
recette pratique de compilation de débogage avec
AddressSanitizer/UndefinedBehaviorSanitizer. Il n'exécute ni `clang-tidy`, ni les
sanitizers, ni `make` (pas même `make -n`, qui peut évaluer `$(shell ...)`), ni un
binaire du projet. Une telle exécution demande une confiance distincte et
explicite dans le comportement de compilation et d'exécution du projet.

Preflight ajoute automatiquement une passe bornée d'analyse statique profonde :
`-fanalyzer` sur GCC, `--analyze` sur Clang. Les flux ordinaires exigent toujours
`--analyzer`. `normfix` choisit d'après la bannière de version du compilateur, ce
qui compte car `/usr/bin/gcc` sur macOS est Clang sous un autre nom.

Ils peuvent *suggérer* une fuite ou un accès invalide ; ils ne prouvent jamais la
correction, et n'autorisent jamais une modification. Un compilateur sans aucun
analyseur signale `CC_ANALYZER_UNAVAILABLE` et l'exécution continue.

`preflight` refuse de se combiner avec `--no-compiler-preflight`, car la passe du
compilateur est la raison d'être de la commande.
