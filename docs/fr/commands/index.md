# Commandes

L'interface sans sous-commande est le chemin le plus court pour formater un
projet, et c'est ce qu'utilisent la plupart des exécutions :

```sh
cd chemin/vers/un/projet-42
normfix
```

Les sous-commandes rendent l'intention explicite, ce qui compte dans les
scripts, en CI et pendant une relecture.

| Commande | Écrit | À utiliser quand |
|---|---|---|
| [`format`](/fr/commands/format) | oui | Vous voulez appliquer les modifications acceptées |
| [`lint`](/fr/commands/lint) | non | Vous voulez des diagnostics sur les octets présents sur le disque, sans rien proposer |
| [`check`](/fr/commands/check) | non | Vous voulez voir ce qu'une exécution de correction *ferait* |
| [`budget`](/fr/commands/budget) | non | Vous voulez la marge de lignes/variables/paramètres par fonction |
| [`preflight`](/fr/commands/preflight) | non | Vous allez soutenir et voulez les vérifications en lecture seule |
| [`explain`](/fr/commands/explain) | non | Vous voulez une règle expliquée sans rien analyser |
| [`undo`](/fr/commands/undo) | oui | Vous voulez restaurer une exécution précédente |
| [`upgrade`](/fr/commands/upgrade) | oui | Vous voulez la version la plus récente, vérifiée |
| [`uninstall`](/fr/commands/uninstall) | oui | Vous voulez retirer normfix de cette machine |

## Chaque exemple de ces pages est réel

La sortie affichée a été produite par `normfix 1.2.0` sur ce fichier :

```c
# include "libft.h"
# include <stdlib.h>

int add(int a,int b){
return a+b;
}

int	scale(int value, int factor)
{
	int result;
	result = value * factor;
	return result;
}
```

Il est volontairement en désordre de manières ordinaires : includes non triés,
définition de fonction repliée, espaces manquants, déclaration non séparée des
instructions et valeurs de `return` sans parenthèses.

## Codes de sortie

Toutes les commandes les partagent :

| Code | Signification |
|---:|---|
| `0` | Rien de bloquant : l'exécution était propre, ou le mode correction est allé au bout |
| `1` | Des diagnostics manuels subsistent, ou un aperçu a trouvé des changements proposés |
| `2` | Échec de découverte, de configuration, d'outil, d'E/S, de transaction ou de quarantaine |
| `130` | Une relecture interactive a été annulée |

Les avis informatifs ne changent jamais le code de sortie. Cela rend les codes
utilisables directement en CI :

```sh
normfix --check || echo "ce projet n'est pas encore conforme à la Norme"
```

## Options acceptées par toutes les commandes

`--format json` et `--no-color` changent la sortie ; `--threads`, `--timeout`,
`--no-cache` et `--norminette PATH` changent la façon dont l'exécution se
déroule. Le tableau complet est dans
[ligne de commande](/fr/guide/command-line).
