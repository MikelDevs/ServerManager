use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::models::{ProjectConfig, ProjectInput, ServiceConfig, ServiceKind};

#[derive(Clone)]
pub struct ProjectRepository {
    path: PathBuf,
    projects: Arc<RwLock<Vec<ProjectConfig>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProject {
    id: String,
    name: String,
    working_directory: String,
    start_command: String,
    stop_command: Option<String>,
    #[serde(default)]
    project_url: Option<String>,
    #[serde(default)]
    ssh_ip: Option<String>,
    #[serde(default)]
    environment_variables: BTreeMap<String, String>,
    icon: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<LegacyProject> for ProjectConfig {
    fn from(legacy: LegacyProject) -> Self {
        // Preserve each old entry rather than guessing which unrelated projects belong together.
        let service = ServiceConfig {
            id: legacy.id.clone(),
            name: legacy.name.clone(),
            kind: if legacy
                .name
                .split_whitespace()
                .any(|word| word.eq_ignore_ascii_case("api"))
            {
                ServiceKind::Api
            } else {
                ServiceKind::App
            },
            working_directory: ".".into(),
            start_command: legacy.start_command,
            stop_command: legacy.stop_command,
            project_url: legacy.project_url,
            environment_variables: legacy.environment_variables,
        };
        Self {
            id: legacy.id,
            name: legacy.name,
            working_directory: legacy.working_directory,
            ssh_ip: legacy.ssh_ip,
            icon: legacy.icon,
            services: vec![service],
            created_at: legacy.created_at,
            updated_at: legacy.updated_at,
        }
    }
}

impl ProjectRepository {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let mut migrated = false;
        let contents = if path.exists() {
            Some(
                fs::read_to_string(&path)
                    .map_err(|error| format!("No se pudo leer la configuración: {error}"))?,
            )
        } else {
            None
        };
        let projects = if let Some(contents) = &contents {
            let values: Vec<serde_json::Value> = serde_json::from_str(contents)
                .map_err(|error| format!("La configuración guardada no es válida: {error}"))?;
            let mut projects = Vec::with_capacity(values.len());
            let mut legacy_ids = HashSet::new();
            for value in values {
                let project = if value.get("services").is_some() {
                    serde_json::from_value::<ProjectConfig>(value)
                } else {
                    migrated = true;
                    serde_json::from_value::<LegacyProject>(value).map(|legacy| {
                        legacy_ids.insert(legacy.id.clone());
                        ProjectConfig::from(legacy)
                    })
                }
                .map_err(|error| format!("La configuración guardada no es válida: {error}"))?;
                projects.push(project);
            }
            validate_identifiers(&projects)?;
            let projects = group_legacy_projects(projects, &legacy_ids);
            validate_identifiers(&projects)?;
            projects
        } else {
            Vec::new()
        };

        let repository = Self {
            path,
            projects: Arc::new(RwLock::new(projects)),
        };
        if migrated {
            // An exact, non-overwriting backup is required before upgrading the file on disk.
            repository.backup_legacy(contents.as_deref().unwrap())?;
            repository.persist(&repository.list())?;
        }
        Ok(repository)
    }

    pub fn list(&self) -> Vec<ProjectConfig> {
        self.projects
            .read()
            .expect("project repository lock poisoned")
            .clone()
    }

    pub fn get(&self, project_id: &str) -> Option<ProjectConfig> {
        self.projects
            .read()
            .expect("project repository lock poisoned")
            .iter()
            .find(|project| project.id == project_id)
            .cloned()
    }

    pub fn get_service(&self, service_id: &str) -> Option<(ProjectConfig, ServiceConfig)> {
        self.projects
            .read()
            .expect("project repository lock poisoned")
            .iter()
            .find_map(|project| {
                project
                    .services
                    .iter()
                    .find(|service| service.id == service_id)
                    .map(|service| (project.clone(), project.resolved_service(service)))
            })
    }

    pub fn save(&self, input: ProjectInput) -> Result<ProjectConfig, String> {
        input.validate()?;
        let mut projects = self
            .projects
            .write()
            .expect("project repository lock poisoned");
        let mut next_projects = projects.clone();
        let saved = if let Some(id) = input.id.as_deref() {
            let index = next_projects
                .iter()
                .position(|project| project.id == id)
                .ok_or_else(|| "No se encontró el proyecto que se quiere editar".to_string())?;
            let created_at = next_projects[index].created_at.clone();
            let mut updated = input.into_new_config();
            updated.created_at = created_at;
            updated.updated_at = Utc::now().to_rfc3339();
            next_projects[index] = updated.clone();
            updated
        } else {
            let created = input.into_new_config();
            next_projects.push(created.clone());
            created
        };
        validate_identifiers(&next_projects)?;
        self.persist(&next_projects)?;
        *projects = next_projects;
        Ok(saved)
    }

    pub fn delete(&self, project_id: &str) -> Result<(), String> {
        let mut projects = self
            .projects
            .write()
            .expect("project repository lock poisoned");
        let mut next_projects = projects.clone();
        let original_length = next_projects.len();
        next_projects.retain(|project| project.id != project_id);
        if next_projects.len() == original_length {
            return Err("No se encontró el proyecto".into());
        }
        self.persist(&next_projects)?;
        *projects = next_projects;
        Ok(())
    }

    fn backup_legacy(&self, contents: &str) -> Result<(), String> {
        let mut backup_path = self.path.as_os_str().to_owned();
        backup_path.push(".v0.3.1.bak");
        let mut backup_path = PathBuf::from(backup_path);
        if backup_path.exists() {
            backup_path.set_file_name(format!("projects.json.v0.3.1.{}.bak", Uuid::new_v4()));
        }
        let mut backup = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup_path)
            .map_err(|error| format!("No se pudo respaldar la configuración anterior: {error}"))?;
        backup
            .write_all(contents.as_bytes())
            .and_then(|_| backup.sync_all())
            .map_err(|error| {
                format!("No se pudo completar el respaldo de la configuración: {error}")
            })
    }

