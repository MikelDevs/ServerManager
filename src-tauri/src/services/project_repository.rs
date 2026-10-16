use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use chrono::Utc;

use crate::models::{ProjectConfig, ProjectInput};

#[derive(Clone)]
pub struct ProjectRepository {
    path: PathBuf,
    projects: Arc<RwLock<Vec<ProjectConfig>>>,
}

impl ProjectRepository {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let projects = if path.exists() {
            let contents = fs::read_to_string(&path)
                .map_err(|error| format!("No se pudo leer la configuración: {error}"))?;
            serde_json::from_str(&contents)
                .map_err(|error| format!("La configuración guardada no es válida: {error}"))?
        } else {
            Vec::new()
        };

        Ok(Self {
            path,
            projects: Arc::new(RwLock::new(projects)),
        })
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

    pub fn save(&self, input: ProjectInput) -> Result<ProjectConfig, String> {
        input.validate()?;
        let mut next_projects = self.list();

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

        self.persist(&next_projects)?;
        *self
            .projects
            .write()
            .expect("project repository lock poisoned") = next_projects;
        Ok(saved)
    }

    pub fn delete(&self, project_id: &str) -> Result<(), String> {
        let mut next_projects = self.list();
        let original_length = next_projects.len();
        next_projects.retain(|project| project.id != project_id);
        if next_projects.len() == original_length {
            return Err("No se encontró el proyecto".into());
        }

        self.persist(&next_projects)?;
        *self
            .projects
            .write()
            .expect("project repository lock poisoned") = next_projects;
        Ok(())
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

        #[cfg(windows)]
        if self.path.exists() {
            fs::remove_file(&self.path)
                .map_err(|error| format!("No se pudo reemplazar la configuración: {error}"))?;
        }

        fs::rename(&temporary_path, &self.path)
            .map_err(|error| format!("No se pudo activar la configuración guardada: {error}"))
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use uuid::Uuid;

    use super::*;

    fn input(name: &str) -> ProjectInput {
        ProjectInput {
            id: None,
            name: name.into(),
            working_directory: std::env::temp_dir().to_string_lossy().into_owned(),
            start_command: "echo ready".into(),
            stop_command: Some("stop".into()),
            project_url: Some("http://localhost:3000".into()),
            ssh_ip: Some(" 192.168.1.20 ".into()),
            environment_variables: BTreeMap::from([("PORT".into(), "3000".into())]),
            icon: Some("🚀".into()),
        }
    }

    #[test]
    fn saves_reloads_updates_and_deletes_projects() {
        let directory =
            std::env::temp_dir().join(format!("server-manager-repository-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join("projects.json");
        let repository = ProjectRepository::load(path.clone()).expect("load empty repository");

        let created = repository.save(input("API")).expect("save project");
        assert_eq!(created.name, "API");
        assert_eq!(repository.list().len(), 1);

        let reloaded = ProjectRepository::load(path.clone()).expect("reload repository");
        assert_eq!(
            reloaded
                .get(&created.id)
                .expect("saved project")
                .stop_command
                .as_deref(),
            Some("stop")
        );
        assert_eq!(
            reloaded
                .get(&created.id)
                .expect("saved project")
                .project_url
                .as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            reloaded.get(&created.id).unwrap().ssh_ip.as_deref(),
            Some("192.168.1.20")
        );
        let saved_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved_json[0]["sshIp"], "192.168.1.20");

        let mut update = input("API actualizada");
        update.id = Some(created.id.clone());
        update.ssh_ip = Some(" 2001:db8::1 ".into());
        let updated = reloaded.save(update).expect("update project");
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.created_at, created.created_at);
        let reloaded = ProjectRepository::load(path.clone()).expect("reload updated repository");
        assert_eq!(
            reloaded.get(&created.id).unwrap().ssh_ip.as_deref(),
            Some("2001:db8::1")
        );

        let mut clear_ip = input("API actualizada");
        clear_ip.id = Some(created.id.clone());
        clear_ip.ssh_ip = Some(" \t\n ".into());
        reloaded.save(clear_ip).expect("clear saved IP");
        let reloaded = ProjectRepository::load(path).expect("reload cleared IP");
        assert!(reloaded.get(&created.id).unwrap().ssh_ip.is_none());

        reloaded.delete(&created.id).expect("delete project");
        assert!(reloaded.list().is_empty());
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn loads_projects_and_inputs_saved_before_ssh_ip_existed() {
        let directory =
            std::env::temp_dir().join(format!("server-manager-legacy-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join("projects.json");
        let mut legacy = serde_json::to_value(input("Legacy API").into_new_config()).unwrap();
        legacy.as_object_mut().unwrap().remove("sshIp");
        let legacy_input: ProjectInput = serde_json::from_value(legacy.clone()).unwrap();
        assert!(legacy_input.ssh_ip.is_none());
        fs::write(&path, serde_json::to_string(&vec![legacy]).unwrap()).unwrap();

        let repository = ProjectRepository::load(path).expect("load legacy project");
        assert_eq!(repository.list().len(), 1);
        assert!(repository.list()[0].ssh_ip.is_none());
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn rejects_invalid_ssh_ips_without_changing_saved_projects() {
        let directory =
            std::env::temp_dir().join(format!("server-manager-invalid-ip-test-{}", Uuid::new_v4()));
        let path = directory.join("projects.json");
        let repository = ProjectRepository::load(path.clone()).expect("load repository");
        let created = repository.save(input("API")).unwrap();
        let saved_before = fs::read(&path).unwrap();

        for value in [
            "example.com",
            "256.1.1.1",
            "192.168.1.20:22",
            "[::1]:22",
            "fe80::1%en0",
            "root@192.168.1.20",
            "127.0.0.1; whoami",
            "127.0.0.1\nwhoami",
            "127.0.0.1$(whoami)",
            "-oProxyCommand=whoami",
        ] {
            for id in [None, Some(created.id.clone())] {
                let mut invalid = input("Invalid");
                invalid.id = id;
                invalid.ssh_ip = Some(value.into());
                assert!(repository.save(invalid).is_err(), "accepted IP: {value}");
            }
        }

        assert_eq!(fs::read(path).unwrap(), saved_before);
        assert_eq!(repository.list().len(), 1);
        assert_eq!(
            repository.get(&created.id).unwrap().ssh_ip.as_deref(),
            Some("192.168.1.20")
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
