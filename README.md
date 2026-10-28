# Server Manager 0.4.0

Aplicación de escritorio para macOS, Windows y Linux que agrupa las APIs y apps
locales bajo un solo proyecto. Cada servicio tiene su carpeta, comandos, variables,
URL, proceso y consola independientes.

## Un proyecto, varios servicios

Por ejemplo, registra **RH** con raíz `/ruta/rh` y dos servicios:

| Servicio | Tipo | Carpeta | Inicio |
| --- | --- | --- | --- |
| API | API | `api` | `npm run dev` |
| App | App | `app` | `npm run dev` |

Ajusta los comandos a los scripts reales de cada servicio. Puedes agregar varias
APIs y apps, usar `.` para ejecutar en la raíz o indicar una ruta absoluta.

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

La primera apertura de 0.4.0 convierte `projects.json` al modelo de proyectos con
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
cd src-tauri && cargo test && cargo check
cd .. && npm run tauri build
```

Los instaladores quedan bajo `src-tauri/target/release/bundle/`. La arquitectura y
los contratos están descritos en [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

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
