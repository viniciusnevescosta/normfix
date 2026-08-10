---
layout: home
title: normfix en français
description: Corrections automatiques sûres et diagnostics utiles pour la Norme de 42.
hero:
  name: normfix
  text: Formatez avec preuve. Examinez le reste.
  tagline: Un outil prudent pour les projets C de 42, avec la Norminette officielle comme autorité.
  actions:
    - theme: brand
      text: Bien démarrer
      link: /fr/guide/getting-started
    - theme: alt
      text: Découvrir le playground
      link: /fr/guide/playground
---

## Ce que fait normfix

`normfix` corrige automatiquement uniquement les transformations dont la sûreté
peut être démontrée. Le reste devient un diagnostic localisé et exploitable.

- traite un projet complet ou des fichiers `.c`, `.h`, `Makefile` et README précis ;
- ajoute l’en-tête officiel lorsqu’une identité 42 valide est disponible ;
- utilise la [Norminette officielle](https://github.com/42school/norminette) comme autorité de compatibilité ;
- propose aperçu, diff, portées Git, budget des fonctions, sauvegardes et undo dans la CLI ;
- fournit un playground privé en WebAssembly, sans envoyer le code à un serveur.

La référence complète reste en anglais. Les pages essentielles d’installation
et du playground sont disponibles en français.
