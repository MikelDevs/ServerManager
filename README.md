# Server Manager

Project Launcher / Server Manager de escritorio para macOS, Windows y Linux.
Permite registrar proyectos locales, ejecutar varios a la vez, consultar sus logs en
tiempo real y enviar comandos a `stdin`.

## V1

- Alta, edición y eliminación de proyectos.
- Carpeta de trabajo, comando de inicio, parada por stdin, entorno e icono.
- Procesos simultáneos con PID y tiempo activo.
- Start, Stop, Restart, Start All y Stop All.
- Consola separada para stdout/stderr y entrada manual.
- Acciones para abrir la carpeta en Visual Studio Code y la URL web en el navegador.
- IP para SSH opcional por proyecto. El botón Abrir terminal, junto a Abrir app,
  ejecuta `ssh root@<ip>` con la IP guardada de ese proyecto.
- Persistencia local en `projects.json` dentro de la configuración de la aplicación.
- La ventana se oculta al cerrarla; los procesos siguen administrados desde la bandeja.
- “Detener y salir” detiene los procesos antes de cerrar la aplicación.

La estructura y las decisiones de diseño están en
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Requisitos de desarrollo

- Node.js 18 o posterior.
- Rust estable mediante [rustup](https://rustup.rs/).
- Dependencias de sistema de Tauri 2 para la plataforma objetivo.

Después de instalar Rust, abre una terminal nueva o carga su entorno:

```sh
. "$HOME/.cargo/env"
```

## Desarrollo

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

Los instaladores generados quedan bajo `src-tauri/target/release/bundle/`.

## Semántica de parada

Si un proyecto tiene `stopCommand`, Stop lo envía a stdin con un salto de línea y
espera hasta 10 segundos. Si el proceso no finaliza, solicita la terminación de su
grupo y después usa una terminación forzada. Sin `stopCommand`, comienza directamente
con la terminación del sistema.

En esta versión los procesos permanecen activos mientras Server Manager siga abierto
en segundo plano. Sobrevivir al cierre forzado o a la salida total requerirá mover el
motor a un daemon independiente.

## Conexión SSH

Configura la **IP para SSH** (IPv4 o IPv6) al crear o editar cada proyecto. Después
pulsa **Abrir terminal**, junto a **Abrir app**, para ejecutar `ssh root@<ip>` con
esa IP. El botón se habilita cuando el proyecto tiene una IP guardada.
La IP se valida al guardar y antes de abrir la terminal. Los proyectos anteriores
siguen disponibles con el campo vacío. Las contraseñas, claves y confirmaciones
de conexión se manejan directamente en el cliente SSH de la terminal.

Se necesita tener instalado el cliente `ssh`. En macOS se abre Terminal; en
Windows, una ventana de CMD; en Linux, una terminal instalada compatible.
En macOS, la primera apertura puede pedir permiso para que Server Manager controle
Terminal; concédelo para iniciar la sesión.
Las sesiones SSH son independientes de los procesos locales administrados por
Server Manager.
