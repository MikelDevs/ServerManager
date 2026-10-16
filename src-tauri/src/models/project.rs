use std::{collections::BTreeMap, net::IpAddr, path::Path};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub working_directory: String,
    pub start_command: String,
    pub stop_command: Option<String>,
    #[serde(default)]
    pub project_url: Option<String>,
    #[serde(default)]
    pub ssh_ip: Option<String>,
    #[serde(default)]
    pub environment_variables: BTreeMap<String, String>,
    pub icon: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub id: Option<String>,
    pub name: String,
    pub working_directory: String,
    pub start_command: String,
    pub stop_command: Option<String>,
    pub project_url: Option<String>,
    #[serde(default)]
    pub ssh_ip: Option<String>,
    #[serde(default)]
    pub environment_variables: BTreeMap<String, String>,
    pub icon: Option<String>,
}

impl ProjectInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("El nombre del proyecto es obligatorio".into());
        }
        if self.start_command.trim().is_empty() {
            return Err("El comando de inicio es obligatorio".into());
        }
        if self.working_directory.trim().is_empty() {
            return Err("La carpeta de trabajo es obligatoria".into());
        }
        if !Path::new(self.working_directory.trim()).is_absolute() {
            return Err("La carpeta de trabajo debe ser una ruta absoluta".into());
        }
        if let Some(value) = self
            .project_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let url = url::Url::parse(value.trim())
                .map_err(|_| "La URL del proyecto no es válida".to_string())?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return Err("La URL del proyecto debe usar http o https".into());
            }
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
        if self
            .environment_variables
            .keys()
            .any(|key| key.trim().is_empty())
        {
            return Err("Las variables de entorno no pueden tener una clave vacía".into());
        }
        Ok(())
    }

    pub fn into_new_config(self) -> ProjectConfig {
        let now = Utc::now().to_rfc3339();
        ProjectConfig {
            id: self.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: self.name.trim().to_owned(),
            working_directory: self.working_directory.trim().to_owned(),
            start_command: self.start_command.trim().to_owned(),
            stop_command: self
                .stop_command
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            project_url: self
                .project_url
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            ssh_ip: self
                .ssh_ip
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            environment_variables: self.environment_variables,
            icon: self
                .icon
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}
