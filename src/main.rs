use std::fmt::format;

use slog::{debug, info};

pub mod api {
    pub mod registry {
        tonic::include_proto!("registry");
    }

    pub mod task {
        tonic::include_proto!("task");
    }
}

mod config;
mod registry;

fn main() {
    let app_config = &config::load_config();
    let logger = config::configure_logger(&app_config);

    info!(logger, "Initlizied config!");
    debug!(logger, "This is a debug message"; "key" => "value");

    let registry_client = api::registry::registry_service_client::RegistryServiceClient::connect(format!("{}:{}", app_config.artifact_registry.host, app_config.artifact_registry.port));
}
