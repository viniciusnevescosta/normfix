# Makefiles, documents README et fichiers de projet

Les Makefiles utilisent un formateur conservateur dédié, car Norminette
n'analyse pas la syntaxe de GNU Make. Le formateur peut :

- retirer un BOM UTF-8 et normaliser les fins de ligne ;
- insérer ou mettre à jour l'en-tête officiel 42 au style `#` ;
- garantir un unique saut de ligne final ;
- compacter de façon gourmande les affectations explicites simples de `.c`
  jusqu'à 80 colonnes d'affichage, en conservant l'ordre et la sémantique de
  l'affectation.

Il préserve délibérément les recettes, les projets à `.RECIPEPREFIX`, les blocs
`define`, les affectations shell, l'expansion de variables/fonctions, les motifs,
les commentaires, les guillemets, les séparateurs de commandes et les autres
constructions ambiguës de Make.

L'analyseur signale :

- une affectation `NAME` manquante ;
- des règles `all`, `clean`, `fclean`, `re` ou `$(NAME)` manquantes ;
- le fait que `all` ne soit pas la cible concrète par défaut ;
- la découverte de sources/objets par joker ;
- des lignes longues qui ne peuvent pas être compactées sans risque ;
- des espaces après une barre oblique inverse de continuation ;
- une ligne de recette indentée avec des espaces, que Make refuse de lire.

Pour une affectation simple de style `SRC`/`SRCS` dont la valeur complète est
faite de chemins `.c` relatifs et littéraux, il vérifie aussi si chaque jeton
existe et si le fichier ordinaire référencé contient un jeton C au-delà des
espaces ou des commentaires. Les chemins sont résolus depuis le répertoire
contenant ce Makefile, y compris pour les Makefiles imbriqués, et chaque
composant doit rester dans la racine canonique du projet et éviter les liens
symboliques. Un chemin manquant ou réduit à des broutilles est signalé par
défaut. `--unsafe` peut ne retirer que le jeton exact prouvé et recompacter la
liste restante sans la réordonner. Les expansions, motifs, guillemets,
commentaires, recettes, blocs `define`, `.RECIPEPREFIX`, chemins qui s'échappent
et résultats incertains du système de fichiers restent inchangés.

Tout flux adossé au système de fichiers compare les prototypes non statiques des
en-têtes du projet à un instantané complet et sans perte des sources
C/en-têtes. Les implémentations absentes et les corps correspondants réduits à
des broutilles sont signalés sur le nom du prototype. La suppression non sûre est
limitée aux implémentations absentes et exige la portée complète du projet, une
autorisation délimitée, aucun autre usage de l'identifiant ni ambiguïté, une
validation par réanalyse dans l'ombre et une vérification d'empreinte, au moment
de la transaction, de toutes les entrées de la preuve. Les définitions existantes
réduites à des broutilles ne sont jamais supprimées : un no-op intentionnel peut
être valide.

L'outil n'ajoute pas automatiquement chaque fichier `.c` trouvé sur le disque à
une variable de sources. L'appartenance à une cible est une décision de
conception de la compilation.

## Preflight du compilateur et avis de fuites

Pour chaque fichier `.c` sélectionné, le pipeline normal exécute une passe en
lecture seule du compilateur équivalente à :

```text
cc -fsyntax-only -Wall -Wextra -Werror
```

Il ajoute des chemins `-I` stables pour les répertoires contenant des en-têtes de
projet découverts, mais il ne devine ni les définitions propres au sujet, ni les
modes de langage, ni les en-têtes générés, ni les options de cible, ni les
entrées de l'éditeur de liens. Utilisez `--cc PATH` pour choisir un compilateur
précis ou `--no-compiler-preflight` pour sauter la passe. Les constats du
compilateur ne sont que des diagnostics : ils n'autorisent ni ne rejettent jamais
une modification du formateur. Un compilateur indisponible ou un contexte de
compilation visiblement incomplet produit un avis clair qui échoue ouvert.

