use tauri::{AppHandle, Manager, State};
use tokio::task::JoinSet;

use crate::{
    models::{ActionResult, ProcessSnapshot, ProjectConfig},
    services::{ProcessManager, ProjectRepository},
    state::AppState,
};

#[tauri::command]
pub async fn get_process_states(
    state: State<'_, AppState>,
) -> Result<Vec<ProcessSnapshot>, String> {
    let mut snapshots = Vec::new();
    for project in state.repository.list() {
        snapshots.push(state.process_manager.snapshot(&project.id).await);
    }
    Ok(snapshots)
}

#[tauri::command]
pub async fn start_project(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessSnapshot, String> {
    let project = find_project(&state.repository, &project_id)?;
    state.process_manager.start(app, project).await
}

#[tauri::command]
pub async fn stop_project(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = find_project(&state.repository, &project_id)?;
    state.process_manager.stop(app, project).await
}

#[tauri::command]
pub async fn restart_project(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessSnapshot, String> {
    let project = find_project(&state.repository, &project_id)?;
    state
        .process_manager
        .stop(app.clone(), project.clone())
        .await?;
    state.process_manager.start(app, project).await
}

#[tauri::command]
pub async fn send_input(
    project_id: String,
    input: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if input.trim().is_empty() {
        return Err("El comando no puede estar vacío".into());
    }
    state.process_manager.send_input(&project_id, &input).await
}

#[tauri::command]
pub async fn start_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ActionResult>, String> {
    Ok(start_projects(app, state.repository.clone(), state.process_manager.clone()).await)
}

#[tauri::command]
pub async fn stop_all(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ActionResult>, String> {
    Ok(stop_projects(app, state.repository.clone(), state.process_manager.clone()).await)
}

pub async fn stop_all_for_app(app: AppHandle) -> Vec<ActionResult> {
    let (repository, process_manager) = {
        let state = app.state::<AppState>();
        (state.repository.clone(), state.process_manager.clone())
    };
    stop_projects(app, repository, process_manager).await
}

async fn start_projects(
    app: AppHandle,
    repository: ProjectRepository,
    process_manager: ProcessManager,
) -> Vec<ActionResult> {
    let mut results = Vec::new();
    let mut tasks = JoinSet::new();

    for project in repository.list() {
        if process_manager.is_running(&project.id).await {
            results.push(ActionResult {
                project_id: project.id,
                ok: true,
                message: format!("{} ya estaba en ejecución", project.name),
            });
            continue;
        }

        let manager = process_manager.clone();
        let handle = app.clone();
        tasks.spawn(async move {
            let project_id = project.id.clone();
            let project_name = project.name.clone();
            let result = manager.start(handle, project).await;
            (project_id, project_name, result)
        });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((project_id, project_name, Ok(_))) => results.push(ActionResult {
                project_id,
                ok: true,
                message: format!("{project_name} iniciado"),
            }),
            Ok((project_id, project_name, Err(error))) => results.push(ActionResult {
                project_id,
                ok: false,
                message: format!("{project_name}: {error}"),
            }),
            Err(error) => results.push(ActionResult {
                project_id: "unknown".into(),
                ok: false,
                message: format!("Falló una tarea de inicio: {error}"),
            }),
        }
    }
    results
}

async fn stop_projects(
    app: AppHandle,
    repository: ProjectRepository,
    process_manager: ProcessManager,
) -> Vec<ActionResult> {
    let mut results = Vec::new();
    let mut tasks = JoinSet::new();

    for project in repository.list() {
        if !process_manager.is_running(&project.id).await {
            continue;
        }

        let manager = process_manager.clone();
        let handle = app.clone();
        tasks.spawn(async move {
            let project_id = project.id.clone();
            let project_name = project.name.clone();
            let result = manager.stop(handle, project).await;
            (project_id, project_name, result)
        });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((project_id, project_name, Ok(()))) => results.push(ActionResult {
                project_id,
                ok: true,
                message: format!("{project_name} detenido"),
            }),
            Ok((project_id, project_name, Err(error))) => results.push(ActionResult {
                project_id,
                ok: false,
                message: format!("{project_name}: {error}"),
            }),
            Err(error) => results.push(ActionResult {
                project_id: "unknown".into(),
                ok: false,
                message: format!("Falló una tarea de parada: {error}"),
            }),
        }
    }
    results
}

fn find_project(repository: &ProjectRepository, project_id: &str) -> Result<ProjectConfig, String> {
    repository
        .get(project_id)
        .ok_or_else(|| "No se encontró el proyecto".into())
}
