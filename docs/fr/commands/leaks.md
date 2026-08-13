# `normfix leaks`

Exécute un programme que vous avez déjà compilé sous un détecteur de fuites et
rapporte ce qu’il a observé.

```sh
normfix leaks ./libft_test
normfix leaks ./push_swap -- 3 1 2
```

Tout le reste de normfix lit votre code. Cette commande l’exécute : elle demande
donc d’abord.

```console
$ normfix leaks ./push_swap
normfix va exécuter ./push_swap sous le détecteur de fuites. Cela exécute votre programme. Continuer ? [y/N] y
Perdus 1024 octets définitivement, et 96 de plus accessibles uniquement par eux.
Alloués à :
  1024 octets dans create_stack (stack.c:23)
  96 octets dans push_node (node.c:41)
Voici ce qu’une exécution a observé avec les arguments reçus. Ce n’est pas une preuve que le programme ne fuit jamais.
```

Les arguments après `--` vont à votre programme, pas au détecteur : vous pouvez
donc exercer le chemin qui compte.

```sh
normfix leaks ./push_swap -- 5 2 9 1
```

La ligne est celle où la mémoire a été allouée, pas celle où elle aurait dû
être libérée — c'est ce que le vérificateur peut voir. Un binaire compilé sans
`-g` ne porte pas de numéros de ligne : le rapport nomme alors la fonction seule
et dit pourquoi.
## Ce qu’elle ne fait pas

`normfix` ne compile jamais votre programme. Compiler signifie exécuter les
recettes de votre Makefile, ce qui est une deuxième catégorie — bien plus large
— d’exécution de code que vous avez écrit ; et « vous l’avez compilé, je l’ai
exécuté » est une promesse bien plus petite que « je l’ai compilé et exécuté ».
Compilez comme d’habitude, puis pointez cette commande sur le résultat.

## Un résultat propre n’est pas une preuve

Le détecteur voit l’unique chemin qu’a pris votre programme avec les arguments
que vous lui avez donnés. Une exécution qui ne perd rien vous dit que ce chemin
est propre ; elle ne dit rien des chemins que vous n’avez pas empruntés. Cette
ligne accompagne chaque résultat pour la même raison que le reste de l’outil
signale ce qu’il ne peut pas prouver au lieu de l’affirmer.

La mémoire encore accessible à la sortie n’est pas comptée comme perdue. 42
évalue la mémoire que plus personne ne peut atteindre, et une arène que votre
programme conserve jusqu’à sa sortie n’en fait pas partie.

Si le détecteur produit une sortie que normfix ne peut pas lire comme un résumé
de fuites, c’est une erreur et non un résultat propre. Un détecteur qui a été tué
et un détecteur qui n’a rien trouvé produisent le même silence, et la différence
compte trop pour être devinée.

## Codes de sortie

| Code | Signification |
|---|---:|
| `0` | Rien n’a été perdu sur le chemin de cette exécution |
| `1` | Quelque chose a été perdu |
| `2` | Le détecteur est indisponible, a été refusé, ou n’a pas pu être lu |

Hors d’un terminal interactif — en CI, ou avec `--format json` — la confirmation
ne peut pas être donnée : `--force` est alors obligatoire.

```sh
normfix leaks --force ./libft_test
```

## Installer un détecteur

| Système | Comment |
|---|---|
| Linux, FreeBSD | Valgrind, depuis votre gestionnaire de paquets |
| macOS | [`LouisBrunner/valgrind-macos`](https://github.com/LouisBrunner/valgrind-macos), puisque Valgrind amont ne se compile pas sur macOS. Sa prise en charge d’Apple Silicon est limitée |
| Windows | Exécutez normfix dans [WSL](https://learn.microsoft.com/windows/wsl/install), où le détecteur Linux fonctionne normalement |

normfix localise `valgrind` dans le `PATH` et le vérifie par son propre
`--version` : n’importe quelle compilation fonctionnelle lui convient. Quand il
n’en trouve aucun, il le dit et nomme la voie pour le système sur lequel vous
êtes.
