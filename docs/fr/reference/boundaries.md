# Limites connues

Chaque limite ci-dessous est délibérée. Les lire est le moyen le plus rapide de comprendre à quoi sert l'outil.

- La compatibilité exacte est testée sur Norminette 3.3.59 ; les autres versions
  analysables s'exécutent avec un avis marqué, sauf si le mode strict de version
  est activé.
- Les fichiers C doivent être de l'UTF-8 valide et ne contenir aucun octet NUL.
- La récupération de Tree-sitter ou des octets de bande non classés désactivent
  les modifications conscientes de la syntaxe pour ce fichier.
- La passe stricte par défaut du compilateur utilise un contexte d'includes
  déduit de façon conservatrice ; les définitions propres au projet, le mode de
  langage, les fichiers générés, les options de cible, l'édition de liens et le
  comportement à l'exécution restent la responsabilité du projet.
- Le `-fanalyzer` de GCC peut suggérer d'éventuelles fuites, mais ne peut pas
  prouver leur absence.
- Le formateur ne déduit ni l'architecture du projet, ni des contrats
  d'évaluation cachés, ni l'intention d'une API publique, ni l'appartenance à une
  cible.
- L'extraction de fonctions longues est suggérée, jamais effectuée
  automatiquement.
- Un résultat strict à 80 colonnes n'est garanti que lorsqu'une coupure sûre
  existe. Les littéraux longs, les commentaires, les directives et les
  expressions ambiguës restent des avertissements.
- La transaction de source est récupérable et ordonnée, mais un système de
  fichiers n'offre pas un renommage atomique unique couvrant plusieurs fichiers ;
  la restauration est la stratégie d'échec inter-fichiers.

## Analyseurs qui ne sont pas intégrés

`--analyzer` utilise ce que le compilateur fournit déjà : `-fanalyzer` sur GCC,
l'analyseur statique de Clang sinon. Les autres outils vous sont délibérément
laissés, car chacun demande une compilation ou une exécution que `normfix`
refuse d'effectuer :

| Outil | Pourquoi il n'est pas exécuté |
|---|---|
| `valgrind`, `leaks` | Outils d'exécution. Ils demandent un binaire lié et une charge de travail, et `normfix` ne compile ni n'exécute jamais votre programme. |
| [AddressSanitizer](https://clang.llvm.org/docs/AddressSanitizer.html), [LeakSanitizer](https://clang.llvm.org/docs/LeakSanitizer.html), UBSan | Compilations instrumentées, pour la même raison. `preflight` donne une recette séparée de compilation de débogage sans modifier le Makefile rendu. |
| [clang-tidy](https://clang.llvm.org/extra/clang-tidy/index.html) | Il lui faut la vraie base de compilation du projet, les chemins d'includes, les définitions et les options de cible. `preflight` indique s'il est disponible, mais ne devine pas une commande. |
| `cppcheck`, `scan-build` | Installations séparées avec leur propre configuration de projet ; les intégrer reviendrait à deviner votre compilation. |

La règle derrière ces quatre lignes est la même que derrière tout le reste : un
résultat que cet outil ne peut pas reproduire et expliquer n'est pas un résultat
qu'il signalera.
