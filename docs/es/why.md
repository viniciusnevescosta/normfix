# Qué es normfix, y por qué

## El objetivo

El recurso más escaso de un estudiante de 42 es el tiempo. No el talento, no el
esfuerzo. Horas. Y una parte significativa de esas horas se va en espacios en
blanco: arreglar indentación, mover declaraciones, partir líneas en 80 columnas,
pegar cabeceras. A lo largo de un cursus que son miles de archivos, proyecto tras
proyecto, y nada de ello te enseña nada la segunda vez que lo haces.

`normfix` existe para devolverte esas horas. Corrige, en un comando y en todo un
proyecto, los errores que son mecánicos, y se niega a tocar los que de verdad
tratan de tu programa, porque esos son los que merecen tu tiempo.

## En un párrafo

Escribes C para un proyecto de 42. La
[Norminette oficial](https://github.com/42School/norminette) te dice que la línea
47 tiene mal la indentación, que una función es demasiado larga, que una
declaración está en el sitio equivocado, y entonces se detiene, porque informar
es todo lo que hace.
`normfix` lee el mismo proyecto, corrige los errores que puede probar que es
seguro corregir, y explica el resto con palabras en vez de con un nombre de
regla. Es un comando que deja tu proyecto más cerca de aprobar de como lo
encontró, o te dice exactamente por qué no pudo.

```sh
cd ruta/a/un/proyecto-42
normfix
```

Esa es toda la interfaz. No hace falta ningún archivo de configuración, no se
sube nada a ninguna parte, y cada archivo que reescribe se respalda antes fuera
del proyecto.

## El problema

La Norma de 42 es un estándar de disposición: tabulaciones reales, 80 columnas,
una declaración por línea, una línea en blanco tras el bloque de declaraciones,
25 líneas por función, cinco funciones por archivo, una cabecera oficial en lo
alto de cada archivo. Nada de eso es difícil. Todo es tedioso, y todo lo
comprueba una herramienta que solo sabe decir *no*.

Así que el día antes de una defensa estás haciendo una de dos cosas: editar
espacios a mano en cuarenta archivos, o ejecutar un formateador genérico y
esperar. Las dos acaban mal. La primera es lenta y se te escapará algo. La
segunda es peor, porque un formateador que no conoce la Norma producirá con
confianza código que Norminette rechaza, y reescribirá tu archivo entero para
hacerlo, así que no puedes distinguir lo que cambió de lo que escribiste.

## Qué hace normfix de forma distinta

**Usa el verificador oficial como autoridad.** La Norminette instalada se ejecuta
antes y después de cada lote de ediciones. Si un lote introduce una violación de
regla que no estaba antes, el lote entero se revierte y tus bytes originales
permanecen. La versión 3.3.59 es la línea base de compatibilidad probada; una
versión instalada distinta sigue siendo utilizable, pero se nombra en un aviso
destacado porque las reglas nativas no han recibido la misma validación.
`normfix` nunca discute con la herramienta que de verdad te evalúa.

**Edita rangos estrechos de bytes, no archivos enteros.** Un cambio toca el rango
sobre el que probó algo y nada más, así que el diff es revisable y el resto de tu
archivo queda idéntico byte a byte. Por eso puedes ejecutarlo sobre trabajo en
curso.

**Rechaza más de lo que acepta.** Reordenar includes a través de un `#ifdef`
podría cambiar qué declaraciones existen, así que se detiene en el condicional.
Extraer una función de un cuerpo de 40 líneas exige nombrar la nueva función, lo
que es una decisión de diseño, así que informa de la longitud y te deja decidir.
Cada rechazo viene con el motivo y el siguiente paso.

**Todo lo que escribe es recuperable.** Las escrituras pasan por una única
transacción con copias externas y un journal. `normfix undo` restaura una
ejecución, y se niega a hacerlo si has editado esos archivos desde entonces.

## Qué no hará

Esta es la lista honesta, y es el propósito de la herramienta, no una limitación
de la versión actual:

- No extraerá una función larga por ti.
- No rediseñará el flujo de control, no renombrará en todo un proyecto ni
  cambiará una firma pública.
- No probará que tu programa no tiene fugas. La pasada del analizador puede
  sugerir una fuga; no puede probar su ausencia.
- No llamará "soportada" a una versión no probada de Norminette. Continúa con un
  aviso visible de compatibilidad para que una actualización de 42 no deje la
  herramienta inutilizable, mientras `--strict-norminette-version` restaura el
  comportamiento de fallar cerrado.
- No garantizará 80 columnas cuando no existe una ruptura segura. Una cadena
  larga o una macro sigue siendo larga y se informa.

## Dónde encaja

| Momento | Comando |
|---|---|
| Mientras escribes | `normfix --changed` sobre lo que acabas de tocar |
| Antes de confirmar | `normfix --check` como puerta; el código de salida `1` significa que queda trabajo |
| En una revisión | `normfix lint --format json` para un diagnóstico sin ediciones |
| Antes de una defensa | [`normfix preflight`](/es/commands/preflight), que añade la pasada estricta del compilador |
| Tras una mala ejecución | [`normfix undo`](/es/commands/undo) |

## La regla sobre la que está construido

> Cambia lo que puede probarse, explica lo que no, y nunca conviertas la
> incertidumbre en permiso.

Cada decisión de diseño en [la arquitectura](/es/ARCHITECTURE) se deriva de esa
frase, incluidas las que hacen que la herramienta haga menos de lo que podría.

## A continuación

- [Primeros pasos](/es/guide/getting-started): instálalo y haz una primera
  ejecución reversible.
- [Comandos](/es/commands/): una página por subcomando, con salida real.
- [Todas las flags](/es/reference/flags): qué hace cada una, con un ejemplo.
- [Playground en el navegador](/es/guide/playground): prueba el formateador sin
  instalar nada.
