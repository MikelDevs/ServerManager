use std::path::Path;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::{services::ProjectRepository, state::AppState};

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
pub fn open_service_url(
    service_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let url = service_url(&state.repository, &service_id)?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| format!("No se pudo abrir la URL: {error}"))
}

fn service_url(repository: &ProjectRepository, service_id: &str) -> Result<Url, String> {
    let (_, service) = repository
        .get_service(service_id)
        .ok_or_else(|| "No se encontró el servicio".to_string())?;
    let value = service
        .project_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Configura primero la URL del servicio".to_string())?;
    let url =
        Url::parse(value.trim()).map_err(|_| "La URL configurada no es válida".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Solo se permiten URLs http o https".into());
    }
    Ok(url)
}

#[tauri::command]
pub async fn open_project_terminal(
    project_id: String,
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
    open_local_terminal(&project.working_directory).await
}

// The directory is an AppleScript argument, never interpolated into script source.
// `quoted form` then protects it from expansion by Terminal's login shell.
#[cfg(target_os = "macos")]
const LOCAL_TERMINAL_APPLESCRIPT: &str = r#"on run argv
  set projectPath to item 1 of argv
  tell application "Terminal"
    activate
    do script ("cd -- " & quoted form of projectPath)
  end tell
end run"#;

#[cfg(target_os = "macos")]
async fn open_local_terminal(directory: &str) -> Result<(), String> {
    let output = tokio::process::Command::new("/usr/bin/osascript")
        .args(["-e", LOCAL_TERMINAL_APPLESCRIPT, "--", directory])
        .output()
        .await
        .map_err(|error| format!("No se pudo abrir Terminal: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    if detail.contains("-1743") {
        return Err("Permite que Server Manager controle Terminal en Ajustes del Sistema > Privacidad y seguridad > Automatización".into());
    }
    Err(format!("No se pudo abrir Terminal: {}", detail.trim()))
}

#[cfg(target_os = "windows")]
async fn open_local_terminal(directory: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_CONSOLE: u32 = 0x00000010;
    let mut command = tokio::process::Command::new("cmd.exe");
    // Set the working directory through the process API, not a CMD expression.
    command.current_dir(directory).args(["/D", "/K"]);
    command.as_std_mut().creation_flags(CREATE_NEW_CONSOLE);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("No se pudo abrir la terminal local: {error}"))
}