    fn persist(&self, projects: &[ProjectConfig]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("No se pudo crear la carpeta de configuración: {error}")
            })?;
        }
        let serialized = serde_json::to_string_pretty(projects)
            .map_err(|error| format!("No se pudo serializar la configuración: {error}"))?;
        let temporary_path = temporary_path(&self.path);
        fs::write(&temporary_path, serialized)
            .map_err(|error| format!("No se pudo guardar la configuración temporal: {error}"))?;
        fs::rename(&temporary_path, &self.path)
            .map_err(|error| format!("No se pudo activar la configuración guardada: {error}"))
    }
}

// Only unambiguous API/App pairs with matching names and sibling folders are grouped.
fn legacy_group_key(project: &ProjectConfig) -> Option<(String, PathBuf, String)> {
    let service = project.services.first()?;
    let words: Vec<&str> = project.name.split_whitespace().collect();
    let (kind, name) = match (words.first(), words.last()) {
        (Some(first), _)
            if first.eq_ignore_ascii_case("api") || first.eq_ignore_ascii_case("app") =>
        {
            (first.to_lowercase(), words[1..].join(" "))
        }
        (_, Some(last)) if last.eq_ignore_ascii_case("api") || last.eq_ignore_ascii_case("app") => {
            (last.to_lowercase(), words[..words.len() - 1].join(" "))
        }
        _ => return None,
    };
    if name.is_empty() || (kind == "api") != (service.kind == ServiceKind::Api) {
        return None;
    }
    let directory = Path::new(&project.working_directory);
    if !directory.is_absolute() {
        return None;
    }
    let basename = directory.file_name()?.to_str()?.to_lowercase();
    if basename != kind && !basename.ends_with(&format!("-{kind}")) {
        return None;
    }
    Some((name.to_lowercase(), directory.parent()?.to_owned(), name))
}

