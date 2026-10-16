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
pub fn save_project(
    input: ProjectInput,
    state: State<'_, AppState>,
) -> Result<ProjectConfig, String> {
    state.repository.save(input)
}

#[tauri::command]
pub async fn delete_project(project_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if state.process_manager.is_running(&project_id).await {
        return Err("Detén el proyecto antes de eliminarlo".into());
    }
    state.repository.delete(&project_id)
}
