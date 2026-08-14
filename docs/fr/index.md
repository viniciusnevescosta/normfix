---
layout: home
title: normfix en français
description: Corrections automatiques sûres et diagnostics utiles pour la Norme de 42.
hero:
  name: normfix
  text: Des corrections sûres pour la Norme de 42
  tagline: >-
    Une seule commande corrige les erreurs mécaniques d’un projet 42 entier, et
    explique celles qui méritent votre temps. Ce sont vos heures, la ressource
    rare.
  actions:
    - theme: brand
      text: Pourquoi normfix
      link: /fr/why
    - theme: alt
      text: Bien démarrer
      link: /fr/guide/getting-started
    - theme: alt
      text: L’essayer dans le navigateur
      link: /fr/guide/playground

features:
  - title: Il ne change que ce qu'il peut prouver
    details: >-
      Une modification touche exactement le morceau sur lequel il a prouvé
      quelque chose, et rien d'autre. Ce qu'il ne peut pas prouver, il le signale
      et n'y touche pas — vous pouvez donc le lancer en plein travail et lire le
      diff.
  - title: Le dernier mot revient à la Norminette
    details: >-
      normfix ne discute jamais avec l'outil qui vous évalue. Il lance le
      vérificateur officiel avant et après ses modifications, et jette tout lot
      qui a aggravé les choses.
  - title: Rien ne se perd
    details: >-
      Chaque fichier qu'il réécrit est d'abord copié hors de votre projet.
      `normfix undo` annule une exécution, et refuse si vous avez touché à ces
      fichiers depuis.
  - title: Essayez sans rien installer
    details: >-
      Le playground tourne dans votre onglet. Rien n'est envoyé, il n'y a pas de
      compte, et personne ne regarde ce que vous y collez.
---

## Ce qu’est normfix

`normfix` met en forme et vérifie les fichiers C, les headers, les Makefiles et
les READMEs d’un projet 42. Ce n’est pas un outil de réécriture C généraliste :
il travaille dans les règles de mise en page de la Norme, considère la
[Norminette officielle](https://github.com/42School/norminette) comme l’autorité
sur ce que ces règles veulent dire, et refuse de deviner dès que la syntaxe C
seule ne montre pas qu’un changement est sûr.

## Ce qu’il ne fera pas

Chaque limite ci-dessous est délibérée et documentée dans la
[politique de compatibilité](/fr/COMPATIBILITY) et
dans le [registre d’architecture](/fr/ARCHITECTURE) :

- il n'appellera pas « supportée » une version de la Norminette non testée : il
  dit laquelle il a trouvée et continue avec un avertissement ;
- il ne découpera pas une fonction longue à votre place, car choisir où la
  couper change la façon dont votre programme est construit ;
- il ne prouvera pas que votre programme ne fuit pas ; l'analyseur désigne une
  fuite probable, jamais l'absence de fuite ;
- il ne forcera pas 80 colonnes quand il n'existe aucun endroit sûr où couper la
  ligne ;
- il n'effacera rien sans que vous le demandiez, et jamais sans une copie depuis
  laquelle restaurer.
