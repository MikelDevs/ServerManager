# Server Manager — arquitectura 0.4.0

## Modelo

Tauri 2 mantiene los procesos en Rust; React/TypeScript presenta proyectos,
servicios, estados y consolas. La interfaz invoca acciones por ID y no ejecuta
comandos directamente.

`ProjectConfig` contiene `id`, `name`, `icon`, `workingDirectory` (raíz absoluta),
`sshIp`, fechas y `services`. Cada `ServiceConfig` contiene un ID global único,
`name`, `kind` (`api` o `app`), `workingDirectory`, `startCommand`, `stopCommand`,
`projectUrl` y `environmentVariables`.

`icon` admite una miniatura PNG como data URL, de hasta 256 px por lado y 512 KiB
serializados. La interfaz convierte la imagen elegida y conserva su proporción;
Rust limita el tamaño y valida el formato almacenado. Los iconos de texto de las
versiones anteriores siguen siendo compatibles.

Una ruta relativa de servicio se resuelve contra la raíz del proyecto. Una ruta
absoluta permite servicios ubicados fuera de ella. La carpeta debe existir al
iniciar el servicio. La raíz es siempre el destino de Code y de la terminal local.

## Persistencia y compatibilidad

`ProjectRepository` guarda `projects.json` en el directorio de configuración de
la aplicación. Las escrituras se realizan mediante un archivo temporal y reemplazo.
Al cargar el formato anterior se conserva un respaldo antes de escribir la nueva
configuración. La migración conserva las configuraciones de ejecución y la
identidad de los servicios, y agrupa pares API/App solo cuando la raíz, el nombre
y las IP SSH permiten hacerlo sin ambigüedad.

La edición valida rutas, comandos, URLs, IP y IDs de servicios. No se permite
editar o eliminar un proyecto mientras alguno de sus servicios esté activo. Así
se evita perder la configuración necesaria para detenerlo o reasignar un proceso
en ejecución a otro servicio.

## Ejecución y eventos

`ProcessManager` mantiene una sesión por `serviceId` con proceso hijo, stdin,
PID, hora de inicio y estado. `process-output` y `process-state` incluyen ese ID;
el frontend mantiene estados y buffers de logs independientes por servicio.

- `start_service`, `stop_service`, `restart_service` y `send_input` operan por
  `serviceId`.
- `start_project`, `stop_project` y `restart_project` resuelven todos los servicios
  del `projectId` seleccionado.
- `start_all` y `stop_all` recorren todos los proyectos.
- Las operaciones de grupo devuelven un resultado por servicio, identifican fallos
  parciales y omiten los servicios ya en el estado solicitado.

Los servicios de un grupo se lanzan de forma concurrente. El arranque indica que
se creó el proceso, no que su endpoint HTTP ya esté disponible; no hay comprobación
de salud ni orden de dependencias entre APIs y apps.

Los controles de ciclo de vida se coordinan con la edición para evitar arranques,
paradas y cambios de configuración incompatibles. En Unix cada proceso tiene un
grupo propio para terminar sus descendientes. En Windows se usa un grupo de proceso
y `taskkill /T` como alternativa de parada.

## Integraciones

- `open_project_in_code(projectId)` abre la raíz en VS Code.
- `open_project_terminal(projectId)` abre una terminal local en esa raíz.
- `open_ssh_terminal(projectId)` abre SSH a la IP validada del proyecto.
- `open_service_url(serviceId)` abre la URL HTTP(S) validada del servicio.

Las rutas se pasan como argumentos y se protegen de interpretación por el shell.
Las terminales locales y sesiones SSH son externas al administrador de procesos.

## Ventana y salida

Cerrar la ventana la oculta. La bandeja permite mostrarla, detener todos los
servicios o salir. La salida normal intenta detener todos los procesos antes de
cerrar la aplicación. Un cierre forzado no ofrece esas garantías; la continuidad
independiente requeriría un daemon.
