# Server Manager — arquitectura V1

## Objetivo

Aplicación de escritorio para macOS, Windows y Linux que registra proyectos locales,
ejecuta varios procesos simultáneamente y conserva acceso a `stdin`, `stdout` y
`stderr` de cada uno.

La V1 usa Tauri 2: React/TypeScript presenta la interfaz y Rust es el único dueño de
los procesos. La interfaz nunca ejecuta comandos directamente.

## Estructura

```text
src/
  components/             Interfaz y formularios
  stores/                 Estado de UI y puente con comandos/eventos Tauri
  types/                  Contratos TypeScript compartidos con Rust
src-tauri/src/
  commands/               API invocable por la interfaz
  models/                 Configuración persistente y estado de ejecución
  services/
    project_repository.rs Persistencia JSON atómica
    process_manager.rs    Ciclo de vida, stdin y captura de salida
  state.rs                Estado global de la aplicación
  main.rs                 Ventana, bandeja y composición
```

## Modelo persistente

`ProjectConfig` contiene:

- `id`, `name` e `icon`.
- `workingDirectory`.
- `startCommand`.
- `stopCommand`: texto que se escribe en `stdin` al detener, por ejemplo `stop`.
- `projectUrl`: URL web opcional para React, Vite u otra aplicación local.
- `environmentVariables`.
- Fechas de creación y modificación.

La carpeta de trabajo se selecciona mediante el diálogo nativo del sistema. El campo
permanece editable para permitir pegar rutas conocidas o ajustar configuraciones.
Las acciones externas reciben solo el ID del proyecto: Rust resuelve la carpeta o URL
persistida antes de abrir Visual Studio Code o el navegador predeterminado.

Se guarda como `projects.json` dentro del directorio de configuración de la
aplicación. La escritura usa un archivo temporal y después lo reemplaza para evitar
configuraciones truncadas.

## Estado de ejecución

`ProcessManager` mantiene en memoria una sesión independiente por ID de proyecto:

```text
Project ID -> Child process + stdin + PID + startedAt + status
```

`stdout` y `stderr` se leen en tareas independientes y se publican como eventos
`process-output`. Los cambios de estado se publican como `process-state`. React se
suscribe a ambos eventos y limita el buffer visual para evitar crecimiento sin
límite.

## Flujo Start / Stop / Restart

1. `start_project` consulta la configuración persistente.
2. Valida que la carpeta exista y crea un shell en ese directorio.
3. Conecta las tres tuberías, registra la sesión y emite estado `running`.
4. Dos tareas transmiten stdout/stderr y otra observa la salida del proceso.
5. `send_input` escribe una línea en stdin.
6. `stop_project` marca `stopping` y, si existe, envía `stopCommand` por stdin.
7. Espera una salida limpia; después aplica terminación y finalmente terminación
   forzada si el proceso no responde.
8. `restart_project` espera a que Stop termine antes de ejecutar Start.

En Unix el proceso se inicia en un grupo propio para poder terminar también sus
descendientes. En Windows se crea un grupo nuevo y el fallback usa `taskkill /T`.

## Cierre de ventana y salida

- Cerrar la ventana la oculta; el backend y los servidores continúan activos.
- La bandeja permite restaurar la ventana, detener todos o salir.
- Salir (incluido Cmd/Ctrl+Q) ejecuta Stop All antes de terminar la aplicación.
- Un cierre forzado del proceso principal no ofrece garantías. Hacer que los
  servidores sobrevivan a la salida total requerirá un daemon independiente.

## Extensiones previstas

- Métricas CPU/RAM: proveedor de métricas asociado a cada PID/grupo.
- Búsqueda e historial: reemplazar el buffer de UI por un `LogStore` persistente.
- Comandos rápidos: colección persistente que reutiliza `send_input`.
- Autostart y perfiles: servicios sobre el repositorio, sin cambiar ProcessManager.
- Daemon: mover ProcessManager a un proceso separado y conservar esta API como
  cliente IPC.