`--analyzer` demande en plus au compilateur choisi la sortie `-fanalyzer` de GCC
dans les flux ordinaires. Preflight effectue cette passe bornée de l'analyseur
automatiquement. Elle peut faire apparaître d'éventuelles fuites d'allocation et
des chemins d'accès invalide, mais elle est plus lente et volontairement
informative. Ce n'est pas une preuve d'absence de fuite : l'exploration des
chemins est incomplète, une unité de traduction est inspectée à la fois, et la
propriété cachée derrière des fonctions externes ou stockée dans des structures
peut échapper à l'analyse. Un compilateur sans aucune des interfaces d'analyseur
prises en charge est signalé et ignoré.

### Mode avant soutenance

```sh
normfix preflight
```

`preflight` est l'aperçu en lecture seule formateur/linter prévu juste avant
l'évaluation. Il agrège les résultats officiels de Norminette, les limites
natives de la Norme et les suggestions d'extraction, les en-têtes officiels et
les gardes d'en-tête, la politique de fonctions autorisées, la structure du
Makefile et les références littérales de sources, les sources de Makefile
réduites à des broutilles, les prototypes d'en-tête sans définition dans le
projet, les corps d'implémentation réduits à des broutilles, les fichiers
inattendus, les constats de README, la passe stricte du compilateur et
l'analyseur du compilateur. Les passes du compilateur ne peuvent pas être
désactivées dans ce flux.

La `Pre-defense estimate` finale est volontairement non concluante. Les fichiers
inattendus, les constats de la Norminette installée et les diagnostics de
Makefile produisent un échec avec des emplacements de source exacts. La note de
0 à 100 et la lettre ne font que prioriser le travail restant ; ce n'est pas une
note officielle.

Les indices d'échec reposent sur les diagnostics d'origine de Norminette et du
Makefile présents sur le disque, plus tout constat nouvellement exposé qui
subsiste dans l'ombre. Une modification sûre proposée par le mode check ne fait
pas réussir rétroactivement les octets rendus.

Quand `normfix.toml` est absent, preflight émet
`FUNCTION_POLICY_NOT_CONFIGURED` au lieu de faire comme si la vérification des
fonctions autorisées avait eu lieu. Il émet aussi `PREFLIGHT_MANUAL_STEPS` : la
commande n'exécute délibérément pas les recettes Make, ne lie ni n'inspecte le
binaire final, n'exécute ni le programme ni les tests, et n'invoque pas d'outils
de fuite à l'exécution. Effectuez ces étapes propres au projet séparément. Il
indique si `clang-tidy` est dans le `PATH` et donne des conseils séparés de
sanitizers pour une compilation de débogage, mais n'exécute ni l'un ni l'autre.
Quand aucun Makefile ordinaire n'est sélectionné ni trouvé à la racine du projet,
`MAKEFILE_NOT_FOUND` signale une vérification incomplète sans faire échouer :
seule une politique propre au sujet peut prouver que tout projet en a besoin.

## Prise en charge du README et du Markdown

Les fichiers README sont analysés avec Comrak et réimprimés canoniquement par
défaut :

```sh
normfix README.md
```

La réimpression canonique est idempotente, mais peut créer un large diff à la
première exécution. Utilisez `--check` ou `--diff` pour la prévisualiser.
`--no-format-markdown` garde les fichiers README en lecture seule tout en
signalant les sauts de niveau de titre, les espaces en fin de ligne et l'absence
de saut de ligne final.

Quand preflight découvre un README, `README_42_CRITERIA_REVIEW` vous rappelle de
le comparer à la fiche de sujet et d'évaluation actuelle. L'absence de README
n'émet aucun diagnostic et ne fait jamais échouer le preflight.

## Fichiers de projet inattendus

La découverte récursive signale les fichiers ordinaires autres que `.c`, `.h`,
`Makefile`, les variantes de README, `.normfixignore` et son alias historique
`.norminetteignore`. Hors preflight, cet avertissement seul ne change pas le code
de sortie. Preflight l'utilise comme règle explicite d'échec, car la portée de
rendu évaluée est censée ne contenir que des fichiers de projet pris en charge.
Cela n'implique jamais qu'un fichier soit jetable.

N'utilisez `--remove-unexpected` que si vous comptez déplacer tous les fichiers
ordinaires inattendus éligibles vers la quarantaine externe. Les liens
symboliques, les répertoires, les chemins hors du projet, les instantanés
modifiés et les chemins de récupération qui se chevauchent sont rejetés.
