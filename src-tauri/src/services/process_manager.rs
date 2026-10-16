use std::{collections::HashMap, path::Path, process::Stdio, sync::Arc, time::Duration};

use chrono::Utc;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command},
    sync::Mutex,
    time::{sleep, Instant},
};

use crate::models::{
    OutputStream, ProcessOutputEvent, ProcessSnapshot, ProcessStateEvent, ProcessStatus,
    ProjectConfig,
};

const GRACEFUL_INPUT_TIMEOUT: Duration = Duration::from_secs(10);
const TERMINATE_TIMEOUT: Duration = Duration::from_secs(3);
const FORCE_TIMEOUT: Duration = Duration::from_secs(2);

struct ManagedProcess {
    child: Child,
    stdin: ChildStdin,
    pid: u32,
    started_at: String,
    status: ProcessStatus,
}

type SharedProcess = Arc<Mutex<ManagedProcess>>;

#[derive(Clone, Default)]
pub struct ProcessManager {
    processes: Arc<Mutex<HashMap<String, SharedProcess>>>,
}

impl ProcessManager {
    pub async fn start(
        &self,
        app: AppHandle,
        project: ProjectConfig,
    ) -> Result<ProcessSnapshot, String> {
        let working_directory = Path::new(&project.working_directory);
        if !working_directory.is_dir() {
            return Err(format!(
                "La carpeta de trabajo no existe o no es accesible: {}",
                project.working_directory
            ));
        }

        let mut processes = self.processes.lock().await;
        if processes.contains_key(&project.id) {
            return Err(format!("{} ya está en ejecución", project.name));
        }

        let mut command = platform_command(&project)?;
        command
            .current_dir(working_directory)
            .envs(&project.environment_variables)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| format!("No se pudo iniciar {}: {error}", project.name))?;
        let pid = child
            .id()
            .ok_or_else(|| "El sistema no devolvió un PID para el proceso".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "No se pudo conectar stdin".to_string())?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let started_at = Utc::now().to_rfc3339();
        let session = Arc::new(Mutex::new(ManagedProcess {
            child,
            stdin,
            pid,
            started_at: started_at.clone(),
            status: ProcessStatus::Running,
        }));

        processes.insert(project.id.clone(), session.clone());
        drop(processes);

        if let Some(stdout) = stdout {
            spawn_output_reader(
                app.clone(),
                project.id.clone(),
                OutputStream::Stdout,
                stdout,
            );
        }
        if let Some(stderr) = stderr {
            spawn_output_reader(
                app.clone(),
                project.id.clone(),
                OutputStream::Stderr,
                stderr,
            );
        }
        self.spawn_exit_watcher(app.clone(), project.id.clone(), session);

        let snapshot = ProcessSnapshot {
            project_id: project.id.clone(),
            status: ProcessStatus::Running,
            pid: Some(pid),
            started_at: Some(started_at.clone()),
        };
        emit_state(
            &app,
            ProcessStateEvent {
                project_id: project.id,
                status: ProcessStatus::Running,
                pid: Some(pid),
                started_at: Some(started_at),
                exit_code: None,
                message: Some(format!("Proceso iniciado con PID {pid}")),
            },
        );
        Ok(snapshot)
    }

    pub async fn stop(&self, app: AppHandle, project: ProjectConfig) -> Result<(), String> {
        let session = self
            .get_session(&project.id)
            .await
            .ok_or_else(|| format!("{} no está en ejecución", project.name))?;

        let (pid, started_at, already_stopping, input_error) = {
            let mut process = session.lock().await;
            let already_stopping = process.status == ProcessStatus::Stopping;
            process.status = ProcessStatus::Stopping;

            let input_error = if !already_stopping {
                if let Some(stop_command) = project.stop_command.as_deref() {
                    let line = line_for_stdin(stop_command);
                    match process.stdin.write_all(line.as_bytes()).await {
                        Ok(()) => process
                            .stdin
                            .flush()
                            .await
                            .err()
                            .map(|error| error.to_string()),
                        Err(error) => Some(error.to_string()),
                    }
                } else {
                    None
                }
            } else {
                None
            };
            (
                process.pid,
                process.started_at.clone(),
                already_stopping,
                input_error,
            )
        };
        let stop_input_failed = input_error.is_some();

        emit_state(
            &app,
            ProcessStateEvent {
                project_id: project.id.clone(),
                status: ProcessStatus::Stopping,
                pid: Some(pid),
                started_at: Some(started_at),
                exit_code: None,
                message: if already_stopping {
                    None
                } else if let Some(error) = input_error.as_deref() {
                    Some(format!(
                        "No se pudo enviar el comando de parada ({error}); se intentará terminar el proceso"
                    ))
                } else if let Some(command) = project.stop_command.as_deref() {
                    Some(format!("Comando de parada enviado a stdin: {command}"))
                } else {
                    Some("Solicitando terminación del proceso".into())
                },
            },
        );

        if already_stopping {
            if self
                .wait_until_stopped(&project.id, GRACEFUL_INPUT_TIMEOUT + TERMINATE_TIMEOUT)
                .await
            {
                return Ok(());
            }
        } else if project.stop_command.is_some()
            && !stop_input_failed
            && self
                .wait_until_stopped(&project.id, GRACEFUL_INPUT_TIMEOUT)
                .await
        {
            return Ok(());
        }

        terminate_process_tree(pid, false).await?;
        if self
            .wait_until_stopped(&project.id, TERMINATE_TIMEOUT)
            .await
        {
            return Ok(());
        }

        emit_output(
            &app,
            &project.id,
            OutputStream::System,
            "El proceso no respondió; aplicando terminación forzada\n".into(),
        );
        terminate_process_tree(pid, true).await?;
        if self.wait_until_stopped(&project.id, FORCE_TIMEOUT).await {
            Ok(())
        } else {
            Err(format!(
                "No fue posible confirmar la terminación de {} (PID {pid})",
                project.name
            ))
        }
    }

    pub async fn send_input(&self, project_id: &str, input: &str) -> Result<(), String> {
        let session = self
            .get_session(project_id)
            .await
            .ok_or_else(|| "El proyecto no está en ejecución".to_string())?;
        let mut process = session.lock().await;
        if process.status != ProcessStatus::Running {
            return Err("El proceso se está deteniendo y ya no acepta comandos".into());
        }

        process
            .stdin
            .write_all(line_for_stdin(input).as_bytes())
            .await
            .map_err(|error| format!("No se pudo escribir en stdin: {error}"))?;
        process
            .stdin
            .flush()
            .await
            .map_err(|error| format!("No se pudo vaciar stdin: {error}"))
    }

    pub async fn is_running(&self, project_id: &str) -> bool {
        self.processes.lock().await.contains_key(project_id)
    }

    pub async fn snapshot(&self, project_id: &str) -> ProcessSnapshot {
        let session = self.get_session(project_id).await;
        match session {
            Some(session) => {
                let process = session.lock().await;
                ProcessSnapshot {
                    project_id: project_id.to_owned(),
                    status: process.status,
                    pid: Some(process.pid),
                    started_at: Some(process.started_at.clone()),
                }
            }
            None => stopped_snapshot(project_id),
        }
    }

    fn spawn_exit_watcher(&self, app: AppHandle, project_id: String, session: SharedProcess) {
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            let (exit_code, error_message) = loop {
                sleep(Duration::from_millis(200)).await;
                let result = {
                    let mut process = session.lock().await;
                    process.child.try_wait()
                };
                match result {
                    Ok(Some(status)) => break (status.code(), None),
                    Ok(None) => continue,
                    Err(error) => {
                        break (None, Some(format!("Error observando el proceso: {error}")))
                    }
                }
            };

            let mut processes = manager.processes.lock().await;
            let should_remove = processes
                .get(&project_id)
                .is_some_and(|current| Arc::ptr_eq(current, &session));
            if should_remove {
                processes.remove(&project_id);
            }
            drop(processes);

            let message = error_message.unwrap_or_else(|| match exit_code {
                Some(code) => format!("Proceso finalizado con código {code}"),
                None => "Proceso finalizado por una señal del sistema".into(),
            });
            emit_state(
                &app,
                ProcessStateEvent {
                    project_id,
                    status: ProcessStatus::Stopped,
                    pid: None,
                    started_at: None,
                    exit_code,
                    message: Some(message),
                },
            );
        });
    }

    async fn get_session(&self, project_id: &str) -> Option<SharedProcess> {
        self.processes.lock().await.get(project_id).cloned()
    }

    async fn wait_until_stopped(&self, project_id: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !self.is_running(project_id).await {
                return true;
            }
            sleep(Duration::from_millis(100)).await;
        }
        !self.is_running(project_id).await
    }
}

