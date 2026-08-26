# `normfix lint`

Signale ce qui ne va pas dans les octets actuellement sur le disque. Il ne
propose rien et n'écrit rien : ni mise en forme, ni en-tête officiel, ni
modification du Makefile ou du README.

```sh
normfix lint
normfix lint src
```

Utilisez-le quand vous voulez le diagnostic sans le traitement : en CI, pendant
une relecture, ou quand vous comptez corriger quelque chose à la main et ne
voulez pas que l'outil bouge sous vos pieds.

Comme `lint` ne planifie jamais de modification, il refuse les options de mise
en forme, d'identité d'en-tête, de sauvegarde, de diff et de suppression, puis
indique une commande compatible. Aucune option ne semble ainsi fonctionner
tout en étant ignorée silencieusement. Utilisez `normfix check --diff` pour
voir les changements proposés.

## Ce qu'il signale

```console
$ normfix lint
warning[TOO_MANY_WS]: 2 occurrences in 1 file
  math_utils.c:1:1                     Extra whitespaces for indent level
  math_utils.c:2:1                     Extra whitespaces for indent level
 = help: Review this location and apply the named Norm rule manually; no
         semantics-preserving automatic edit was proven.
 = source: official Norminette 3.3.59 compatibility
 = explain: normfix explain TOO_MANY_WS

Summary: fichiers : 1 | proposés : 0 | écrits : 0 | corrections : 0 | restants : 14 | informatifs : 0
```

Remarquez `0 proposed` : `lint` ne planifie jamais de modification. Le même
projet sous [`check`](/fr/commands/check) signale dix-sept corrections
proposées, parce que `check` a le droit de les planifier.

Les diagnostics sont groupés par règle et chaque emplacement est conservé.
Chaque groupe nomme son origine (la Norminette officielle, le compilateur C,
l'analyseur natif ou une règle de projet), pour que vous sachiez avec quelle
autorité vous discutez.

## En CI

```sh
normfix lint --format json > report.json
```

Le JSON conserve les résultats individuels et porte `schema_version`. Le code de
sortie `1` signifie qu'il reste des diagnostics, `0` signifie propre et `2`
signifie que l'exécution elle-même a échoué.
