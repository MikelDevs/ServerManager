use std::path::Path;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::state::AppState;

#[tauri::command]
pub fn open_project_in_code(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = state
        .repository
        .get(&project_id)
        .ok_or_else(|| "No se encontró el proyecto".to_string())?;
    if !Path::new(&project.working_directory).is_dir() {
        return Err(format!(
            "La carpeta del proyecto no existe: {}",
            project.working_directory
        ));
    }

    app.opener()
        .open_path(&project.working_directory, Some(code_application()))
        .map_err(|error| {
            format!("No se pudo abrir Visual Studio Code. Confirma que esté instalado: {error}")
        })
}

#[tauri::command]
pub fn open_project_url(
    project_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project = state
        .repository
        .get(&project_id)
        .ok_or_else(|| "No se encontró el proyecto".to_string())?;
    let value = project
        .project_url
        .as_deref()
        .ok_or_else(|| "Configura primero la URL del proyecto".to_string())?;
    let url = Url::parse(value).map_err(|_| "La URL configurada no es válida".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Solo se permiten URLs http o https".into());
    }

    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| format!("No se pudo abrir la URL: {error}"))
}

#[cfg(target_os = "macos")]
fn code_application() -> &'static str {
    "Visual Studio Code"
}

#[cfg(not(target_os = "macos"))]
fn code_application() -> &'static str {
    "code"
}