fn stopped_snapshot(project_id: &str) -> ProcessSnapshot {
    ProcessSnapshot {
        project_id: project_id.to_owned(),
        status: ProcessStatus::Stopped,
        pid: None,
        started_at: None,
    }
}

fn line_for_stdin(input: &str) -> String {
    if input.ends_with('\n') {
        input.to_owned()
    } else {
        format!("{input}\n")
    }
}

fn spawn_output_reader<R>(app: AppHandle, project_id: String, stream: OutputStream, mut reader: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let mut buffer = vec![0_u8; 4096];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => break,
                Ok(read) => emit_output(
                    &app,
                    &project_id,
                    stream,
                    String::from_utf8_lossy(&buffer[..read]).into_owned(),
                ),
                Err(error) => {
                    emit_output(
                        &app,
                        &project_id,
                        OutputStream::System,
                        format!("Error leyendo la salida: {error}\n"),
                    );
                    break;
                }
            }
        }
    });
}

fn emit_output(app: &AppHandle, project_id: &str, stream: OutputStream, chunk: String) {
    let _ = app.emit(
        "process-output",
        ProcessOutputEvent {
            project_id: project_id.to_owned(),
            stream,
            chunk,
            timestamp: Utc::now().to_rfc3339(),
        },
    );
}

fn emit_state(app: &AppHandle, event: ProcessStateEvent) {
    let _ = app.emit("process-state", event);
}

#[cfg(unix)]
fn platform_command(project: &ProjectConfig) -> Result<Command, String> {
    let mut command = Command::new("/bin/sh");
    command.args(["-lc", &format!("exec {}", project.start_command)]);
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(command)
}

#[cfg(windows)]
fn platform_command(project: &ProjectConfig) -> Result<Command, String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoLogo", "-NoProfile", "-Command", &project.start_command])
        .creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    Ok(command)
}

#[cfg(unix)]
async fn terminate_process_tree(pid: u32, force: bool) -> Result<(), String> {
    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    let result = unsafe { libc::kill(-(pid as i32), signal) };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(format!(
            "No se pudo enviar la señal al proceso {pid}: {error}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::line_for_stdin;

    #[test]
    fn stdin_commands_always_end_in_one_line_break() {
        assert_eq!(line_for_stdin("stop"), "stop\n");
        assert_eq!(line_for_stdin("status\n"), "status\n");
    }
}

#[cfg(windows)]
async fn terminate_process_tree(pid: u32, force: bool) -> Result<(), String> {
    let mut command = Command::new("taskkill.exe");
    command.args(["/PID", &pid.to_string(), "/T"]);
    if force {
        command.arg("/F");
    }
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| format!("No se pudo ejecutar taskkill para {pid}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("taskkill no pudo terminar el proceso {pid}"))
    }
}
