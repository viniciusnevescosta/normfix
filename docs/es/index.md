---
layout: home
title: normfix en español
description: Correcciones automáticas seguras y diagnósticos útiles para la Norma de 42.
hero:
  name: normfix
  text: Correcciones seguras para la Norma de 42
  tagline: >-
    Un solo comando corrige los errores mecánicos de todo un proyecto de 42, y
    explica los que merecen tu tiempo. Tus horas son el recurso escaso.
  actions:
    - theme: brand
      text: Por qué normfix
      link: /es/why
    - theme: alt
      text: Primeros pasos
      link: /es/guide/getting-started
    - theme: alt
      text: Probarlo en el navegador
      link: /es/guide/playground

features:
  - title: Solo cambia lo que puede demostrar
    details: >-
      Una edición toca exactamente el trozo sobre el que demostró algo, y nada
      más. Lo que no puede demostrar lo avisa y lo deja en paz, así que puedes
      ejecutarlo a mitad del trabajo y seguir leyendo el diff.
  - title: La última palabra es de la Norminette
    details: >-
      normfix nunca discute con la herramienta que te evalúa. Ejecuta el
      verificador oficial antes y después de sus ediciones, y descarta cualquier
      lote que haya empeorado las cosas.
  - title: No se pierde nada
    details: >-
      Cada archivo que reescribe se copia antes fuera de tu proyecto. `normfix
      undo` deshace una ejecución, y se niega si has tocado esos archivos
      después.
  - title: Pruébalo sin instalar
    details: >-
      El playground funciona en la pestaña de tu navegador. No se sube nada, no
      hay cuenta, y no hay nadie mirando lo que pegas ahí.
---

## Qué es normfix

`normfix` formatea y revisa los archivos C, headers, Makefiles y READMEs de un
proyecto de 42. No sirve para reescribir C en general: trabaja dentro de las
reglas de formato de la Norma, trata la
[Norminette oficial](https://github.com/42School/norminette) como la autoridad
sobre lo que significan esas reglas, y se niega a adivinar siempre que la
sintaxis de C por sí sola no demuestra que un cambio es seguro.

## Qué no va a hacer

Cada límite de abajo es deliberado y está documentado en la
[política de compatibilidad](/es/COMPATIBILITY) y
en el [registro de arquitectura](/es/ARCHITECTURE):

- no va a llamar soportada a una versión de la Norminette que no se ha probado:
  dice cuál encontró y sigue con un aviso;
- no va a partir una función larga por ti, porque elegir dónde cortarla cambia
  cómo está montado tu programa;
- no va a demostrar que tu programa no pierde memoria; el analizador señala una
  fuga probable, nunca la ausencia de una;
- no va a forzar 80 columnas cuando no hay un sitio seguro para cortar la línea;
- no va a borrar nada sin que lo pidas, y nunca sin una copia desde la que
  puedas restaurar.
