//! Scoped maintenance of Server Manager's Launch Services references.
//! This module never deletes files, resets the global database, or stops applications.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Default, Clone)]
pub struct CleanupOptions {
    pub dry_run: bool,
    pub build_bundle_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupFailure {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub supported: bool,
    pub candidates: Vec<String>,
    pub removed: Vec<String>,
    pub failures: Vec<CleanupFailure>,
    pub preserved: usize,
    pub dry_run: bool,
}

pub fn cleanup_app_registrations(options: CleanupOptions) -> Result<CleanupReport, String> {
    #[cfg(target_os = "macos")]
    {
        platform::cleanup(options)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(CleanupReport {
            dry_run: options.dry_run,
            ..CleanupReport::default()
        })
    }
}

#[cfg(any(target_os = "macos", test))]
mod platform {
    use std::{
        collections::{BTreeSet, HashMap},
        fs,
        io::Read,
        path::{Component, Path, PathBuf},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use super::{CleanupFailure, CleanupOptions, CleanupReport};

    const IDENTIFIER: &str = "com.lawcorp.servermanager";
    const APP_NAME: &str = "Server Manager.app";
    const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
    const TOTAL_TIMEOUT: Duration = Duration::from_secs(60);
    const MAX_DUMP_BYTES: u64 = 32 * 1024 * 1024;

    struct Context {
        home: PathBuf,
        current_bundle: Option<PathBuf>,
    }

    #[derive(Clone)]
    enum Presence {
        Missing,
        Present(PathBuf),
        Symlink,
        Unknown,
    }

    trait Registry {
        fn dump(&mut self) -> Result<String, String>;
        fn unregister(&mut self, path: &Path) -> Result<(), String>;
    }

    struct SystemRegistry {
        deadline: Instant,
    }

    impl SystemRegistry {
        fn execute(&self, arguments: &[&std::ffi::OsStr]) -> Result<String, String> {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(
                    "Se agotó el tiempo para actualizar el registro de aplicaciones".into(),
                );
            }
            run_command(arguments, COMMAND_TIMEOUT.min(remaining))
        }
    }

    impl Registry for SystemRegistry {
        fn dump(&mut self) -> Result<String, String> {
            self.execute(&[std::ffi::OsStr::new("-dump")])
        }

        fn unregister(&mut self, path: &Path) -> Result<(), String> {
            self.execute(&[std::ffi::OsStr::new("-u"), path.as_os_str()])
                .map(|_| ())
        }
    }