fn group_legacy_projects(
    projects: Vec<ProjectConfig>,
    legacy_ids: &HashSet<String>,
) -> Vec<ProjectConfig> {
    let keys: Vec<_> = projects
        .iter()
        .map(|project| {
            if legacy_ids.contains(&project.id) {
                legacy_group_key(project)
            } else {
                None
            }
        })
        .collect();
    let mut consumed = HashSet::new();
    let mut grouped = Vec::new();
    for (index, project) in projects.iter().enumerate() {
        if consumed.contains(&index) {
            continue;
        }
        let mut project = project.clone();
        if let Some((name_key, parent, name)) = &keys[index] {
            let matching: Vec<usize> = keys
                .iter()
                .enumerate()
                .filter_map(|(candidate, key)| {
                    key.as_ref()
                        .filter(|(other_name, other_parent, _)| {
                            other_name == name_key && other_parent == parent
                        })
                        .map(|_| candidate)
                })
                .collect();
            if matching.len() == 2 {
                let other_index = *matching
                    .iter()
                    .find(|&&candidate| candidate != index)
                    .unwrap();
                let other = &projects[other_index];
                let ip = |project: &ProjectConfig| {
                    project
                        .ssh_ip
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| {
                            value
                                .parse::<std::net::IpAddr>()
                                .map(|ip| ip.to_string())
                                .unwrap_or_else(|_| value.to_owned())
                        })
                };
                let (first_ip, second_ip) = (ip(&project), ip(other));
                if project.services[0].kind != other.services[0].kind
                    && (first_ip.is_none() || second_ip.is_none() || first_ip == second_ip)
                {
                    project.name = name.clone();
                    project.working_directory = parent.to_string_lossy().into_owned();
                    project.ssh_ip = first_ip.or(second_ip);
                    project.icon = project.icon.or_else(|| other.icon.clone());
                    project.created_at = project.created_at.min(other.created_at.clone());
                    project.updated_at = project.updated_at.max(other.updated_at.clone());
                    project.services = matching
                        .iter()
                        .map(|&candidate| {
                            let original = &projects[candidate];
                            let mut service = original.services[0].clone();
                            service.working_directory = Path::new(&original.working_directory)
                                .file_name()
                                .unwrap()
                                .to_string_lossy()
                                .into_owned();
                            service
                        })
                        .collect();
                    consumed.insert(other_index);
                }
            }
        }
        grouped.push(project);
    }
    grouped
}

