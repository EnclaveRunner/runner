use std::env::home_dir;

use anyhow::Ok;
use figment::{
    Figment,
    providers::{Format, Yaml},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ArtifactRegistry {
    host: String,
    port: u32,
}

impl Default for ArtifactRegistry {
    fn default() -> Self {
        Self {
            host: "artifactregistry".into(),
            port: 5000,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Redis {
    host: String,
    port: u32,
    db: u16,
    username: Option<String>,
    password: Option<String>,
}

impl Default for Redis {
    fn default() -> Self {
        Self {
            host: "redis".into(),
            port: 6379,
            db: 0,
            username: Option::None,
            password: Option::None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    artifact_registry: ArtifactRegistry,
    redis: Redis,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            artifact_registry: Default::default(),
            redis: Default::default(),
        }
    }
}

pub fn load_settings() -> Result<AppConfig, anyhow::Error> {
    let mut figment = Figment::new().merge(Yaml::file("runner.yaml"));

    if home_dir().is_some() {
        let home = home_dir().unwrap();
        let config_path = home.join(".enclave").join("runner.yaml");
        figment = figment.merge(Yaml::file(config_path));
    }

    figment = figment.merge(Yaml::file("/etc/enclave/runner.yaml"));

    let config: AppConfig = figment.extract()?;
    Ok(config)
}
