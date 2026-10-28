use tauri::State;

use crate::{
    models::{ProjectConfig, ProjectInput},
    state::AppState,
};

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> Vec<ProjectConfig> {
    state.repository.list()
}

#[tauri::command]
pub async fn save_project(
    input: ProjectInput,
    state: State<'_, AppState>,
) -> Result<ProjectConfig, String> {
    let _operation = state.process_manager.operation_lock.lock().await;
    if let Some(id) = input.id.as_deref() {
        ensure_stopped(&state, id).await?;
    }
    state.repository.save(input)
}

#[tauri::command]
pub async fn delete_project(project_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let _operation = state.process_manager.operation_lock.lock().await;
    ensure_stopped(&state, &project_id).await?;
    state.repository.delete(&project_id)
}

async fn ensure_stopped(state: &AppState, project_id: &str) -> Result<(), String> {
    let project = state
        .repository
        .get(project_id)
        .ok_or_else(|| "No se encontró el proyecto".to_string())?;
    for service in &project.services {
        if state.process_manager.is_running(&service.id).await {
            return Err(
                "Detén todas las APIs y apps del proyecto antes de editarlo o eliminarlo".into(),
            );
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{
        models::{ServiceInput, ServiceKind},
        services::ProjectRepository,
    };
    use std::{collections::BTreeMap, fs};

    #[tokio::test]
    async fn editing_and_deleting_are_blocked_while_any_child_is_running() {
        let directory = std::env::temp_dir().join(format!(
            "server-manager-active-test-{}",
            uuid::Uuid::new_v4()
        ));
        let repository = ProjectRepository::load(directory.join("projects.json")).unwrap();
        let project = repository
            .save(ProjectInput {
                id: None,
                name: "RH".into(),
                working_directory: std::env::temp_dir().to_string_lossy().into_owned(),
                ssh_ip: None,
                icon: None,
                services: vec![ServiceInput {
                    id: None,
                    name: "API".into(),
                    kind: ServiceKind::Api,
                    working_directory: ".".into(),
                    start_command: "cat".into(),
                    stop_command: None,
                    project_url: None,
                    environment_variables: BTreeMap::new(),
                }],
            })
            .unwrap();
        let app = tauri::test::mock_app();
        let state = AppState::new(repository);
        let service = project.resolved_service(&project.services[0]);
        state
            .process_manager
            .start(app.handle().clone(), service.clone())
            .await
            .unwrap();
        let check = ensure_stopped(&state, &project.id).await;
        state
            .process_manager
            .stop(app.handle().clone(), service)
            .await
            .unwrap();
        assert!(check
            .unwrap_err()
            .contains("antes de editarlo o eliminarlo"));
        assert!(ensure_stopped(&state, &project.id).await.is_ok());
        fs::remove_dir_all(directory).unwrap();
    }
}
