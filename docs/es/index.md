---
layout: home
title: normfix en español
description: Correcciones automáticas seguras y diagnósticos útiles para la Norma de 42.
hero:
  name: normfix
  text: Formatea con pruebas. Revisa lo demás.
  tagline: Una herramienta conservadora para proyectos C de 42, con la Norminette oficial como autoridad.
  actions:
    - theme: brand
      text: Primeros pasos
      link: /es/guide/getting-started
    - theme: alt
      text: Conocer el playground
      link: /es/guide/playground
---

## Qué hace normfix

`normfix` corrige automáticamente solo las transformaciones que puede demostrar
como seguras. Lo demás aparece como un diagnóstico localizado y accionable.

- procesa proyectos completos o archivos `.c`, `.h`, `Makefile` y README concretos;
- incluye la cabecera oficial cuando existe una identidad 42 válida;
- usa la [Norminette oficial](https://github.com/42school/norminette) como autoridad de compatibilidad;
- ofrece vista previa, diff, ámbitos Git, presupuesto de funciones, copias y undo en la CLI;
- ofrece un playground privado en WebAssembly, sin enviar código a un servidor.

La referencia completa aún está en inglés. Las páginas esenciales de
instalación y del playground están disponibles en español.
