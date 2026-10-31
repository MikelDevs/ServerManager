# Server Manager 0.4.1

Aplicación de escritorio para macOS, Windows y Linux que agrupa las APIs y apps
locales bajo un solo proyecto. Cada servicio tiene su carpeta, comandos, variables,
URL, proceso y consola independientes.

## Novedad en 0.4.1: referencias antiguas en macOS

Al abrir la copia instalada en `/Applications` o `~/Applications`, la aplicación
retira en segundo plano las referencias antiguas de Server Manager registradas
por macOS: copias de la Papelera e instaladores conocidos que ya no están montados.
El empaquetado también retira el registro de la copia temporal que acaba de generar.

La limpieza se limita al identificador `com.lawcorp.servermanager` y a esas rutas
conocidas. Conserva la aplicación en ejecución, las instalaciones válidas y las
rutas cuya condición no se puede comprobar. Verifica en una segunda consulta qué
referencias se retiraron y comunica cualquier fallo; no
borra archivos, proyectos o configuración ni detiene servicios. El paquete incluye
además su icono de aplicación explícitamente.

La búsqueda de macOS puede tardar en actualizar lo que muestra después de retirar
una referencia. Mientras una copia `.app` siga existiendo, incluso en la Papelera,
macOS puede registrarla de nuevo al detectarla. La limpieza retira las referencias
presentes; no bloquea registros futuros. Esta mejora no ejecuta una desinstalación
al mover la app a la Papelera: actúa al abrir una instalación válida y al generar
el instalador.

## Un proyecto, varios servicios

Por ejemplo, registra **RH** con raíz `/ruta/rh` y dos servicios:

| Servicio | Tipo | Carpeta | Inicio |
| --- | --- | --- | --- |
| API | API | `api` | `npm run dev` |
| App | App | `app` | `npm run dev` |

Ajusta los comandos a los scripts reales de cada servicio. Puedes agregar varias
APIs y apps, usar `.` para ejecutar en la raíz o indicar una ruta absoluta.

Al crear o editar un proyecto puedes elegir una imagen PNG, JPG o WebP de hasta
5 MB. Se guarda una miniatura dentro de la configuración y se muestra en la lista
y el encabezado; no depende de conservar el archivo original en su ubicación.

- **Iniciar todo**, **Detener todo** y **Reiniciar todo** actúan sobre los servicios
  del proyecto seleccionado. Un fallo se informa por servicio y no impide intentar
  la operación en los demás. Iniciar todo conserva los servicios que ya están activos.
- Cada servicio también tiene controles individuales, PID, tiempo activo, logs
  `stdout`/`stderr` y entrada manual a `stdin`.
- **Abrir Code** abre la carpeta raíz del proyecto en Visual Studio Code.
- **Abrir terminal** abre una terminal local en la carpeta raíz.
- **Abrir servidor** ejecuta `ssh root@<ip>` con la IP guardada en el proyecto.
- **Abrir app** abre la URL del servicio seleccionado en el navegador.
- Las acciones globales de la barra lateral controlan todos los proyectos.
- Detén todos los servicios de un proyecto antes de editarlo o eliminarlo.

La ventana se oculta al cerrarla; los procesos siguen administrados desde la
bandeja. **Detener y salir** detiene los servicios antes de cerrar la aplicación.

## Actualización desde 0.3.1

La primera apertura de 0.4.x convierte `projects.json` al modelo de proyectos con
servicios y guarda una copia de la configuración anterior junto al archivo original.
Se conservan los IDs de los servicios, comandos, variables de entorno, URLs e IP SSH.

La conversión agrupa registros como **API RH** y **APP RH** cuando tienen el mismo
nombre de proyecto, sus carpetas de servicio comparten una raíz y sus IP SSH son
compatibles. También reconoce nombres como **CORE APP** y carpetas terminadas en
`-api` o `-app`. Los registros que no se pueden agrupar con seguridad permanecen
como proyectos independientes con un servicio. La migración no modifica las
carpetas ni el código de los proyectos.

## Desarrollo

Requiere Node.js 18 o posterior, Rust estable mediante [rustup](https://rustup.rs/)
y las dependencias de sistema de Tauri 2 para la plataforma objetivo.

```sh
npm install
npm run tauri dev
```

## Validación y compilación

```sh
npm run build
npm run test:packaging
cd src-tauri && cargo test && cargo check
cd .. && npm run tauri build
```

Los instaladores quedan bajo `src-tauri/target/release/bundle/`. La arquitectura y
los contratos están descritos en [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

`npm run tauri build` usa el CLI oficial de Tauri a través de un wrapper local.
En macOS, al terminar el empaquetado ejecuta el mantenimiento desde el binario
compilado. También lo intenta si el empaquetado falla o se interrumpe, siempre que
exista un binario con el modo de mantenimiento de 0.4.1 o posterior. Nunca ejecuta
un binario anterior para intentar limpiar: esas versiones abrirían la interfaz al
recibir argumentos desconocidos.

Si Tauri falla, se conserva su código de salida. Si Tauri termina correctamente
pero el mantenimiento falla, el comando informa que la limpieza está pendiente
y devuelve error. Los comandos `dev`, ayuda, `--no-bundle` y las compilaciones en
otros sistemas no ejecutan este mantenimiento. Se admiten los layouts estándar
`src-tauri/target/[target]/release` y `debug` para los targets macOS; un
`CARGO_TARGET_DIR` personalizado produce un aviso de limpieza no soportada y
evita ejecutar un binario de una ruta supuesta.

Para revisar qué referencias se limpiarían, sin cambiarlas:

```sh
./src-tauri/target/release/server-manager --cleanup-app-registrations --dry-run --json
```

## Parada de servicios

Si un servicio tiene `stopCommand`, detenerlo envía esa línea a stdin y espera
hasta 10 segundos. Si no finaliza, solicita la terminación de su grupo y después
usa terminación forzada. Sin `stopCommand`, comienza con la terminación del sistema.
Los procesos permanecen activos mientras Server Manager siga abierto en segundo
plano; un cierre forzado de la aplicación no garantiza su continuidad ni su parada.

## Terminal local y SSH

En macOS se abre Terminal; en Windows, una ventana de CMD; en Linux, una terminal
instalada compatible. En macOS, la primera apertura puede pedir permiso para que
Server Manager controle Terminal.

La IP SSH es opcional y debe ser IPv4 o IPv6, sin usuario, puerto ni comandos.
**Abrir servidor** requiere tener `ssh` instalado. Contraseñas, claves y
confirmaciones se manejan directamente en el cliente SSH. Las sesiones externas
son independientes de los procesos locales administrados por Server Manager.