fn validate_identifiers(projects: &[ProjectConfig]) -> Result<(), String> {
    let mut project_ids = HashSet::new();
    let mut service_ids = HashSet::new();
    for project in projects {
        if project.id.trim().is_empty() || !project_ids.insert(&project.id) {
            return Err(
                "La configuración contiene identificadores de proyecto vacíos o duplicados".into(),
            );
        }
        for service in &project.services {
            if service.id.trim().is_empty() || !service_ids.insert(&service.id) {
                return Err("Los identificadores de las APIs y apps deben ser únicos en todos los proyectos".into());
            }
        }
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ServiceInput;
    use serde_json::json;

    fn input(name: &str) -> ProjectInput {
        ProjectInput {
            id: None,
            name: name.into(),
            working_directory: std::env::temp_dir().to_string_lossy().into_owned(),
            ssh_ip: Some(" 192.168.1.20 ".into()),
            icon: Some("🚀".into()),
            services: vec![
                ServiceInput {
                    id: None,
                    name: "API".into(),
                    kind: ServiceKind::Api,
                    working_directory: "api".into(),
                    start_command: "echo ready".into(),
                    stop_command: Some("stop".into()),
                    project_url: Some("http://localhost:3000".into()),
                    environment_variables: BTreeMap::from([("PORT".into(), "3000".into())]),
                },
                ServiceInput {
                    id: None,
                    name: "App".into(),
                    kind: ServiceKind::App,
                    working_directory: "app".into(),
                    start_command: "echo app".into(),
                    stop_command: None,
                    project_url: Some("http://localhost:5173".into()),
                    environment_variables: BTreeMap::new(),
                },
            ],
        }
    }
    fn test_path() -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("server-manager-repository-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        directory.join("projects.json")
    }

    #[test]
    fn saves_reloads_updates_and_deletes_projects_with_independent_services() {
        let path = test_path();
        let repository = ProjectRepository::load(path.clone()).unwrap();
        let created = repository.save(input("RH")).unwrap();
        assert_eq!(created.services.len(), 2);
        assert_ne!(created.services[0].id, created.services[1].id);
        let reloaded = ProjectRepository::load(path.clone()).unwrap();
        let saved = reloaded.get(&created.id).unwrap();
        assert_eq!(saved.ssh_ip.as_deref(), Some("192.168.1.20"));
        assert_eq!(saved.services[0].stop_command.as_deref(), Some("stop"));
        assert_eq!(saved.services[0].environment_variables["PORT"], "3000");
        assert_eq!(
            saved.services[1].project_url.as_deref(),
            Some("http://localhost:5173")
        );
        let mut update = input("RH actualizada");
        update.id = Some(saved.id.clone());
        for (new, old) in update.services.iter_mut().zip(&saved.services) {
            new.id = Some(old.id.clone());
        }
        update.services[0].start_command = "echo changed".into();
        update.ssh_ip = Some(" \t ".into());
        let updated = reloaded.save(update).unwrap();
        assert_eq!(updated.created_at, saved.created_at);
        assert_eq!(updated.services[0].id, saved.services[0].id);
        assert_eq!(updated.services[1].start_command, "echo app");
        assert!(updated.ssh_ip.is_none());
        reloaded.delete(&created.id).unwrap();
        assert!(ProjectRepository::load(path.clone())
            .unwrap()
            .list()
            .is_empty());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn migration_preserves_every_legacy_field_and_creates_exact_backup_once() {
        let path = test_path();
        let legacy = json!([{
            "id":"old-api", "name":"RH API", "workingDirectory":"/tmp/rh/api", "startCommand":"npm run dev",
            "stopCommand":"quit", "projectUrl":"http://localhost:4000", "sshIp":"192.168.1.20",
            "environmentVariables":{"PORT":"4000", "EXAMPLE":"a=b"}, "icon":"R",
            "createdAt":"2026-10-12T00:00:00Z", "updatedAt":"2026-10-13T00:00:00Z"
        }, {
            "id":"old-app", "name":"OTHER APP", "workingDirectory":"/tmp/rh/app", "startCommand":"npm start",
            "stopCommand":null, "icon":null, "createdAt":"before", "updatedAt":"after"
        }]);
        let original = serde_json::to_string_pretty(&legacy).unwrap();
        fs::write(&path, &original).unwrap();
        let repository = ProjectRepository::load(path.clone()).unwrap();
        assert_eq!(repository.list().len(), 2);
        let api = repository.get("old-api").unwrap();
        let service = &api.services[0];
        assert_eq!(api.working_directory, "/tmp/rh/api");
        assert_eq!(api.created_at, "2026-10-12T00:00:00Z");
        assert_eq!(api.updated_at, "2026-10-13T00:00:00Z");
        assert_eq!(api.ssh_ip.as_deref(), Some("192.168.1.20"));
        assert_eq!(service.kind, ServiceKind::Api);
        assert_eq!(service.working_directory, ".");
        assert_eq!(service.start_command, "npm run dev");
        assert_eq!(service.stop_command.as_deref(), Some("quit"));
        assert_eq!(
            service.project_url.as_deref(),
            Some("http://localhost:4000")
        );
        assert_eq!(service.environment_variables["EXAMPLE"], "a=b");
        assert_eq!(
            fs::read_to_string(path.with_file_name("projects.json.v0.3.1.bak")).unwrap(),
            original
        );
        let reloaded = ProjectRepository::load(path.clone()).unwrap();
        assert_eq!(reloaded.get("old-api").unwrap().services[0].id, service.id);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 2);
        assert!(reloaded.get("old-app").unwrap().ssh_ip.is_none());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn duplicate_ids_and_invalid_fields_do_not_change_saved_data() {
        let path = test_path();
        let repository = ProjectRepository::load(path.clone()).unwrap();
        let original = repository.save(input("RH")).unwrap();
        let before = fs::read(&path).unwrap();
        let mut duplicate = input("Other");
        duplicate.services[0].id = Some(original.services[0].id.clone());
        assert!(repository.save(duplicate).unwrap_err().contains("únicos"));
        let mut duplicate = input("Other");
        duplicate.services[0].id = Some("same".into());
        duplicate.services[1].id = Some("same".into());
        assert!(repository.save(duplicate).is_err());
        for ip in [
            "example.com",
            "256.1.1.1",
            "192.168.1.20:22",
            "127.0.0.1; whoami",
            "-oProxyCommand=whoami",
        ] {
            let mut invalid = input("Invalid");
            invalid.ssh_ip = Some(ip.into());
            assert!(repository.save(invalid).is_err());
        }
        let mut invalid = input("Invalid");
        invalid.services.clear();
        assert!(repository.save(invalid).is_err());
        let mut invalid = input("Invalid");
        invalid.services[0].project_url = Some("file:///tmp/test".into());
        assert!(repository.save(invalid).is_err());
        let mut invalid = input("Invalid");
        invalid.services[0].start_command = " ".into();
        assert!(repository.save(invalid).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(repository.list().len(), 1);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn resolves_relative_services_against_their_project_root_and_preserves_absolute_paths() {
        let path = test_path();
        let repository = ProjectRepository::load(path.clone()).unwrap();
        let mut config = input("RH");
        config.services[1].working_directory = std::env::temp_dir()
            .join("external-app")
            .to_string_lossy()
            .into_owned();
        let project = repository.save(config).unwrap();
        let (_, api) = repository.get_service(&project.services[0].id).unwrap();
        assert_eq!(
            Path::new(&api.working_directory),
            Path::new(&project.working_directory).join("api")
        );
        let (_, app) = repository.get_service(&project.services[1].id).unwrap();
        assert_eq!(app.working_directory, project.services[1].working_directory);
        assert_eq!(
            repository.get(&project.id).unwrap().services[0].working_directory,
            "api"
        );
        assert!(repository.get_service("missing").is_none());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    fn legacy_entry(id: &str, name: &str, directory: &str, ip: Option<&str>) -> serde_json::Value {
        json!({"id":id,"name":name,"workingDirectory":directory,"startCommand":format!("echo {id}"),
            "stopCommand":"stop","projectUrl":"http://localhost:4000","sshIp":ip,
            "environmentVariables":{"IDENTITY":id},"icon":null,"createdAt":"2026-10-10","updatedAt":"2026-10-20"})
    }

    #[test]
    fn migration_groups_rh_and_core_pairs_and_preserves_service_settings_and_ssh() {
        let path = test_path();
        let entries = vec![
            legacy_entry("rh-app", "APP RH", "/tmp/rh/app", None),
            legacy_entry(
                "core-api",
                "API CORE",
                "/tmp/core/capacitaciones-api",
                Some("2001:0db8::1"),
            ),
            legacy_entry("rh-api", "API RH", "/tmp/rh/api", Some("192.168.1.20")),
            legacy_entry(
                "core-app",
                "CORE APP",
                "/tmp/core/capacitaciones-app",
                Some("2001:db8::1"),
            ),
        ];
        let original = serde_json::to_string(&entries).unwrap();
        fs::write(&path, &original).unwrap();
        let repository = ProjectRepository::load(path.clone()).unwrap();
        let projects = repository.list();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].id, "rh-app");
        assert_eq!(projects[0].name, "RH");
        assert_eq!(projects[0].working_directory, "/tmp/rh");
        assert_eq!(projects[0].ssh_ip.as_deref(), Some("192.168.1.20"));
        assert_eq!(projects[0].services[0].id, "rh-app");
        assert_eq!(projects[0].services[0].working_directory, "app");
        assert_eq!(projects[0].services[1].id, "rh-api");
        assert_eq!(projects[0].services[1].working_directory, "api");
        assert_eq!(
            projects[0].services[1].environment_variables["IDENTITY"],
            "rh-api"
        );
        assert_eq!(projects[1].name, "CORE");
        assert_eq!(projects[1].working_directory, "/tmp/core");
        assert_eq!(projects[1].ssh_ip.as_deref(), Some("2001:db8::1"));
        assert_eq!(
            projects[1].services[0].working_directory,
            "capacitaciones-api"
        );
        assert_eq!(projects[1].services[1].start_command, "echo core-app");
        assert_eq!(
            repository
                .get_service("core-app")
                .unwrap()
                .1
                .working_directory,
            "/tmp/core/capacitaciones-app"
        );
        assert_eq!(
            fs::read_to_string(path.with_file_name("projects.json.v0.3.1.bak")).unwrap(),
            original
        );
        let upgraded = fs::read_to_string(&path).unwrap();
        let reloaded = ProjectRepository::load(path.clone()).unwrap();
        assert_eq!(reloaded.list().len(), 2);
        assert_eq!(fs::read_to_string(&path).unwrap(), upgraded);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn migration_leaves_ambiguous_names_parents_and_conflicting_ips_separate() {
        let path = test_path();
        let entries = vec![
            legacy_entry("a", "API RH", "/tmp/one/api", None),
            legacy_entry("b", "APP RH", "/tmp/two/app", None),
            legacy_entry("c", "API CORE", "/tmp/core/api", Some("192.168.1.1")),
            legacy_entry("d", "APP CORE", "/tmp/core/app", Some("192.168.1.2")),
            legacy_entry("e", "API MANY", "/tmp/many/api", None),
            legacy_entry("f", "APP MANY", "/tmp/many/app", None),
            legacy_entry("g", "MANY APP", "/tmp/many/admin-app", None),
            legacy_entry("h", "API UNKNOWN", "/tmp/unknown/backend", None),
            legacy_entry("i", "APP UNKNOWN", "/tmp/unknown/frontend", None),
        ];
        fs::write(&path, serde_json::to_string(&entries).unwrap()).unwrap();
        let repository = ProjectRepository::load(path.clone()).unwrap();
        assert_eq!(repository.list().len(), entries.len());
        assert!(repository
            .list()
            .iter()
            .all(|project| project.services.len() == 1));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
