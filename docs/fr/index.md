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
  - title: Uniquement des éditions prouvées
    details: >-
      Les règles proposent des remplacements sur des plages d’octets étroites,
      contre des tampons fantômes immuables. Une preuve qui échoue ne peut pas
      modifier un fichier à moitié, et tout ce qui est ambigu est signalé plutôt
      que réécrit.
  - title: C’est le vérificateur officiel qui tranche
    details: >-
      La Norminette officielle installée fait autorité. La version 3.3.59 est la
      base vérifiée ; une autre version reste utilisable, avec un avertissement
      de compatibilité visible.
  - title: Récupérable par construction
    details: >-
      Les écritures passent par une transaction unique et auditable, avec
      sauvegardes externes, journal, commits ordonnés, rollback et un undo qui
      échoue en position fermée devant toute cible modifiée.
  - title: Privé dans le navigateur
    details: >-
      Le playground WebAssembly réutilise le même analyseur natif et les mêmes
      actions C dans votre onglet. Aucun envoi, compte, analytique ni backend.
---

## Ce qu’est normfix

`normfix` formate et diagnostique le code C, les headers, les Makefiles et les
documents README des projets 42. Ce n’est pas un réécrivain C généraliste : il
opère sous les règles de mise en page physique de la Norme, garde la
[Norminette officielle](https://github.com/42School/norminette) comme autorité
de compatibilité, et refuse de deviner là où la syntaxe C seule ne peut pas
prouver qu’un changement est sûr.

## Ce qu’il ne fera pas

Chaque limite ci-dessous est délibérée et documentée dans la
[politique de compatibilité](/fr/COMPATIBILITY) et
dans le [registre d’architecture](/fr/ARCHITECTURE) :

- il ne revendique pas de compatibilité testée des règles natives pour une
  version de la Norminette autre que 3.3.59 ; il identifie cette version avant
  de continuer ;
- il n’extrait pas les fonctions trop longues à votre place, car choisir où une
  fonction s’arrête change la structure du programme ;
- il ne prouve pas l’absence de fuites, et la sortie de l’analyseur reste
  informative ;
- il ne garantit pas un résultat strict de 80 colonnes lorsqu’aucune coupure
  sûre n’existe ;
- il ne supprime rien sans une autorisation de capacité explicite et sans
  stockage externe récupérable.
