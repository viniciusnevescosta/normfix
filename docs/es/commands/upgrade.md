# `normfix upgrade`

Reemplaza el binario en ejecución por la versión publicada más reciente de su
canal de actualización.

```sh
normfix upgrade          # descarga, verifica e instala
normfix upgrade --check  # solo informa
```

```console
$ normfix upgrade --check
normfix 1.1.1 is already the newest release.
```

## Qué hace, en orden

1. Selecciona el canal de actualización a partir de la versión en ejecución. Una
   compilación estable consulta el endpoint `/releases/latest` de GitHub buscando
   la versión estable más reciente. Una prepublicación sigue el feed completo de
   versiones, de modo que puede avanzar a una nueva candidata o a la estable
   final.
2. Se detiene si ya la estás ejecutando.
3. Se niega si el binario está gestionado por Homebrew, e indica el comando que
   hace lo correcto en ese caso.
4. Descarga el archivo de tu plataforma y el `SHA256SUMS` publicado.
5. **Verifica el resumen.** Una discrepancia aborta e imprime ambos valores; no
   se escribe nada.
6. Extrae en un directorio de preparación *dentro* del destino, para que el paso
   final sea un renombrado en el mismo sistema de archivos: el binario o se
   reemplaza, o queda exactamente como estaba.

Reemplazar un ejecutable en ejecución es seguro en Unix, porque el proceso en
ejecución conserva el archivo antiguo hasta que termina.

La frontera entre canales es deliberada: `upgrade` y el aviso diario de versión
nunca mueven una instalación estable a una beta o a una candidata. Optar por una
prepublicación sigue siendo una elección explícita en el momento de instalar.

## Cuándo se niega

| Situación | Qué dice |
|---|---|
| Instalado con Homebrew | Te remite a `brew upgrade viniciusnevescosta/normfix/normfix` |
| Sin permiso de escritura | Nombra la ruta y dice que revises la propiedad; nunca pide `sudo` |
| Suma de verificación distinta | Imprime ambos resúmenes y no instala nada |
| Sin `curl` ni `wget` | Dice qué herramienta falta |
| Plataforma no soportada | Sugiere compilar desde el código o usar el playground |

## El aviso de versión

Una ejecución normal imprime una línea cuando existe una versión más reciente:

```text
normfix 1.0.0 is available; this is 1.0.0-rc.1. Run `normfix upgrade`.
```

Es el único acceso a la red fuera del propio `upgrade`, así que es
deliberadamente estrecho:

- como mucho **una vez al día**, con la marca de tiempo en caché en
  `$XDG_CACHE_HOME/normfix/last-update-check`;
- solo para **salida humana interactiva**, nunca para `--format json` y nunca
  cuando stderr no es un terminal, así que los scripts y la CI no se ven
  afectados;
- **silencioso ante cualquier fallo**, porque un formateador que no alcanza la
  red no tiene nada malo;
- el intento se registra *antes* de la petición, así que una red inaccesible no
  hace que cada ejecución pague la misma consulta.

Desactívalo por completo:

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

::: tip Nada sobre tu código sale de la máquina
La comprobación pide a GitHub metadatos públicos de versión. No envía ninguna
ruta, ningún código fuente ni ningún identificador de ningún tipo.
:::
