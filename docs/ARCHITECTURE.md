# Server Manager — arquitectura 0.4.1

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

## Registro de aplicaciones en macOS

`maintenance` consulta Launch Services y selecciona únicamente registros con el
identificador exacto `com.lawcorp.servermanager`. Retira referencias a copias de la
Papelera del usuario y a ubicaciones conocidas de instaladores que ya no existen.
Las rutas cuyo estado no se puede comprobar se conservan. El modo de empaquetado
puede incluir expresamente su `.app` generado bajo
`src-tauri/target/[target]/release|debug/bundle/macos/`. La copia en ejecución y las
instalaciones válidas se conservan. No se eliminan archivos ni se reinicia el
registro global de aplicaciones.

La copia instalada en `/Applications/Server Manager.app` o en la carpeta
`Applications` del usuario inicia esta tarea en segundo plano. El modo CLI
`--cleanup-app-registrations` se resuelve antes de inicializar Tauri o leer los
proyectos y permite usar `--dry-run` y `--json` para diagnóstico. El resultado
separa candidatos, referencias retiradas, registros conservados y fallos. Las
referencias se consideran retiradas solo cuando una segunda consulta al registro
confirma que ya no aparecen.

`scripts/tauri.mjs` mantiene los argumentos y el código de salida del CLI oficial.
Después de un `build` macOS que genera bundles, busca el binario en el layout
conocido, comprueba el marcador `SERVER_MANAGER_MAINTENANCE_CLI_V1` sin ejecutarlo
y confirma el modo de mantenimiento antes de invocar la limpieza. Esto evita
abrir por accidente la interfaz de una versión antigua tras un build fallido.
La limpieza posterior se intenta también tras errores de empaquetado si existe
un binario compatible, y cualquier fallo se comunica sin ocultar el error
original. Un empaquetado exitoso con limpieza fallida devuelve error.

Este mantenimiento no garantiza que el buscador refresque su caché de inmediato
y no funciona como un gancho de desinstalación al enviar el paquete a la Papelera.
Si una copia `.app` sigue existiendo, macOS puede registrarla de nuevo al detectarla;
el resultado describe las referencias verificadas en esa ejecución y no bloquea
registros posteriores.

## Ventana y salida

Cerrar la ventana la oculta. La bandeja permite mostrarla, detener todos los
servicios o salir. La salida normal intenta detener todos los procesos antes de
cerrar la aplicación. Un cierre forzado no ofrece esas garantías; la continuidad
independiente requeriría un daemon.
