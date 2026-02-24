use std::env::home_dir;

use figment::{
    Figment,
    providers::{Format, Yaml},
};
use serde::{Deserialize, Serialize};
use sloggers::{Build, types::Severity};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ArtifactRegistry {
    pub host: String,
    pub port: u32,
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
    pub host: String,
    pub port: u16,
    pub db: u16,
    pub username: Option<String>,
    pub password: Option<String>,
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
pub struct Database {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db: String,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            host: "postgres".to_string(),
            port: 5432,
            username: "enclave".to_string(),
            password: "enclave".to_string(),
            db: "enclave".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub artifact_registry: ArtifactRegistry,
    pub redis: Redis,
    pub database: Database,
    pub log_level: Severity,
    pub human_readable_output: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            artifact_registry: Default::default(),
            redis: Default::default(),
            database: Default::default(),
            log_level: Severity::Info,
            human_readable_output: false,
        }
    }
}

pub fn load_config() -> AppConfig {
    let mut figment = Figment::new().merge(Yaml::file("runner.yml"));

    if home_dir().is_some() {
        let home = home_dir().unwrap();
        let config_path = home.join(".enclave").join("runner.yml");
        figment = figment.merge(Yaml::file(config_path));
    }

    figment = figment.merge(Yaml::file("/etc/enclave/runner.yml"));

    figment = figment.merge(figment::providers::Env::prefixed("ENCLAVE_"));

    figment.extract().expect("Failed to load config")
}

pub fn configure_logger(config: &AppConfig) -> slog::Logger {
    sloggers::terminal::TerminalLoggerBuilder::new()
        .level(config.log_level)
        .format(if config.human_readable_output {
            sloggers::types::Format::Full
        } else {
            sloggers::types::Format::Json
        })
        .build()
        .expect("Failed to create logger")
}
