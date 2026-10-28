use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    models::{ActionResult, ProcessSnapshot, ProjectConfig, ServiceConfig},
    services::{ProcessManager, ProjectRepository},
    state::AppState,
};

#[tauri::command]
pub async fn get_process_states(
    state: State<'_, AppState>,
) -> Result<Vec<ProcessSnapshot>, String> {
    let mut snapshots = Vec::new();
    for project in state.repository.list() {
        for service in project.services {
            snapshots.push(state.process_manager.snapshot(&service.id).await);
        }
    }
    Ok(snapshots)
}

#[tauri::command]
pub async fn start_service(
    service_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessSnapshot, String> {
    let _operation = state.process_manager.operation_lock.lock().await;
    ensure_not_quitting(&state)?;
    let service = find_service(&state.repository, &service_id)?;
    state.process_manager.start(app, service).await
}

#[tauri::command]
pub async fn stop_service(
    service_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _operation = state.process_manager.operation_lock.lock().await;
    let service = find_service(&state.repository, &service_id)?;
    if !state.process_manager.is_running(&service_id).await {
        return Ok(());
    }
    state.process_manager.stop(app, service).await
}

#[tauri::command]
pub async fn restart_service(
    service_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessSnapshot, String> {
    let _operation = state.process_manager.operation_lock.lock().await;
    ensure_not_quitting(&state)?;
    let service = find_service(&state.repository, &service_id)?;
    if state.process_manager.is_running(&service_id).await {
        state
            .process_manager
            .stop(app.clone(), service.clone())
            .await?;
    }
    state.process_manager.start(app, service).await
}

#[tauri::command]
pub async fn start_project(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ActionResult>, String> {
    run_project(project_id, app, &state, Operation::Start).await
}

#[tauri::command]
pub async fn stop_project(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ActionResult>, String> {
    run_project(project_id, app, &state, Operation::Stop).await
}

#[tauri::command]
pub async fn restart_project(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ActionResult>, String> {
    run_project(project_id, app, &state, Operation::Restart).await
}

async fn run_project(
    project_id: String,
    app: AppHandle,
    state: &AppState,
    operation: Operation,
) -> Result<Vec<ActionResult>, String> {
    let _guard = state.process_manager.operation_lock.lock().await;
    if !matches!(operation, Operation::Stop) {
        ensure_not_quitting(state)?;
    }
    let project = state
        .repository
        .get(&project_id)
        .ok_or_else(|| "No se encontró el proyecto".to_string())?;
    Ok(run_services(app, vec![project], state.process_manager.clone(), operation).await)
}

#[tauri::command]
pub async fn send_input(
    service_id: String,
    input: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if input.trim().is_empty() {
        return Err("El comando no puede estar vacío".into());
    }
    find_service(&state.repository, &service_id)?;
    state.process_manager.send_input(&service_id, &input).await
}

#[tauri::command]
pub async fn start_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ActionResult>, String> {
    let _guard = state.process_manager.operation_lock.lock().await;
    ensure_not_quitting(&state)?;
    Ok(run_services(
        app,
        state.repository.list(),
        state.process_manager.clone(),
        Operation::Start,
    )
    .await)
}

#[tauri::command]
pub async fn stop_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ActionResult>, String> {
    let _guard = state.process_manager.operation_lock.lock().await;
    Ok(run_services(
        app,
        state.repository.list(),
        state.process_manager.clone(),
        Operation::Stop,
    )
    .await)
}

pub async fn stop_all_for_app(app: AppHandle) -> Vec<ActionResult> {
    let (repository, manager) = {
        let state = app.state::<AppState>();
        (state.repository.clone(), state.process_manager.clone())
    };
    let _guard = manager.operation_lock.lock().await;
    run_services(app, repository.list(), manager.clone(), Operation::Stop).await
}

#[derive(Clone, Copy)]
enum Operation {
    Start,
    Stop,
    Restart,
}

async fn run_services<R: Runtime>(
    app: AppHandle<R>,
    projects: Vec<ProjectConfig>,
    manager: ProcessManager,
    operation: Operation,
) -> Vec<ActionResult> {
    let mut tasks = Vec::new();
    for project in projects {
        for service in &project.services {
            let service = project.resolved_service(service);
            let service_id = service.id.clone();
            let label = format!("{} / {}", project.name, service.name);
            let manager = manager.clone();
            let app = app.clone();
            let task = tokio::spawn(async move {
                let running = manager.is_running(&service.id).await;
                match operation {
                    Operation::Start if running => Ok("ya estaba en ejecución"),
                    Operation::Stop if !running => Ok("ya estaba detenido"),
                    Operation::Start => manager.start(app, service).await.map(|_| "iniciado"),
                    Operation::Stop => manager.stop(app, service).await.map(|_| "detenido"),
                    Operation::Restart => {
                        if running {
                            manager.stop(app.clone(), service.clone()).await?;
                        }
                        manager.start(app, service).await.map(|_| "reiniciado")
                    }
                }
            });
            tasks.push((service_id, label, task));
        }
    }
    let mut results = Vec::with_capacity(tasks.len());
    for (service_id, label, task) in tasks {
        let result = task
            .await
            .unwrap_or_else(|error| Err(format!("Falló la tarea: {error}")));
        results.push(match result {
            Ok(message) => ActionResult {
                service_id,
                ok: true,
                message: format!("{label}: {message}"),
            },
            Err(error) => ActionResult {
                service_id,
                ok: false,
                message: format!("{label}: {error}"),
            },
        });
    }
    results
}

fn find_service(repository: &ProjectRepository, service_id: &str) -> Result<ServiceConfig, String> {
    repository
        .get_service(service_id)
        .map(|(_, service)| service)
        .ok_or_else(|| "No se encontró la API o app".into())
}

fn ensure_not_quitting(state: &AppState) -> Result<(), String> {
    if state.quitting.load(Ordering::SeqCst) {
        Err("Server Manager se está cerrando; no se pueden iniciar servicios".into())
    } else {
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::models::{ProcessStatus, ServiceKind};
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tauri::Listener;

    fn fixture() -> ProjectConfig {
        ProjectConfig {
            id: "rh".into(),
            name: "RH".into(),
            working_directory: std::env::temp_dir().to_string_lossy().into_owned(),
            ssh_ip: None,
            icon: None,
            created_at: String::new(),
            updated_at: String::new(),
            services: vec!["api", "app"]
                .into_iter()
                .map(|id| ServiceConfig {
                    id: id.into(),
                    name: id.into(),
                    kind: if id == "api" {
                        ServiceKind::Api
                    } else {
                        ServiceKind::App
                    },
                    working_directory: ".".into(),
                    start_command: format!("/bin/sh -c 'printf ready-{id}; exec cat'"),
                    stop_command: None,
                    project_url: None,
                    environment_variables: BTreeMap::new(),
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn starts_both_services_once_routes_output_and_input_and_stops_every_service() {
        let app = tauri::test::mock_app();
        let manager = ProcessManager::default();
        let project = fixture();
        let events = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let captured = events.clone();
        app.listen("process-output", move |event| {
            captured
                .lock()
                .unwrap()
                .push(serde_json::from_str(event.payload()).unwrap());
        });
        let starts = run_services(
            app.handle().clone(),
            vec![project.clone()],
            manager.clone(),
            Operation::Start,
        )
        .await;
        let before = vec![manager.snapshot("api").await, manager.snapshot("app").await];
        let second_start = run_services(
            app.handle().clone(),
            vec![project.clone()],
            manager.clone(),
            Operation::Start,
        )
        .await;
        let after = vec![manager.snapshot("api").await, manager.snapshot("app").await];
        let sent = manager.send_input("api", "hello-api").await;
        for _ in 0..50 {
            let received = {
                let events = events.lock().unwrap();
                events.iter().any(|event| {
                    event["serviceId"] == "api"
                        && event["chunk"].as_str().unwrap().contains("hello-api")
                }) && events.iter().any(|event| {
                    event["serviceId"] == "app"
                        && event["chunk"].as_str().unwrap().contains("ready-app")
                })
            };
            if received {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let stops = run_services(
            app.handle().clone(),
            vec![project.clone()],
            manager.clone(),
            Operation::Stop,
        )
        .await;
        let stopped_again = run_services(
            app.handle().clone(),
            vec![project],
            manager.clone(),
            Operation::Stop,
        )
        .await;
        assert!(starts.iter().all(|result| result.ok));
        assert!(second_start
            .iter()
            .all(|result| result.ok && result.message.contains("ya estaba")));
        assert!(before
            .iter()
            .all(|snapshot| snapshot.status == ProcessStatus::Running && snapshot.pid.is_some()));
        assert_ne!(before[0].pid, before[1].pid);
        assert_eq!(before[0].pid, after[0].pid);
        assert_eq!(before[1].pid, after[1].pid);
        assert!(sent.is_ok());
        assert!(stops.iter().all(|result| result.ok));
        assert!(stopped_again.iter().all(|result| result.ok));
        assert!(!manager.is_running("api").await && !manager.is_running("app").await);
        let events = events.lock().unwrap();
        assert!(events.iter().any(|event| event["serviceId"] == "api"
            && event["chunk"].as_str().unwrap().contains("hello-api")));
        assert!(events.iter().any(|event| event["serviceId"] == "app"
            && event["chunk"].as_str().unwrap().contains("ready-app")));
    }

    #[tokio::test]
    async fn one_failed_service_does_not_prevent_other_services_from_starting() {
        let app = tauri::test::mock_app();
        let manager = ProcessManager::default();
        let mut project = fixture();
        project.services[0].working_directory = format!("missing-{}", uuid::Uuid::new_v4());
        let starts = run_services(
            app.handle().clone(),
            vec![project.clone()],
            manager.clone(),
            Operation::Start,
        )
        .await;
        let app_was_running = manager.is_running("app").await;
        let stops = run_services(
            app.handle().clone(),
            vec![project],
            manager.clone(),
            Operation::Stop,
        )
        .await;
        assert_eq!(starts.len(), 2);
        assert!(
            !starts[0].ok
                && starts[0].service_id == "api"
                && starts[0].message.contains("RH / api")
        );
        assert!(starts[1].ok && starts[1].service_id == "app");
        assert!(app_was_running);
        assert!(stops.iter().all(|result| result.ok));
        assert!(!manager.is_running("api").await && !manager.is_running("app").await);
    }
    #[test]
    fn refuses_new_starts_once_shutdown_begins() {
        let path = std::env::temp_dir()
            .join(format!("server-manager-shutdown-{}", uuid::Uuid::new_v4()))
            .join("projects.json");
        let state = AppState::new(ProjectRepository::load(path).unwrap());
        assert!(ensure_not_quitting(&state).is_ok());
        state.quitting.store(true, Ordering::SeqCst);
        assert!(ensure_not_quitting(&state).is_err());
    }
}
