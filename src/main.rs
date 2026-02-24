use std::{clone, process::{ExitCode, exit}};

use slog::{debug, info, error};

use crate::api::registry::{ArtifactIdentifier, FullyQualifiedName, artifact_identifier::Identifier};

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

#[tokio::main]
async fn main() -> ExitCode {
    let app_config = &config::load_config();
    let logger = config::configure_logger(&app_config);

    info!(logger, "Initlizied config!");

    let registry_client = match api::registry::registry_service_client::RegistryServiceClient::connect(format!("{}:{}", app_config.artifact_registry.host, app_config.artifact_registry.port)).await {
        Ok(client) => client,
        Err(err) => {
            error!(logger, "Failed to connect to artifact registry"; "error" => %err);
            return ExitCode::FAILURE
        }
    };

    let 

    let fqn = FullyQualifiedName {
        source: "enclave".to_string(),
        author: "example".to_string(),
        name: "hello-world".to_string(),
    };

    let identifier = "0.0.1";

    let artifact_identifier = ArtifactIdentifier {
        fqn: Some(fqn.clone()),
        identifier: Some(Identifier::Tag(identifier.to_string()))
    };

    let artifact_content = match registry::retrieve_artifact(registry_client, artifact_identifier).await {
        Ok(content) => content,
        Err(err) => {
            error!(logger, "Failed to retrieve artifact"; "error" => %err, "artifact" => format!("{}:{}/{}@{}", fqn.source, fqn.author, fqn.name, identifier));
            return ExitCode::FAILURE
        }
    };

    info!(logger, "Successfully pulled artifact!");
    ExitCode::SUCCESS
}
