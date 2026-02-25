use anyhow::Ok;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

use crate::api::{self, registry::registry_service_client::RegistryServiceClient};

pub async fn retrieve_artifact(
    mut registry: RegistryServiceClient<Channel>,
    identifier: api::registry::ArtifactIdentifier,
) -> Result<Vec<u8>, anyhow::Error> {
    let mut stream = registry.pull_artifact(identifier).await?.into_inner();

    let mut buffer: Vec<u8> = Vec::new();

    while let Some(response) = stream.next().await {
        buffer.append(&mut response?.data);
    }

    Ok(buffer)
}