#[cfg(any(target_os = "linux", test))]
const LINUX_LOCAL_TERMINALS: &[(&str, &[&str])] = &[
    ("x-terminal-emulator", &["-e"]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("xterm", &["-e"]),
];

#[cfg(any(target_os = "linux", test))]
fn linux_local_terminal_args(prefix: &[&str], directory: &str) -> Vec<String> {
    let mut args: Vec<String> = prefix.iter().map(|value| (*value).into()).collect();
    // Explicit cd also handles terminals that reuse a server process and ignore cwd.
    args.extend([
        "/bin/sh".into(),
        "-c".into(),
        "cd -- \"$1\" && exec \"${SHELL:-/bin/sh}\" -i".into(),
        "server-manager".into(),
        directory.into(),
    ]);
    args
}

#[cfg(target_os = "linux")]
async fn open_local_terminal(directory: &str) -> Result<(), String> {
    let mut launch_errors = Vec::new();
    for (program, prefix) in LINUX_LOCAL_TERMINALS {
        match tokio::process::Command::new(program)
            .args(linux_local_terminal_args(prefix, directory))
            .current_dir(directory)
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => launch_errors.push(format!("{program}: {error}")),
        }
    }
    if launch_errors.is_empty() {
        Err("No se encontró una terminal compatible. Instala GNOME Terminal, Konsole, Xfce Terminal, Kitty, Alacritty o xterm".into())
    } else {
        Err(format!(
            "No se pudo abrir la terminal local: {}",
            launch_errors.join("; ")
        ))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
async fn open_local_terminal(_directory: &str) -> Result<(), String> {
    Err("La apertura de terminales está disponible en macOS, Windows y Linux".into())
}

#[cfg(target_os = "macos")]
fn code_application() -> &'static str {
    "Visual Studio Code"
}

#[cfg(not(target_os = "macos"))]
fn code_application() -> &'static str {
    "code"
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn resolves_urls_by_service_and_rejects_unsafe_saved_urls() {
        let directory =
            std::env::temp_dir().join(format!("server-manager-url-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("projects.json");
        let service = |id: &str, url: Option<&str>| {
            json!({
                "id": id, "name": id, "kind": "app", "workingDirectory": ".",
                "startCommand": "echo ready", "stopCommand": null, "projectUrl": url
            })
        };
        let project = |id: &str, services: Vec<serde_json::Value>| {
            json!({
                "id": id, "name": id, "workingDirectory": directory.to_string_lossy(),
                "services": services, "icon": null,
                "createdAt": "2026-10-28T00:00:00Z", "updatedAt": "2026-10-28T00:00:00Z"
            })
        };
        fs::write(
            &path,
            serde_json::to_vec(&vec![
                project(
                    "rh",
                    vec![
                        service("rh-api", Some("http://localhost:3000/api")),
                        service("rh-app", Some("https://localhost:5173/app")),
                        service("unset", None),
                        service("invalid", Some("javascript:alert(1)")),
                        service("file", Some("file:///tmp/index.html")),
                    ],
                ),
                project(
                    "other",
                    vec![service("other-app", Some("https://example.com/"))],
                ),
            ])
            .unwrap(),
        )
        .unwrap();
        let repository = ProjectRepository::load(path).unwrap();
        assert_eq!(
            service_url(&repository, "rh-api").unwrap().as_str(),
            "http://localhost:3000/api"
        );
        assert_eq!(
            service_url(&repository, "rh-app").unwrap().as_str(),
            "https://localhost:5173/app"
        );
        assert_eq!(
            service_url(&repository, "other-app").unwrap().as_str(),
            "https://example.com/"
        );
        assert!(service_url(&repository, "missing")
            .unwrap_err()
            .contains("No se encontró"));
        assert!(service_url(&repository, "unset")
            .unwrap_err()
            .contains("Configura primero"));
        for id in ["invalid", "file"] {
            assert!(service_url(&repository, id)
                .unwrap_err()
                .contains("http o https"));
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn linux_passes_directory_as_one_argument_to_every_terminal() {
        let directory = "/tmp/RH's files $HOME; echo bad & `whoami`";
        for (_, prefix) in LINUX_LOCAL_TERMINALS {
            let args = linux_local_terminal_args(prefix, directory);
            assert_eq!(&args[..prefix.len()], *prefix);
            assert_eq!(args.last().unwrap(), directory);
            assert_eq!(args.len(), prefix.len() + 5);
            assert!(!args[prefix.len() + 2].contains(directory));
        }
    }

    #[cfg(unix)]
    #[test]
    fn local_shell_enters_literal_directory_without_evaluating_its_name() {
        use std::os::unix::fs::PermissionsExt;

        let base =
            std::env::temp_dir().join(format!("server-manager-terminal-test-{}", Uuid::new_v4()));
        let directory = base.join("RH ' \" $HOME $(touch INJECTED) `touch INJECTED` ; &\nline");
        fs::create_dir_all(&directory).unwrap();
        let capture_shell = base.join("capture-shell");
        fs::write(&capture_shell, "#!/bin/sh\npwd -P\n").unwrap();
        fs::set_permissions(&capture_shell, fs::Permissions::from_mode(0o700)).unwrap();
        let args = linux_local_terminal_args(&[], directory.to_str().unwrap());
        let output = std::process::Command::new(&args[0])
            .args(&args[1..])
            .env("SHELL", &capture_shell)
            .current_dir(&base)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim_end(),
            directory.canonicalize().unwrap().to_str().unwrap()
        );
        assert!(!base.join("INJECTED").exists());
        assert!(!directory.join("INJECTED").exists());
        fs::remove_dir_all(base).unwrap();
    }
}
