use std::{collections::BTreeMap, net::IpAddr, path::Path};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceKind {
    Api,
    App,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub id: String,
    pub name: String,
    pub kind: ServiceKind,
    pub working_directory: String,
    pub start_command: String,
    pub stop_command: Option<String>,
    #[serde(default)]
    pub project_url: Option<String>,
    #[serde(default)]
    pub environment_variables: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInput {
    pub id: Option<String>,
    pub name: String,
    pub kind: ServiceKind,
    pub working_directory: String,
    pub start_command: String,
    pub stop_command: Option<String>,
    pub project_url: Option<String>,
    #[serde(default)]
    pub environment_variables: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub working_directory: String,
    #[serde(default)]
    pub ssh_ip: Option<String>,
    pub icon: Option<String>,
    pub services: Vec<ServiceConfig>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub id: Option<String>,
    pub name: String,
    pub working_directory: String,
    #[serde(default)]
    pub ssh_ip: Option<String>,
    pub icon: Option<String>,
    pub services: Vec<ServiceInput>,
}

impl ProjectConfig {
    /// ProcessManager receives a copy with its directory resolved against the project root.
    pub fn resolved_service(&self, service: &ServiceConfig) -> ServiceConfig {
        let mut resolved = service.clone();
        let directory = Path::new(&service.working_directory);
        if !directory.is_absolute() {
            resolved.working_directory = Path::new(&self.working_directory)
                .join(directory)
                .to_string_lossy()
                .into_owned();
        }
        resolved
    }
}

impl ProjectInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("El nombre del proyecto es obligatorio".into());
        }
        if self.working_directory.trim().is_empty() {
            return Err("La carpeta raíz del proyecto es obligatoria".into());
        }
        if !Path::new(self.working_directory.trim()).is_absolute() {
            return Err("La carpeta raíz debe ser una ruta absoluta".into());
        }
        if self.services.is_empty() {
            return Err("Agrega al menos una API o app al proyecto".into());
        }
        if self
            .ssh_ip
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .is_some_and(|value| value.trim().parse::<IpAddr>().is_err())
        {
            return Err(
                "La IP SSH debe ser una dirección IPv4 o IPv6 válida, sin puertos ni comandos"
                    .into(),
            );
        }
        for service in &self.services {
            service
                .validate()
                .map_err(|error| format!("{}: {error}", service.name.trim()))?;
        }
        Ok(())
    }

    pub fn into_new_config(self) -> ProjectConfig {
        let now = Utc::now().to_rfc3339();
        ProjectConfig {
            id: self.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: self.name.trim().to_owned(),
            working_directory: self.working_directory.trim().to_owned(),
            ssh_ip: clean_optional(self.ssh_ip),
            icon: clean_optional(self.icon),
            services: self
                .services
                .into_iter()
                .map(ServiceInput::into_config)
                .collect(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

impl ServiceInput {
    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("El nombre del servicio es obligatorio".into());
        }
        if self.id.as_deref().is_some_and(|id| id.trim().is_empty()) {
            return Err("El identificador del servicio no puede estar vacío".into());
        }
        if self.start_command.trim().is_empty() {
            return Err("El comando de inicio es obligatorio".into());
        }
        if self.working_directory.trim().is_empty() {
            return Err("La carpeta del servicio es obligatoria; usa . para la raíz".into());
        }
        if let Some(value) = self
            .project_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let url = url::Url::parse(value.trim())
                .map_err(|_| "La URL del servicio no es válida".to_string())?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return Err("La URL del servicio debe usar http o https".into());
            }
        }
        if self
            .environment_variables
            .keys()
            .any(|key| key.trim().is_empty())
        {
            return Err("Las variables de entorno no pueden tener una clave vacía".into());
        }
        Ok(())
    }

    fn into_config(self) -> ServiceConfig {
        ServiceConfig {
            id: self.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: self.name.trim().into(),
            kind: self.kind,
            working_directory: self.working_directory.trim().into(),
            start_command: self.start_command.trim().into(),
            stop_command: clean_optional(self.stop_command),
            project_url: clean_optional(self.project_url),
            environment_variables: self.environment_variables,
        }
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