    pub(super) fn cleanup(options: CleanupOptions) -> Result<CleanupReport, String> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| safe_absolute_path(path))
            .ok_or_else(|| "No se pudo determinar la carpeta del usuario".to_string())?;
        let executable = std::env::current_exe()
            .map_err(|error| format!("No se pudo identificar la aplicación actual: {error}"))?;
        let current_bundle = executable
            .ancestors()
            .find(|path| path.extension().is_some_and(|extension| extension == "app"))
            .map(|path| path.canonicalize().unwrap_or_else(|_| path.to_owned()));
        let context = Context {
            home,
            current_bundle,
        };
        let mut registry = SystemRegistry {
            deadline: Instant::now() + TOTAL_TIMEOUT,
        };
        clean_with_registry(options, &context, presence, &mut registry)
    }

    fn clean_with_registry(
        options: CleanupOptions,
        context: &Context,
        file_presence: impl Fn(&Path) -> Presence,
        registry: &mut impl Registry,
    ) -> Result<CleanupReport, String> {
        if options
            .build_bundle_path
            .as_deref()
            .is_some_and(|path| !valid_build_bundle(path))
        {
            return Err(
                "La ruta de compilación no corresponde a un bundle de Server Manager".into(),
            );
        }
        let entries = own_registrations(&registry.dump()?);
        let candidates: Vec<PathBuf> = entries
            .iter()
            .filter(|path| eligible(path, &options, context, file_presence(path)))
            .cloned()
            .collect();
        let mut report = CleanupReport {
            supported: true,
            preserved: entries.len() - candidates.len(),
            candidates: candidates.iter().map(|path| display(path)).collect(),
            dry_run: options.dry_run,
            ..CleanupReport::default()
        };
        if options.dry_run || candidates.is_empty() {
            return Ok(report);
        }

        let mut attempts = Vec::new();
        let mut errors = HashMap::new();
        for path in candidates {
            // A disk could have mounted or a valid copy could have appeared since the first dump.
            if !eligible(&path, &options, context, file_presence(&path)) {
                report.preserved += 1;
                continue;
            }
            if let Err(error) = registry.unregister(&path) {
                errors.insert(path.clone(), error);
            }
            attempts.push(path);
        }
        if attempts.is_empty() {
            return Ok(report);
        }
        match registry.dump() {
            Ok(after) => {
                let remaining = own_registrations(&after);
                for path in attempts {
                    if remaining.contains(&path) {
                        report.failures.push(CleanupFailure {
                            path: display(&path),
                            message: errors.remove(&path).unwrap_or_else(|| {
                                "macOS todavía conserva esta referencia; se puede reintentar".into()
                            }),
                        });
                    } else {
                        report.removed.push(display(&path));
                    }
                }
            }
            Err(error) => {
                for path in attempts {
                    report.failures.push(CleanupFailure {
                        path: display(&path),
                        message: format!("No se pudo verificar la limpieza: {error}"),
                    });
                }
            }
        }
        Ok(report)
    }

    fn display(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    fn presence(path: &Path) -> Presence {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => Presence::Symlink,
            Ok(_) => path
                .canonicalize()
                .map(Presence::Present)
                .unwrap_or(Presence::Unknown),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Presence::Missing,
            Err(_) => Presence::Unknown,
        }
    }

    fn safe_absolute_path(path: &Path) -> bool {
        path.is_absolute()
            && path.to_str().is_some_and(|value| {
                !value.chars().any(char::is_control)
                    && !value.split('/').any(|part| part == "." || part == "..")
            })
            && !path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    }

    fn installed_path(path: &Path, context: &Context) -> bool {
        path.starts_with("/Applications") || path.starts_with(context.home.join("Applications"))
    }

    fn eligible(
        path: &Path,
        options: &CleanupOptions,
        context: &Context,
        presence: Presence,
    ) -> bool {
        if !safe_absolute_path(path)
            || installed_path(path, context)
            || context.current_bundle.as_deref() == Some(path)
            || matches!(presence, Presence::Symlink | Presence::Unknown)
        {
            return false;
        }
        if let Presence::Present(canonical) = &presence {
            if installed_path(canonical, context)
                || context.current_bundle.as_deref() == Some(canonical)
            {
                return false;
            }
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let is_own_name =
            name == APP_NAME || (name.starts_with("Server Manager ") && name.ends_with(".app"));
        if !is_own_name {
            return false;
        }
        // Trash entries remain recoverable. Only their Launch Services references are removed.
        if path.parent() == Some(context.home.join(".Trash").as_path()) {
            return true;
        }
        if options.build_bundle_path.as_deref() == Some(path) {
            return true;
        }
        if name != APP_NAME || !matches!(presence, Presence::Missing) {
            return false;
        }
        let Some(volume) = path.parent() else {
            return false;
        };
        if volume.parent() != Some(Path::new("/Volumes")) {
            return false;
        }
        let volume_name = volume
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        volume_name == "Server Manager"
            || volume_name.strip_prefix("dmg.").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
    }

    fn valid_build_bundle(path: &Path) -> bool {
        if !safe_absolute_path(path) {
            return false;
        }
        let parts: Vec<_> = path.iter().filter_map(|part| part.to_str()).collect();
        if parts.len() < 6 || parts[parts.len() - 3..] != ["bundle", "macos", APP_NAME] {
            return false;
        }
        let profile = parts.len() - 4;
        if !matches!(parts[profile], "release" | "debug") {
            return false;
        }
        parts[profile - 1] == "target"
            || (profile >= 2
                && parts[profile - 2] == "target"
                && matches!(
                    parts[profile - 1],
                    "aarch64-apple-darwin" | "x86_64-apple-darwin" | "universal-apple-darwin"
                ))
    }

    fn own_registrations(dump: &str) -> BTreeSet<PathBuf> {
        let mut entries = BTreeSet::new();
        let mut path = None;
        let mut identifier = None;
        let mut duplicate = false;
        let flush = |path: &mut Option<PathBuf>,
                     identifier: &mut Option<String>,
                     duplicate: &mut bool,
                     entries: &mut BTreeSet<PathBuf>| {
            if !*duplicate && identifier.as_deref() == Some(IDENTIFIER) {
                if let Some(value) = path.take().filter(|path| safe_absolute_path(path)) {
                    entries.insert(value);
                }
            }
            *path = None;
            *identifier = None;
            *duplicate = false;
        };
        for line in dump.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("bundle id:")
                || (trimmed.len() >= 10 && trimmed.bytes().all(|byte| byte == b'-'))
            {
                flush(&mut path, &mut identifier, &mut duplicate, &mut entries);
            }
            let Some((key, value)) = trimmed.split_once(':') else {
                continue;
            };
            match key.trim() {
                "path" => {
                    if path.is_some() {
                        duplicate = true;
                    }
                    path = Some(PathBuf::from(strip_hex_suffix(value.trim())));
                }
                "identifier" => {
                    if identifier.is_some() {
                        duplicate = true;
                    }
                    identifier = Some(value.trim().to_owned());
                }
                _ => {}
            }
        }
        flush(&mut path, &mut identifier, &mut duplicate, &mut entries);
        entries
    }

    fn strip_hex_suffix(value: &str) -> &str {
        if let Some((path, suffix)) = value.rsplit_once(" (0x") {
            if suffix.strip_suffix(')').is_some_and(|hex| {
                !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            }) {
                return path;
            }
        }
        value
    }

    fn run_command(arguments: &[&std::ffi::OsStr], timeout: Duration) -> Result<String, String> {
        let mut child = Command::new(LSREGISTER)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                format!("No se pudo consultar el registro de aplicaciones: {error}")
            })?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let reader = |stream: Box<dyn Read + Send>, limit| {
            thread::spawn(move || {
                let mut bytes = Vec::new();
                stream
                    .take(limit + 1)
                    .read_to_end(&mut bytes)
                    .map(|_| bytes)
            })
        };
        let out = reader(Box::new(stdout), MAX_DUMP_BYTES);
        let err = reader(Box::new(stderr), 128 * 1024);
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
                result => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(match result {
                        Err(error) => {
                            format!("No se pudo esperar al registro de aplicaciones: {error}")
                        }
                        _ => "El registro de aplicaciones tardó demasiado en responder".into(),
                    });
                }
            }
        };
        let stdout = out
            .join()
            .map_err(|_| "No se pudo leer la respuesta del registro")?
            .map_err(|error| error.to_string())?;
        let stderr = err
            .join()
            .map_err(|_| "No se pudo leer el error del registro")?
            .map_err(|error| error.to_string())?;
        let status = status?;
        if stdout.len() as u64 > MAX_DUMP_BYTES || stderr.len() > 128 * 1024 {
            return Err(
                "La respuesta del registro de aplicaciones excedió el límite permitido".into(),
            );
        }
        if !status.success() {
            let detail: String = String::from_utf8_lossy(&stderr).chars().take(500).collect();
            return Err(format!(
                "macOS no pudo actualizar el registro ({status}): {}",
                detail.trim()
            ));
        }
        String::from_utf8(stdout).map_err(|_| "El registro devolvió una respuesta ilegible".into())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::collections::VecDeque;

        fn context() -> Context {
            Context {
                home: "/Users/test".into(),
                current_bundle: Some("/Applications/Server Manager.app".into()),
            }
        }
        fn record(path: &str, identifier: &str) -> String {
            format!("--------------------------------\nbundle id: Server Manager (0x1234)\npath: {path} (0x1234)\nname: Server Manager\nidentifier: {identifier}\nversionString: 0.4.0\n")
        }
        fn own(path: &str) -> String {
            record(path, IDENTIFIER)
        }
        struct FakeRegistry {
            dumps: VecDeque<Result<String, String>>,
            calls: Vec<PathBuf>,
            fail: bool,
        }
        impl Registry for FakeRegistry {
            fn dump(&mut self) -> Result<String, String> {
                self.dumps.pop_front().expect("unexpected dump")
            }
            fn unregister(&mut self, path: &Path) -> Result<(), String> {
                self.calls.push(path.to_owned());
                if self.fail {
                    Err("example failure".into())
                } else {
                    Ok(())
                }
            }
        }
        #[test]
        fn parses_only_exact_bundle_records_and_preserves_literal_paths() {
            let path = "/Users/test/.Trash/Server Manager 'old' (0xab).app";
            let dump = own(path)
                + &record(
                    "/Volumes/dmg.ABC/Server Manager.app",
                    "com.other.servermanager",
                )
                + &record("/tmp/Server Manager.app", "com.lawcorp.servermanager.fake")
                + &own(path)
                + &own("/Volumes/../Applications/Server Manager.app");
            assert_eq!(
                own_registrations(&dump),
                BTreeSet::from([PathBuf::from(path)])
            );
            assert!(own_registrations(
                &(own(path) + "path: /Volumes/dmg.def/Server Manager.app\n")
            )
            .is_empty());
        }
        #[test]
        fn selects_only_own_trash_and_missing_known_installer_paths() {
            let options = CleanupOptions::default();
            let ctx = context();
            for path in [
                "/Users/test/.Trash/Server Manager old 0.1.app",
                "/Volumes/dmg.ABC123/Server Manager.app",
                "/Volumes/Server Manager/Server Manager.app",
            ] {
                assert!(
                    eligible(Path::new(path), &options, &ctx, Presence::Missing),
                    "{path}"
                );
            }
            for path in [
                "/Applications/Server Manager.app",
                "/Users/test/Applications/Server Manager.app",
                "/Users/other/.Trash/Server Manager.app",
                "/Volumes/External/Server Manager.app",
                "/Volumes/dmg.ABC/nested/Server Manager.app",
                "/tmp/Server Manager.app",
                "/Volumes/dmg.abc-bad/Server Manager.app",
                "/Users/test/.Trash/Other.app",
            ] {
                assert!(
                    !eligible(Path::new(path), &options, &ctx, Presence::Missing),
                    "{path}"
                );
            }
            let mounted = Path::new("/Volumes/dmg.ABC/Server Manager.app");
            assert!(!eligible(
                mounted,
                &options,
                &ctx,
                Presence::Present(mounted.into())
            ));
            let trash = Path::new("/Users/test/.Trash/Server Manager.app");
            assert!(!eligible(trash, &options, &ctx, Presence::Unknown));
            assert!(!eligible(trash, &options, &ctx, Presence::Symlink));
            assert!(!eligible(
                trash,
                &options,
                &ctx,
                Presence::Present("/Applications/Server Manager.app".into())
            ));
            let running = Context {
                home: ctx.home,
                current_bundle: Some(trash.into()),
            };
            assert!(!eligible(trash, &options, &running, Presence::Missing));
        }
        #[test]
        fn accepts_supported_build_layouts_but_preserves_current_or_installed_bundles() {
            for suffix in [
                "target/release",
                "target/debug",
                "target/aarch64-apple-darwin/release",
                "target/x86_64-apple-darwin/debug",
                "target/universal-apple-darwin/release",
            ] {
                let path = PathBuf::from(format!("/repo/{suffix}/bundle/macos/Server Manager.app"));
                assert!(valid_build_bundle(&path));
                let options = CleanupOptions {
                    build_bundle_path: Some(path.clone()),
                    dry_run: false,
                };
                assert!(eligible(
                    &path,
                    &options,
                    &context(),
                    Presence::Present(path.clone())
                ));
                let current = Context {
                    home: "/Users/test".into(),
                    current_bundle: Some(path.clone()),
                };
                assert!(!eligible(
                    &path,
                    &options,
                    &current,
                    Presence::Present(path.clone())
                ));
            }
            for path in [
                "target/release/bundle/macos/Server Manager.app",
                "/repo/target/release/bundle/macos/Other.app",
                "/repo/target/release/../release/bundle/macos/Server Manager.app",
                "/repo/not-target/release/bundle/macos/Server Manager.app",
            ] {
                assert!(!valid_build_bundle(Path::new(path)), "{path}");
            }
        }
        #[test]
        fn dry_run_does_not_unregister_and_actual_cleanup_reports_verified_partial_results() {
            let trash = "/Users/test/.Trash/Server Manager 0.3.app";
            let volume = "/Volumes/dmg.ABC/Server Manager.app";
            let installed = "/Applications/Server Manager.app";
            let before = own(trash)
                + &own(volume)
                + &own(installed)
                + &record("/Volumes/dmg.ABC/Other.app", "org.other.app");
            let mut fake = FakeRegistry {
                dumps: VecDeque::from([Ok(before.clone())]),
                calls: vec![],
                fail: false,
            };
            let report = clean_with_registry(
                CleanupOptions {
                    dry_run: true,
                    ..Default::default()
                },
                &context(),
                |_| Presence::Missing,
                &mut fake,
            )
            .unwrap();
            assert_eq!(report.candidates.len(), 2);
            assert_eq!(report.preserved, 1);
            assert!(report.removed.is_empty() && fake.calls.is_empty());
            let mut fake = FakeRegistry {
                dumps: VecDeque::from([Ok(before), Ok(own(volume) + &own(installed))]),
                calls: vec![],
                fail: true,
            };
            let report = clean_with_registry(
                CleanupOptions::default(),
                &context(),
                |_| Presence::Missing,
                &mut fake,
            )
            .unwrap();
            assert_eq!(report.removed, vec![trash]);
            assert_eq!(report.failures.len(), 1);
            assert_eq!(report.failures[0].path, volume);
            assert_eq!(report.failures[0].message, "example failure");
            assert_eq!(
                fake.calls,
                vec![PathBuf::from(trash), PathBuf::from(volume)]
            );
        }
        #[test]
        fn does_not_claim_success_if_verification_fails_or_delete_a_newly_mounted_copy() {
            let path = "/Volumes/dmg.ABC/Server Manager.app";
            let mut fake = FakeRegistry {
                dumps: VecDeque::from([Ok(own(path)), Err("registry unavailable".into())]),
                calls: vec![],
                fail: false,
            };
            let report = clean_with_registry(
                CleanupOptions::default(),
                &context(),
                |_| Presence::Missing,
                &mut fake,
            )
            .unwrap();
            assert!(report.removed.is_empty());
            assert!(report.failures[0].message.contains("verificar"));
            let calls = std::cell::Cell::new(0);
            let mut fake = FakeRegistry {
                dumps: VecDeque::from([Ok(own(path))]),
                calls: vec![],
                fail: false,
            };
            let report = clean_with_registry(
                CleanupOptions::default(),
                &context(),
                |path| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        Presence::Missing
                    } else {
                        Presence::Present(path.into())
                    }
                },
                &mut fake,
            )
            .unwrap();
            assert!(fake.calls.is_empty());
            assert_eq!(report.preserved, 1);
        }
    }
}
