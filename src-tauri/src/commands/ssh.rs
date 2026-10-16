use std::net::IpAddr;

use tauri::State;

use crate::{services::ProjectRepository, state::AppState};

#[tauri::command]
pub async fn open_ssh_terminal(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let address = project_address(&state.repository, &project_id)?;
    open_terminal(address).await
}

fn project_address(repository: &ProjectRepository, project_id: &str) -> Result<IpAddr, String> {
    let project = repository
        .get(project_id)
        .ok_or_else(|| "No se encontró el proyecto para abrir su terminal SSH".to_string())?;
    let ip = project
        .ssh_ip
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Agrega una IP SSH en la configuración del proyecto".to_string())?;
    parse_ip(ip).map_err(|_| {
        "La IP SSH guardada del proyecto no es válida. Edítala e ingresa una dirección IPv4 o IPv6 sin puertos ni comandos".to_string()
    })
}

fn parse_ip(value: &str) -> Result<IpAddr, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Ingresa una dirección IP para abrir la conexión SSH".into());
    }

    value
        .parse()
        .map_err(|_| "Ingresa una dirección IPv4 o IPv6 válida, sin puertos ni comandos".into())
}

fn ssh_destination(address: IpAddr) -> String {
    format!("root@{address}")
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
fn ssh_command(address: IpAddr) -> String {
    format!("ssh {}", ssh_destination(address))
}

#[cfg(any(target_os = "macos", test))]
fn terminal_applescript(address: IpAddr) -> String {
    let command = ssh_command(address)
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("tell application \"Terminal\"\n  activate\n  do script \"{command}\"\nend tell")
}

#[cfg(target_os = "macos")]
async fn open_terminal(address: IpAddr) -> Result<(), String> {
    let output = tokio::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(terminal_applescript(address))
        .output()
        .await
        .map_err(|error| format!("No se pudo abrir Terminal para la conexión SSH: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let detail = String::from_utf8_lossy(&output.stderr);
    if detail.contains("-1743") {
        return Err("Permite que Server Manager controle Terminal en Ajustes del Sistema > Privacidad y seguridad > Automatización".into());
    }

    Err(format!(
        "No se pudo abrir Terminal para la conexión SSH: {}",
        detail.trim()
    ))
}

#[cfg(any(target_os = "windows", test))]
fn windows_terminal_args(address: IpAddr) -> [String; 3] {
    ["/D".into(), "/K".into(), ssh_command(address)]
}

#[cfg(target_os = "windows")]
async fn open_terminal(address: IpAddr) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_CONSOLE: u32 = 0x00000010;
    let mut command = tokio::process::Command::new("cmd.exe");
    command.args(windows_terminal_args(address));
    command.as_std_mut().creation_flags(CREATE_NEW_CONSOLE);
    // Dropping the handle leaves the interactive console independent of Server Manager.
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("No se pudo abrir la consola para la conexión SSH: {error}"))
}

#[cfg(any(target_os = "linux", test))]
const LINUX_TERMINALS: &[(&str, &[&str])] = &[
    ("x-terminal-emulator", &["-e"]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("xterm", &["-e"]),
];

#[cfg(any(target_os = "linux", test))]
fn linux_terminal_args(prefix: &[&str], address: IpAddr) -> Vec<String> {
    let mut args: Vec<String> = prefix.iter().map(|value| (*value).into()).collect();
    args.push("ssh".into());
    args.push(ssh_destination(address));
    args
}

#[cfg(target_os = "linux")]
async fn open_terminal(address: IpAddr) -> Result<(), String> {
    let mut launch_errors = Vec::new();
    for (program, prefix) in LINUX_TERMINALS {
        // The terminal owns its PTY and SSH process; ProcessManager never tracks this session.
        match tokio::process::Command::new(program)
            .args(linux_terminal_args(prefix, address))
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
            "No se pudo abrir una terminal para la conexión SSH: {}",
            launch_errors.join("; ")
        ))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
async fn open_terminal(_address: IpAddr) -> Result<(), String> {
    Err("La apertura de terminales SSH está disponible en macOS, Windows y Linux".into())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn resolves_the_selected_project_and_validates_its_saved_ip() {
        let directory =
            std::env::temp_dir().join(format!("server-manager-ssh-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join("projects.json");
        let project = |id: &str, ssh_ip: Option<&str>| {
            json!({
                "id": id,
                "name": id,
                "workingDirectory": "/tmp",
                "startCommand": "echo ready",
                "stopCommand": null,
                "sshIp": ssh_ip,
                "icon": null,
                "createdAt": "2026-10-12T00:00:00Z",
                "updatedAt": "2026-10-12T00:00:00Z"
            })
        };
        let projects = [
            project("one", Some("192.168.1.20")),
            project("two", Some(" 2001:0DB8:0000:0000:0000:0000:0000:0002 ")),
            project("unset", None),
            project("blank", Some(" \t ")),
            project("invalid", Some("127.0.0.1; whoami")),
        ];
        fs::write(&path, serde_json::to_string(&projects).unwrap()).unwrap();
        let repository = ProjectRepository::load(path).unwrap();

        assert_eq!(
            ssh_command(project_address(&repository, "two").unwrap()),
            "ssh root@2001:db8::2"
        );
        assert_eq!(
            ssh_command(project_address(&repository, "one").unwrap()),
            "ssh root@192.168.1.20"
        );
        for id in ["unset", "blank"] {
            assert!(project_address(&repository, id)
                .unwrap_err()
                .contains("Agrega una IP SSH"));
        }
        assert!(project_address(&repository, "missing")
            .unwrap_err()
            .contains("No se encontró el proyecto"));
        assert!(project_address(&repository, "invalid")
            .unwrap_err()
            .contains("IP SSH guardada del proyecto no es válida"));
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn trims_and_normalizes_valid_addresses() {
        assert_eq!(
            ssh_command(parse_ip(" 192.168.1.20 \n").unwrap()),
            "ssh root@192.168.1.20"
        );
        assert_eq!(
            ssh_command(parse_ip(" 2001:0DB8:0000:0000:0000:0000:0000:0001 ").unwrap()),
            "ssh root@2001:db8::1"
        );
        assert_eq!(ssh_command(parse_ip("::1").unwrap()), "ssh root@::1");
    }

    #[test]
    fn rejects_invalid_addresses_and_command_injection() {
        for value in [
            "",
            "   ",
            "example.com",
            "256.1.1.1",
            "192.168.1.20:22",
            "[::1]:22",
            "fe80::1%en0",
            "root@192.168.1.20",
            "127.0.0.1; touch /tmp/injected",
            "127.0.0.1 && whoami",
            "127.0.0.1\nwhoami",
            "127.0.0.1$(whoami)",
            "127.0.0.1`whoami`",
            "127.0.0.1\"",
            "127.0.0.1\\",
            "-oProxyCommand=whoami",
        ] {
            assert!(parse_ip(value).is_err(), "accepted unsafe input: {value}");
        }
    }

    #[test]
    fn builds_a_single_applescript_command_from_a_normalized_address() {
        assert_eq!(
            terminal_applescript(parse_ip("2001:0db8::1").unwrap()),
            "tell application \"Terminal\"\n  activate\n  do script \"ssh root@2001:db8::1\"\nend tell"
        );
    }

    #[test]
    fn windows_keeps_the_console_open_and_disables_autorun() {
        assert_eq!(
            windows_terminal_args(parse_ip("127.0.0.1").unwrap()),
            ["/D", "/K", "ssh root@127.0.0.1"]
        );
    }

    #[test]
    fn linux_passes_ssh_and_the_destination_as_separate_arguments() {
        let address = parse_ip("2001:db8::1").unwrap();
        for (_, prefix) in LINUX_TERMINALS {
            let args = linux_terminal_args(prefix, address);
            assert_eq!(&args[..prefix.len()], *prefix);
            assert_eq!(&args[prefix.len()..], ["ssh", "root@2001:db8::1"]);
        }
    }
}
