# Ce qu'est normfix, et pourquoi

## Le but

La ressource la plus rare d'un étudiant de 42, c'est le temps. Pas le talent, pas
l'effort. Des heures. Et une part importante de ces heures part dans les
espaces : corriger l'indentation, déplacer des déclarations, couper des lignes à
80 colonnes, coller des en-têtes. Sur un cursus qui représente des milliers de
fichiers, projet après projet, et rien de tout cela ne vous apprend quoi que ce
soit la deuxième fois.

`normfix` existe pour vous rendre ces heures. Il corrige, en une commande et sur
tout un projet, les erreurs qui sont mécaniques, et refuse de toucher à celles
qui parlent vraiment de votre programme, car ce sont celles qui méritent votre
temps.

## En un paragraphe

Vous écrivez du C pour un projet 42. La
[Norminette officielle](https://github.com/42School/norminette) vous dit que la
ligne 47 a la mauvaise indentation, qu'une fonction est trop longue, qu'une
déclaration est au mauvais endroit, puis s'arrête, car signaler est tout ce
qu'elle fait.
`normfix` lit le même projet, corrige les erreurs dont il peut prouver qu'il est
sûr de les corriger, et explique le reste avec des mots plutôt qu'avec un nom de
règle. C'est une commande qui laisse votre projet plus près de réussir qu'elle ne
l'a trouvé, ou vous dit exactement pourquoi elle n'a pas pu.

```sh
cd chemin/vers/un/projet-42
normfix
```

C'est toute l'interface. Aucun fichier de configuration n'est requis, rien n'est
envoyé nulle part, et chaque fichier qu'il réécrit est d'abord sauvegardé hors du
projet.

## Le problème

La Norme 42 est un standard de disposition : de vraies tabulations, 80 colonnes,
une déclaration par ligne, une ligne vide après le bloc de déclarations, 25
lignes par fonction, cinq fonctions par fichier, un en-tête officiel en haut de
chaque fichier. Rien de cela n'est difficile. Tout cela est fastidieux, et tout
cela est vérifié par un outil qui ne sait dire que *non*.

Alors la veille d'une soutenance, vous faites l'une de deux choses : éditer des
espaces à la main dans quarante fichiers, ou lancer un formateur générique en
espérant. Les deux tournent mal. La première est lente et quelque chose vous
échappera. La seconde est pire, car un formateur qui ne connaît pas la Norme
produira avec assurance du code que la Norminette rejette, et réécrira tout votre
fichier pour cela, si bien que vous ne pouvez plus distinguer ce qu'il a changé
de ce que vous aviez écrit.

## Ce que normfix fait différemment

**Il prend le vérificateur officiel comme autorité.** La Norminette installée
s'exécute avant et après chaque lot de modifications. Si un lot introduit une
violation de règle qui n'existait pas, tout le lot est annulé et vos octets
d'origine restent. La version 3.3.59 est la référence de compatibilité testée ;
une autre version installée reste utilisable, mais est nommée dans un avis
marqué, car les règles natives n'ont pas reçu la même validation. `normfix` ne
discute jamais avec l'outil qui vous évalue vraiment.

**Il modifie des plages d'octets étroites, pas des fichiers entiers.** Un
changement touche la plage sur laquelle il a prouvé quelque chose et rien
d'autre, si bien que le diff est relisable et que le reste de votre fichier reste
identique octet pour octet. C'est pour cela que vous pouvez le lancer sur du
travail en cours.

**Il refuse plus qu'il n'accepte.** Réordonner des includes à travers un `#ifdef`
pourrait changer quelles déclarations existent, il s'arrête donc à la condition.
Extraire une fonction d'un corps de 40 lignes exige de nommer la nouvelle
fonction, ce qui est une décision de conception, il signale donc la longueur et
vous laisse décider. Chaque refus vient avec la raison et l'étape suivante.

**Tout ce qu'il écrit est récupérable.** Les écritures passent par une seule
transaction avec sauvegardes externes et journal. `normfix undo` restaure une
exécution, et refuse de le faire si vous avez modifié ces fichiers depuis.

## Ce qu'il ne fera pas

Voici la liste honnête, et elle est la raison d'être de l'outil, pas une limite
de la version actuelle :

- Il n'extraira pas une fonction longue à votre place.
- Il ne repensera pas le flux de contrôle, ne renommera pas à l'échelle d'un
  projet et ne changera pas une signature publique.
- Il ne prouvera pas que votre programme est sans fuite. La passe de l'analyseur
  peut suggérer une fuite ; elle ne peut pas prouver son absence.
- Il ne qualifiera pas de « prise en charge » une version non testée de
  Norminette. Il continue avec un avis de compatibilité visible pour qu'une mise
  à jour de 42 ne rende pas l'outil inutilisable, tandis que
  `--strict-norminette-version` rétablit le comportement d'échec fermé.
- Il ne garantira pas 80 colonnes quand aucune coupure sûre n'existe. Une longue
  chaîne ou une macro reste longue et est signalée.

## Où il s'insère

| Moment | Commande |
|---|---|
| Pendant l'écriture | `normfix --changed` sur ce que vous venez de toucher |
| Avant de valider | `normfix --check` comme garde-fou ; le code de sortie `1` signifie qu'il reste du travail |
| En relecture | `normfix lint --format json` pour un diagnostic sans modification |
| Avant une soutenance | [`normfix preflight`](/fr/commands/preflight), qui ajoute la passe stricte du compilateur |
| Après une mauvaise exécution | [`normfix undo`](/fr/commands/undo) |

## La règle sur laquelle il est bâti

> Changez ce qui peut être prouvé, expliquez ce qui ne peut pas l'être, et ne
> transformez jamais l'incertitude en permission.

Chaque décision de conception dans [l'architecture](/ARCHITECTURE) découle de
cette phrase, y compris celles qui font que l'outil en fait moins qu'il ne le
pourrait.

## Ensuite

- [Premiers pas](/fr/guide/getting-started) : installez-le et faites une première
  exécution réversible.
- [Commandes](/fr/commands/) : une page par sous-commande, avec de vraies
  sorties.
- [Toutes les options](/fr/reference/flags) : ce que fait chacune, avec un
  exemple.
- [Playground dans le navigateur](/fr/guide/playground) : essayez le formateur
  sans rien installer.
