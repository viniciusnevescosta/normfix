# Qué se corrige y qué no

El formateador nativo de C trata actualmente casos probados en estas áreas:

- eliminación de BOM UTF-8, normalización de CRLF, espacios al final de línea,
  secuencias de líneas en blanco, espacios al principio del archivo y un único
  salto de línea final;
- indentación y espaciado de preprocesador, excepto formas multilínea sensibles;
- orden del bloque de includes: cabeceras de sistema antes que las del proyecto,
  en orden alfabético dentro de cada categoría;
- líneas en blanco obligatorias y prohibidas alrededor de declaraciones,
  preprocesadores y funciones;
- llaves y cuerpos de control que necesitan su propia línea física;
- disposición de control Allman, eliminación conservadora de bloques redundantes
  de una sola instrucción y una limpieza estrecha de `else` redundante cuando
  ambas ramas retornan;
- indentación con paradas de tabulación de cuatro columnas y diagnósticos
  corrientes de espacio/tabulación;
- indentación y la línea en blanco obligatoria siguiente para grupos simples de
  declaraciones locales iniciales;
- espaciado alrededor de operadores, punteros, paréntesis, palabras clave y
  declaradores de función;
- alineación de grupo para variables simples de una línea y prototipos de
  función, incluidos los declaradores de puntero cuando el grupo es inequívoco;
- la declaración separada del valor que recibió: `int teste = 10;` pasa a ser
  `int teste;` más una asignación después del bloque de declaraciones, que es lo
  que pide la regla oficial `DECL_ASSIGN_LINE`;
- borrado de una sentencia que es solo un `;`, cuando está dentro de un bloque o
  en el ámbito del archivo y no viene justo después de una directiva de
  preprocesador;
- `return value;` a `return (value);`;
- listas de parámetros vacías en definiciones de función a `(void)`;
- `return (0);` de retorno-puntero a `return (NULL);` cuando el tipo de retorno y
  un proveedor visible de `NULL` están ambos probados;
- ajuste de línea en operadores o comas probados;
- reunión codiciosa de líneas de continuación mientras el resultado se mantenga
  dentro de 80 columnas de visualización.

El empaquetado de líneas largas no cruza comentarios, directivas de
preprocesamiento, empalmes de línea ni instrucciones no relacionadas. Las cadenas
y los comentarios no se dividen. Las líneas de preprocesador no se reescriben
solo para cumplir el ancho.

### Orden de los includes

Una secuencia de directivas `#include` se reordena solo mientras **cada** línea
de ella sea exactamente una directiva de include. La primera línea que sea otra
cosa (un comentario, una línea en blanco, un condicional, una definición de macro
o texto tras el delimitador final) termina la secuencia, y las directivas de cada
lado se ordenan de forma independiente. Ninguna directiva se mueve nunca a través
de tal construcción, porque cruzarla puede cambiar declaraciones, macros de
característica o compilación condicional.

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

La ordenación es primero por categoría (`<sistema>` antes que `"proyecto"`),
luego por el nombre de la cabecera, comparado sin distinguir mayúsculas. Los
nombres iguales conservan su orden relativo original. Usa
`--no-reorder-includes` para dejar cada bloque intacto; el informe recurre
entonces a la advertencia `INCLUDE_ORDER_REVIEW`.

El formateador mide celdas de visualización de terminal: las tabulaciones usan
paradas de cuatro columnas, las marcas combinantes usan cero celdas y los
caracteres Unicode anchos usan dos.

### Pruebas obligatorias

El formateo ocurre primero solo en memoria. Para cada acción de disposición:

- la fuente debe analizarse sin regiones `ERROR`, `MISSING` ni de cinta
  desconocida;
- la cinta de tokens debe cubrir y reconstruir la entrada completa;
- la huella ordenada de tokens y comentarios debe permanecer idéntica;
- el candidato debe reanalizarse sin recuperación;
- los rangos de edición deben ser válidos y no solaparse.

Después de producir el candidato completo, Norminette se ejecuta otra vez. Si
cualquier recuento de regla aumenta respecto a la línea base validada, el lote de
formateo nativo se revierte para ese archivo. Los fallos operativos nunca
autorizan una escritura parcial.

Las acciones estrechas que cambian tokens, como `return (...)` y `(void)`, son
acciones semánticas separadas con reglas propias de construcción; no se tratan
como ediciones genéricas de espacios en blanco.

## Diagnósticos que siguen siendo manuales

El informe del terminal explica la regla, el fragmento exacto de fuente, el
origen y un siguiente paso concreto para trabajos como:

- funciones con más de 25 líneas de cuerpo;
- más de 4 parámetros, 5 variables locales o 5 funciones por archivo `.c`;
- líneas de más de 80 columnas sin una ruptura segura en operador/coma;
- estructuras de control prohibidas, ternarios, `goto`, etiquetas y asignaciones
  en condiciones;
- las declaraciones que aparecen después de una sentencia;
- identificadores públicos o globales que necesitan renombrado en todo el
  proyecto;
- movimiento de tipos/includes y cambios de estructura del proyecto;
- declaraciones ambiguas, punteros a función, atributos, campos de bits y
  declaradores multilínea;
- C malformado o recuperado por el analizador;
- guardas de cabecera que no superan la prueba cerrada del árbol de trabajo.

La capa semántica evalúa un subconjunto conservador de expresiones constantes
enteras de C, incluidas las constantes de enum. Eso permite que un límite de enum
conocido, como `count[op_total]`, se informe como un falso positivo informativo
de compatibilidad con Norminette, en vez de un array de longitud variable real.
Las expresiones no soportadas siguen siendo desconocidas; nunca se adivinan.

Para una función larga, el diagnóstico sugiere extraer una región cohesiva e
informa del presupuesto aplicable. Nunca mueve instrucciones, inventa parámetros
ni crea una función auxiliar automáticamente: el flujo de datos, los nombres, la
visibilidad y la intención del proyecto no pueden probarse solo a partir de
hechos de formato.
