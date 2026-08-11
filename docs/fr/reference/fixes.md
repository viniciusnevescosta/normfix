# Ce qui est corrigé, et ce qui ne l'est pas

Le formateur C natif traite actuellement des cas prouvés dans ces domaines :

- suppression du BOM UTF-8, normalisation des CRLF, espaces en fin de ligne,
  suites de lignes vides, espaces en début de fichier et un unique saut de ligne
  final ;
- indentation et espacement du préprocesseur, hors formes multilignes sensibles ;
- ordre du bloc d'includes : en-têtes système avant ceux du projet, par ordre
  alphabétique dans chaque catégorie ;
- lignes vides obligatoires et interdites autour des déclarations, des
  préprocesseurs et des fonctions ;
- accolades et corps de contrôle qui ont besoin de leur propre ligne physique ;
- disposition de contrôle Allman, suppression conservatrice des blocs redondants
  à instruction unique et nettoyage étroit d'un `else` redondant quand les deux
  branches retournent ;
- indentation par tabulations de quatre colonnes et diagnostics courants
  espace/tabulation ;
- indentation et ligne vide obligatoire suivante pour les groupes simples de
  déclarations locales initiales ;
- espacement autour des opérateurs, des pointeurs, des parenthèses, des
  mots-clés et des déclarateurs de fonction ;
- alignement de groupe pour les variables simples sur une ligne et les prototypes
  de fonction, y compris les déclarateurs de pointeur quand le groupe est sans
  ambiguïté ;
- `return value;` en `return (value);` ;
- listes de paramètres vides dans les définitions de fonction en `(void)` ;
- `return (0);` de retour-pointeur en `return (NULL);` quand le type de retour et
  un fournisseur visible de `NULL` sont tous deux prouvés ;
- retour à la ligne sur des opérateurs ou des virgules prouvés ;
- réunion gourmande des lignes de continuation tant que le résultat reste dans
  80 colonnes d'affichage.

Le compactage des lignes longues ne traverse ni commentaires, ni directives de
prétraitement, ni raccords de ligne, ni instructions sans rapport. Les chaînes et
les commentaires ne sont pas coupés. Les lignes de préprocesseur ne sont pas
réécrites uniquement pour respecter la largeur.

### Ordre des includes

Une suite de directives `#include` n'est réordonnée que tant que **chaque** ligne
est exactement une directive d'include. La première ligne qui est autre chose (un
commentaire, une ligne vide, une condition, une définition de macro ou du texte
après le délimiteur final) termine la suite, et les directives de chaque côté
sont triées indépendamment. Aucune directive n'est jamais déplacée à travers une
telle construction, car la traverser peut changer des déclarations, des macros de
fonctionnalité ou la compilation conditionnelle.

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

Le tri se fait d'abord par catégorie (`<système>` avant `"projet"`), puis par nom
d'en-tête, comparé sans tenir compte de la casse. Les noms identiques conservent
leur ordre relatif d'origine. Utilisez `--no-reorder-includes` pour laisser
chaque bloc intact ; le rapport se rabat alors sur l'avertissement
`INCLUDE_ORDER_REVIEW`.

Le formateur mesure des cellules d'affichage de terminal : les tabulations
utilisent des taquets de quatre colonnes, les signes combinants zéro cellule et
les caractères Unicode larges deux.

### Preuves obligatoires

La mise en forme a d'abord lieu uniquement en mémoire. Pour chaque action de
disposition :

- la source doit s'analyser sans régions `ERROR`, `MISSING` ou de bande
  inconnue ;
- la bande de jetons doit couvrir et reconstruire l'entrée complète ;
- l'empreinte ordonnée des jetons et des commentaires doit rester identique ;
- le candidat doit se réanalyser sans récupération ;
- les plages de modification doivent être valides et sans chevauchement.

Une fois le candidat complet produit, Norminette est relancée. Si un décompte de
règle augmente par rapport à la référence validée, le lot de mise en forme native
est annulé pour ce fichier. Les échecs opérationnels n'autorisent jamais une
écriture partielle.

Les actions étroites qui changent des jetons, comme `return (...)` et `(void)`,
sont des actions sémantiques distinctes avec leurs propres règles de
construction ; elles ne sont pas traitées comme des modifications génériques
d'espaces.

## Diagnostics qui restent manuels

Le rapport du terminal explique la règle, l'étendue exacte de source, l'origine
et une étape suivante concrète pour des travaux comme :

- des fonctions de plus de 25 lignes de corps ;
- plus de 4 paramètres, 5 variables locales ou 5 fonctions par fichier `.c` ;
- des lignes de plus de 80 colonnes sans coupure sûre sur opérateur/virgule ;
- des structures de contrôle interdites, des ternaires, `goto`, des étiquettes et
  des affectations dans les conditions ;
- la séparation déclaration/affectation et les déclarations après instructions ;
- des identifiants publics ou globaux à renommer dans tout le projet ;
- des déplacements de types/includes et des changements de structure du projet ;
- des déclarations ambiguës, des pointeurs de fonction, des attributs, des champs
  de bits et des déclarateurs multilignes ;
- du C malformé ou récupéré par l'analyseur ;
- des gardes d'en-tête qui échouent à la preuve fermée de l'arbre de travail.

La couche sémantique évalue un sous-ensemble conservateur d'expressions
constantes entières de C, y compris les constantes d'énumération. Cela permet de
signaler une borne d'énumération connue comme `count[op_total]` en tant que faux
positif informatif de compatibilité Norminette, plutôt qu'un vrai tableau de
longueur variable. Les expressions non prises en charge restent inconnues ; elles
ne sont jamais devinées.

Pour une fonction longue, le diagnostic suggère d'extraire une région cohérente
et indique le budget applicable. Il ne déplace jamais d'instructions, n'invente
pas de paramètres et ne crée pas de fonction auxiliaire automatiquement : le flux
de données, les noms, la visibilité et l'intention du projet ne peuvent pas être
prouvés à partir de faits de mise en forme seuls.
