# Límites conocidos

Cada límite de abajo es deliberado. Leerlos es la forma más rápida de entender para qué sirve la herramienta.

- La compatibilidad exacta se prueba contra Norminette 3.3.59; otras versiones
  analizables se ejecutan con un aviso destacado, salvo que se active el modo
  estricto de versión.
- Los archivos C deben ser UTF-8 válido y no contener bytes NUL.
- La recuperación de Tree-sitter o los bytes de cinta sin clasificar desactivan
  las ediciones conscientes de la sintaxis en ese archivo.
- La pasada estricta por defecto del compilador usa un contexto de include
  inferido de forma conservadora; los defines específicos del proyecto, el modo
  de lenguaje, los archivos generados, las flags de destino, el enlazado y el
  comportamiento en ejecución siguen siendo responsabilidad del proyecto.
- El `-fanalyzer` de GCC puede sugerir posibles fugas, pero no puede probar la
  ausencia de fugas.
- El formateador no infiere la arquitectura del proyecto, contratos ocultos de
  evaluación, la intención de una API pública ni la pertenencia a un destino.
- La extracción de funciones largas se sugiere, nunca se realiza
  automáticamente.
- Un resultado estricto de 80 columnas solo se garantiza cuando existe una
  ruptura segura. Los literales largos, los comentarios, las directivas y las
  expresiones ambiguas siguen siendo advertencias.
- La transacción de fuente es recuperable y ordenada, pero un sistema de
  archivos no ofrece un único renombrado atómico que abarque varios archivos; la
  reversión es la estrategia de fallo entre archivos.

## Analizadores que preflight no ejecuta

`--analyzer` usa lo que el compilador ya trae: `-fanalyzer` en GCC, el analizador
estático de Clang en otro caso. Otras herramientas se dejan deliberadamente en
tus manos, porque cada una necesita una compilación o una ejecución que preflight
se niega a realizar:

| Herramienta | Por qué no se ejecuta |
|---|---|
| `valgrind`, `leaks` | Herramientas de tiempo de ejecución. Necesitan un binario enlazado y una carga de trabajo. El comando separado y explícito [`normfix leaks`](/es/commands/leaks) ejecuta el binario indicado; preflight nunca lo hace. |
| [AddressSanitizer](https://clang.llvm.org/docs/AddressSanitizer.html), [LeakSanitizer](https://clang.llvm.org/docs/LeakSanitizer.html), UBSan | Compilaciones instrumentadas, por el mismo motivo. `preflight` da una receta separada de compilación de depuración sin cambiar el Makefile entregado. |
| [clang-tidy](https://clang.llvm.org/extra/clang-tidy/index.html) | Necesita la base de compilación real del proyecto, las rutas de include, los defines y las flags de destino. `preflight` informa si está disponible, pero no adivina un comando. |
| `cppcheck`, `scan-build` | Instalaciones separadas con su propia configuración de proyecto; integrarlas significaría adivinar tu compilación. |

La regla detrás de las cuatro filas es la misma que detrás de todo lo demás: un
resultado que esta herramienta no puede reproducir y explicar no es un resultado
que vaya a informar.
