use anyhow::{Error, anyhow};
use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions, LogsOptions, RemoveContainerOptions, StartContainerOptions, WaitContainerOptions};
use bollard::image::CreateImageOptions;
use slog::{Logger, info};
use tokio_stream::StreamExt;

use crate::api;

pub async fn execute(
    logger: Logger,
    task: &api::task::Task,
    always_pull: bool,
) -> Result<Vec<u8>, Error> {
    let function = task
        .function
        .as_ref()
        .ok_or_else(|| anyhow!("Task has no function identifier"))?;

    let image_name = function.name.clone();

    let docker = Docker::connect_with_local_defaults()
        .map_err(|e| anyhow!("Failed to connect to Docker daemon: {}", e))?;

    if always_pull {
        info!(logger, "Pulling image"; "image" => &image_name);
        let mut pull_stream = docker.create_image(
            Some(CreateImageOptions {
                from_image: image_name.as_str(),
                ..Default::default()
            }),
            None,
            None,
        );
        while let Some(item) = pull_stream.next().await {
            item.map_err(|e| anyhow!("Failed to pull image: {}", e))?;
        }
    }

    let env: Vec<String> = task
        .environment_variables
        .iter()
        .map(|e| format!("{}={}", e.key, e.value))
        .collect();

    info!(logger, "Creating container"; "image" => &image_name);
    let container = docker
        .create_container(
            None::<CreateContainerOptions<String>>,
            Config {
                image: Some(image_name.clone()),
                env: Some(env),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| anyhow!("Failed to create container: {}", e))?;

    let container_id = container.id;

    docker
        .start_container(&container_id, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| anyhow!("Failed to start container: {}", e))?;

    info!(logger, "Waiting for container"; "id" => &container_id);
    let mut wait_stream = docker.wait_container(&container_id, None::<WaitContainerOptions<String>>);
    while let Some(result) = wait_stream.next().await {
        result.map_err(|e| anyhow!("Container wait error: {}", e))?;
    }

    let mut logs_stream = docker.logs(
        &container_id,
        Some(LogsOptions::<String> {
            stdout: true,
            stderr: true,
            ..Default::default()
        }),
    );

    let mut output = Vec::new();
    while let Some(item) = logs_stream.next().await {
        let chunk = item.map_err(|e| anyhow!("Failed to read container logs: {}", e))?;
        output.extend_from_slice(chunk.into_bytes().as_ref());
    }

    docker
        .remove_container(
            &container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .map_err(|e| anyhow!("Failed to remove container: {}", e))?;

    info!(logger, "Container execution complete"; "image" => &image_name);
    Ok(output)
}
