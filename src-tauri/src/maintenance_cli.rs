use std::{ffi::OsString, path::PathBuf};

use crate::maintenance::{cleanup_app_registrations, CleanupOptions};

// The release wrapper checks this marker without running an older GUI binary.
const MAINTENANCE_VERSION: &str = "SERVER_MANAGER_MAINTENANCE_CLI_V1";

pub fn run() -> Option<i32> {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && arguments[0] == "--maintenance-version" {
        println!("{MAINTENANCE_VERSION}");
        return Some(0);
    }
    if !arguments
        .iter()
        .any(|value| value == "--cleanup-app-registrations")
    {
        return None;
    }
    let (options, json) = match parse_options(&arguments) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return Some(2);
        }
    };
    match cleanup_app_registrations(options) {
        Ok(report) => {
            let code = if report.failures.is_empty() { 0 } else { 1 };
            if json {
                match serde_json::to_string(&report) {
                    Ok(value) => println!("{value}"),
                    Err(error) => {
                        eprintln!("No se pudo generar el informe: {error}");
                        return Some(1);
                    }
                }
            } else if !report.supported {
                println!(
                    "El mantenimiento del registro de aplicaciones solo se necesita en macOS."
                );
            } else {
                println!(
                    "Referencias candidatas: {}; retiradas: {}; conservadas: {}; errores: {}{}",
                    report.candidates.len(),
                    report.removed.len(),
                    report.preserved,
                    report.failures.len(),
                    if report.dry_run {
                        " (solo diagnóstico)"
                    } else {
                        ""
                    }
                );
                for failure in &report.failures {
                    eprintln!("{}: {}", failure.path, failure.message);
                }
            }
            Some(code)
        }
        Err(error) => {
            eprintln!("No se pudo actualizar el registro de Server Manager: {error}");
            Some(1)
        }
    }
}

fn parse_options(arguments: &[OsString]) -> Result<(CleanupOptions, bool), String> {
    let mut options = CleanupOptions {
        dry_run: false,
        build_bundle_path: None,
    };
    let mut json = false;
    let mut command_seen = false;
    let mut args = arguments.iter();
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--cleanup-app-registrations") if !command_seen => command_seen = true,
            Some("--dry-run") => options.dry_run = true,
            Some("--json") => json = true,
            Some("--build-bundle-path") if options.build_bundle_path.is_none() => {
                let value = args
                    .next()
                    .ok_or("Falta la ruta del paquete de compilación")?;
                if value.is_empty() || value.to_string_lossy().starts_with("--") {
                    return Err("Falta la ruta del paquete de compilación".into());
                }
                options.build_bundle_path = Some(PathBuf::from(value));
            }
            _ => {
                return Err(format!(
                    "Opción de mantenimiento no válida: {}",
                    argument.to_string_lossy()
                ))
            }
        }
    }
    if !command_seen {
        return Err("Falta --cleanup-app-registrations".into());
    }
    Ok((options, json))
}

#[cfg(target_os = "macos")]
pub fn is_installed_app() -> bool {
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let Some(bundle) = executable.ancestors().find(|path| {
        path.file_name()
            .is_some_and(|name| name == "Server Manager.app")
    }) else {
        return false;
    };
    if bundle == std::path::Path::new("/Applications/Server Manager.app") {
        return true;
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .is_some_and(|home| bundle == home.join("Applications/Server Manager.app"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn preserves_paths_as_one_argument_and_supports_diagnostic_mode() {
        let (options, json) = parse_options(&args(&[
            "--cleanup-app-registrations",
            "--dry-run",
            "--json",
            "--build-bundle-path",
            "/tmp/project with spaces/target/release/bundle/macos/Server Manager.app",
        ]))
        .unwrap();
        assert!(options.dry_run && json);
        assert_eq!(
            options.build_bundle_path.unwrap(),
            PathBuf::from(
                "/tmp/project with spaces/target/release/bundle/macos/Server Manager.app"
            )
        );
    }

    #[test]
    fn refuses_unknown_or_incomplete_maintenance_commands() {
        for values in [
            vec!["--cleanup-app-registrations", "--all"],
            vec!["--cleanup-app-registrations", "--build-bundle-path"],
            vec![
                "--cleanup-app-registrations",
                "--build-bundle-path",
                "--json",
            ],
            vec!["--cleanup-app-registrations", "--cleanup-app-registrations"],
        ] {
            assert!(parse_options(&args(&values)).is_err());
        }
    }
}
