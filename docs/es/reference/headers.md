# Cabeceras oficiales de 42

Cómo se tratan el bloque de cabecera, la identidad que hay detrás y las guardas de inclusión de cabecera.

Las cabeceras oficiales ausentes se insertan en fuentes C, cabeceras C y
Makefiles cuando hay una identidad validada disponible. La resolución de
identidad usa este orden:

1. `--email`, con comprobación opcional de consistencia mediante `--login`;
2. `NORMFIX_EMAIL`, con un login opcional del entorno o de la CLI;
3. el archivo de configuración INI persistente por usuario;
4. el `user.email` efectivo de Git, si es una dirección 42 soportada;
5. la variable de entorno `MAIL`;
6. ajustes conocidos de cabecera 42 de Vim, Neovim, VS Code, Cursor y VSCodium.

El correo es la fuente de la verdad. El login es la parte local antes de `@`; la
herramienta nunca inventa una dirección ni elige en silencio entre direcciones
guardadas ambiguas.

Cuando no se encuentra un correo válido y tanto la entrada como la salida de
error son terminales interactivos, el modo humano pregunta:

```text
No verified 42 student email was found.
Enter your 42 email (Enter, cancel, or q to skip the header):
```

Tras una respuesta válida, `normfix` guarda el correo/login canónico para
ejecuciones futuras. Enter, `cancel`, `q` o fin de entrada omiten la inserción de
la cabecera mientras el resto de correcciones seguras continúan. Las ejecuciones
JSON y no interactivas nunca preguntan. Ctrl-C cancela el propio comando,
siguiendo el comportamiento normal del terminal.

### Configuración persistente de identidad

Proporcionar un `--email` válido (con un `--login` coincidente opcional) también
actualiza esa configuración automáticamente. En Unix, el directorio de la
aplicación tiene modo `0700` y el archivo sustituido atómicamente tiene modo
`0600`. El correo es un dato corriente de configuración, no un secreto cifrado.

`NORMFIX_CONFIG` selecciona una ruta absoluta explícita. Si no, el valor por
defecto de la plataforma es:

```text
$XDG_CONFIG_HOME/normfix/config.ini                    # explicit XDG base
~/Library/Application Support/normfix/config.ini       # macOS
%APPDATA%\normfix\config.ini                          # Windows
~/.config/normfix/config.ini                           # other Unix
```

El formato soportado es:

```ini
[header]
login = your_login
email = your_login@student.42.fr
```

También se admite configuración por entorno:

```sh
export NORMFIX_LOGIN='your_login'
export NORMFIX_EMAIL='your_login@student.42.fr'
```

Se captura una única marca de tiempo para la ejecución completa.
`SOURCE_DATE_EPOCH` puede proporcionar una marca UTC reproducible; un valor no
válido detiene la ejecución en lugar de usar en silencio el reloj del sistema.

Las cabeceras válidas existentes conservan los campos `By` y `Created`. El nombre
del archivo y la línea `Updated` solo cambian cuando el archivo tiene otra
edición aceptada o cuando el nombre de archivo de su cabecera está obsoleto, lo
que hace idempotente una segunda ejecución limpia.

### Guardas de cabecera

Para cabeceras corrientes, `normfix` puede insertar una guarda ausente derivada
del nombre del archivo, reparar un par `#ifndef`/`#define` que no coincide o
renombrar una guarda simple equivocada. Cada operación exige una prueba cerrada
del árbol de trabajo de Git. La prueba también escanea los archivos ignorados,
verifica que la macro esperada no se usa, rechaza guardas duplicadas derivadas
del nombre del archivo y definiciones dinámicas de compilación, y liga la
aprobación a los hashes del proyecto completo y de la cabecera.

La inserción se rechaza ante preprocesamiento condicional, `#pragma once`,
`#undef` o colisión con otra macro. Un renombrado se rechaza cuando los nombres
antiguos tienen usos más allá del par canónico de todo el archivo. Las cabeceras
complejas, referenciadas, de inclusión repetida, fuera de Git o ambiguas quedan
sin cambios y reciben una advertencia accionable.
