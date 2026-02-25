use std::collections::HashMap;

use anyhow::{Error, anyhow};
use asynq::{
    backend::RedisConnectionType,
    serve_mux::ServeMux,
    server::{Server, ServerConfig},
    task::Task,
};
use prost::Message;
use slog::{Logger, error, info};
use sqlx::{Pool, Postgres};
use tonic::transport::Channel;

use crate::{
    api::{self, registry::registry_service_client::RegistryServiceClient},
    orm::{self, LogIssuer, LogLevel, TaskStatus},
    registry, wasm_host,
};

const TASK_TYPE_NORMAL: &str = "job:normal";

pub async fn start_processor(
    logger: Logger,
    redis_config: RedisConnectionType,
    db_pool: Pool<Postgres>,
    registry_client: RegistryServiceClient<Channel>,
) -> Result<(), asynq::error::Error> {
    let mut queues = HashMap::new();
    queues.insert("critical".to_string(), 6);
    queues.insert("default".to_string(), 3);
    queues.insert("low".to_string(), 1);

    let config = ServerConfig::new().concurrency(1).queues(queues);

    let mut mux = ServeMux::new();

    mux.handle_async_func(TASK_TYPE_NORMAL, move |queue_task: Task| {
        let logger = logger.clone();
        let db_pool = db_pool.clone();
        let registry_client = registry_client.clone();
        async move {
            handle_task(logger.clone(), db_pool, registry_client, queue_task)
                .await
                .map_err(|e| {
                    error!(logger, "Task failed"; "error" => %e);
                    asynq::error::Error::other(e.to_string())
                })
        }
    });

    let mut server = Server::new(redis_config, config).await?;

    server.run(mux).await
}

async fn handle_task(
    logger: Logger,
    db_pool: Pool<Postgres>,
    registry_client: RegistryServiceClient<Channel>,
    queue_task: Task,
) -> Result<(), Error> {
    let task = api::task::Task::decode(queue_task.payload.as_slice())?;
    let logger = logger.new(slog::o!("task_id" => task.task_id.clone()));

    log_task_assigned(logger.new(slog::o!()), db_pool.clone(), &task.task_id).await?;
    let artifact = fetch_artifact(logger.new(slog::o!()), registry_client, &task).await?;
    log_task_running(logger.new(slog::o!()), db_pool.clone(), &task.task_id).await?;
    execute_artifact(logger.new(slog::o!()), db_pool.clone(), &task, artifact).await?;

    Ok(())
}

async fn log_task_assigned(
    logger: Logger,
    db: Pool<Postgres>,
    task_id: &str,
) -> Result<(), Error> {
    info!(logger, "Task assigned");
    orm::update_status(db.clone(), task_id.to_owned(), TaskStatus::Assigned).await?;
    orm::log(
        db,
        task_id.to_owned(),
        LogLevel::Info,
        LogIssuer::System,
        b"Task assigned".to_vec(),
    )
    .await
}

async fn log_task_running(
    logger: Logger,
    db: Pool<Postgres>,
    task_id: &str,
) -> Result<(), Error> {
    info!(logger, "Task running");
    orm::update_status(db.clone(), task_id.to_owned(), TaskStatus::Running).await?;
    orm::log(
        db,
        task_id.to_owned(),
        LogLevel::Info,
        LogIssuer::System,
        b"Task running".to_vec(),
    )
    .await
}

async fn fetch_artifact(
    logger: Logger,
    registry_client: RegistryServiceClient<Channel>,
    task: &api::task::Task,
) -> Result<Vec<u8>, Error> {
    let identifier = task
        .function
        .as_ref()
        .ok_or_else(|| anyhow!("Task has no function identifier"))?
        .artifact
        .clone()
        .ok_or_else(|| anyhow!("Task has no artifact identifier"))?;
    info!(logger, "Fetching artifact");
    registry::retrieve_artifact(registry_client, identifier).await
}

async fn execute_artifact(
    logger: Logger,
    db: Pool<Postgres>,
    task: &api::task::Task,
    artifact: Vec<u8>,
) -> Result<(), Error> {
    match wasm_host::execute_wasm(logger, db.clone(), task.clone(), artifact).await {
        Ok(()) => {}
        Err(err) => return Err(anyhow!("Failed to execute task: {}", err)),
    };
    Ok(())
}
