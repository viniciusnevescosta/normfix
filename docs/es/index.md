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
  - title: Solo ediciones demostradas
    details: >-
      Las reglas proponen sustituciones en rangos estrechos de bytes contra
      búferes sombra inmutables. Una demostración fallida no puede modificar un
      archivo a medias, y todo lo ambiguo se informa en vez de reescribirse.
  - title: Decide el verificador oficial
    details: >-
      La Norminette oficial instalada es la autoridad. La versión 3.3.59 es la
      base verificada; otra versión sigue siendo utilizable, con una advertencia
      de compatibilidad visible.
  - title: Recuperable por construcción
    details: >-
      Las escrituras pasan por una única transacción auditable, con copias de
      seguridad externas, journal, commits ordenados, rollback y un undo que
      falla en cerrado ante cualquier destino modificado.
  - title: Privado en el navegador
    details: >-
      El playground en WebAssembly reutiliza el mismo analizador nativo y las
      mismas acciones de C dentro de tu pestaña. Sin subidas, cuenta, analítica
      ni backend.
---

## Qué es normfix

`normfix` formatea y diagnostica código C, headers, Makefiles y documentos
README de proyectos de 42. No es un reescritor genérico de C: opera bajo las
reglas de disposición física de la Norma, mantiene la
[Norminette oficial](https://github.com/42School/norminette) como autoridad de
compatibilidad, y se niega a adivinar donde la sintaxis de C por sí sola no
puede demostrar que un cambio es seguro.

## Qué no va a hacer

Cada límite de abajo es deliberado y está documentado en la
[política de compatibilidad](/es/COMPATIBILITY) y
en el [registro de arquitectura](/es/ARCHITECTURE):

- no afirma compatibilidad probada de las reglas nativas para una versión de la
  Norminette distinta de la 3.3.59; identifica esa versión antes de continuar;
- no extrae funciones largas por ti, porque elegir dónde termina una función
  cambia la estructura del programa;
- no demuestra la ausencia de fugas de memoria, y la salida del analizador
  sigue siendo informativa;
- no garantiza un resultado estricto de 80 columnas cuando no existe un corte
  seguro;
- no borra nada sin una concesión explícita de capacidad y sin almacenamiento
  externo recuperable.
