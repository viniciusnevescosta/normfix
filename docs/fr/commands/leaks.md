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

error[LEAK_DEFINITELY_LOST]: 1024 octets alloués ici n'ont jamais été libérés
 --> stack.c:23:2
   |
23 |     stack = malloc(sizeof(int) * size);
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: C'est là que la mémoire a été allouée, pas là où elle aurait dû être libérée. Suivez ce point jusqu'au chemin qui perd le pointeur.

error[MEMORY_ERROR]: Invalid read of size 4, dans sort_stack
 --> sort.c:41:2
   |
41 |     return (stack[size]);
   |     ^^^^^^^^^^^^^^^^^^^^
   |
   = help: Le programme a touché de la mémoire qui ne lui appartient pas. C'est un bug, quoi que la Norm dise du fichier.
Voici ce qu’une exécution a observé avec les arguments reçus. Ce n’est pas une preuve que le programme ne fuit jamais.
```

Deux sortes de constat apparaissent ici, et elles répondent à des questions
différentes. Un constat `LEAK_` désigne l'endroit où la mémoire a été allouée
puis perdue — la ligne qui l'a allouée, ce que le vérificateur peut voir, et non
l'endroit où elle aurait dû être libérée. Un `MEMORY_ERROR` désigne la ligne qui
a lu, écrit ou libéré quelque chose que le programme n'avait pas le droit de
toucher ; celle-là, c'est le bug lui-même.

Les arguments après `--` vont à votre programme, pas au détecteur : vous pouvez
donc exercer le chemin qui compte.

```sh
normfix leaks ./push_swap -- 5 2 9 1
```

Un binaire compilé sans `-g` ne porte pas de numéros de ligne : le rapport nomme
alors la fonction seule et dit pourquoi.
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
| macOS | Utilisez un environnement Linux ou WSL sur une autre machine. Les ports communautaires natifs ne sont pas acceptés comme moteur de résultat propre, car un test réel a montré que l’un d’eux pouvait manquer une fuite C connue |
| Windows | Exécutez normfix dans [WSL](https://learn.microsoft.com/windows/wsl/install), où le détecteur Linux fonctionne normalement |

normfix localise un `valgrind` compatible dans le `PATH`, vérifie son identité
et exige un rapport complet qu’il peut interpréter. Les ports communautaires
natifs de macOS échouent de façon fermée au lieu d’annoncer une exécution propre.
Lorsqu’aucun détecteur pris en charge n’est trouvé, normfix l’indique et donne la
voie compatible pour ce système.
