use std::collections::HashMap;

use anyhow::{anyhow, Error};
use asynq::{
    backend::RedisConnectionType,
    serve_mux::ServeMux,
    server::{Server, ServerConfig},
    task::Task,
};
use prost::Message;
use slog::{error, info, Logger};
use sqlx::{Pool, Postgres};
use tonic::transport::Channel;

use crate::{
    api::{self, registry::registry_service_client::RegistryServiceClient},
    orm::{self, LogIssuer, LogLevel, TaskStatus},
    registry,
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
            let task = match api::task::Task::decode(queue_task.payload.as_slice()) {
                Ok(task) => task,
                Err(err) => {
                    error!(logger, "Failed to decode task body"; "error" => %err);
                    return Err(asynq::error::Error::other(err.to_string()));
                }
            };

            let logger = logger.new(slog::o!("task_id" => task.task_id.clone()));

            log_task_assigned(logger.clone(), db_pool.clone(), task.task_id.clone())
                .await
                .map_err(|e| asynq::error::Error::other(e.to_string()))?;

            let artifact = fetch_artifact(logger.clone(), registry_client, &task)
                .await
                .map_err(|e| asynq::error::Error::other(e.to_string()))?;

            log_task_running(logger.clone(), db_pool.clone(), task.task_id.clone())
                .await
                .map_err(|e| asynq::error::Error::other(e.to_string()))?;

            execute_artifact(logger.clone(), db_pool.clone(), &task, artifact)
                .await
                .map_err(|e| asynq::error::Error::other(e.to_string()))?;

            Ok(())
        }
    });

    let mut server = Server::new(redis_config, config).await?;

    server.run(mux).await
}

async fn log_task_assigned(logger: Logger, db: Pool<Postgres>, task_id: String) -> Result<(), Error> {
    info!(logger, "Task assigned");
    orm::update_status(db.clone(), task_id.clone(), TaskStatus::Assigned).await?;
    orm::log(db, task_id, LogLevel::Info, LogIssuer::System, b"Task assigned".to_vec()).await
}

async fn log_task_running(logger: Logger, db: Pool<Postgres>, task_id: String) -> Result<(), Error> {
    info!(logger, "Task running");
    orm::update_status(db.clone(), task_id.clone(), TaskStatus::Running).await?;
    orm::log(db, task_id, LogLevel::Info, LogIssuer::System, b"Task running".to_vec()).await
}

async fn fetch_artifact(
    logger: Logger,
    registry_client: RegistryServiceClient<Channel>,
    task: &api::task::Task,
) -> Result<Vec<u8>, Error> {
    let identifier = task
        .artifact
        .clone()
        .ok_or_else(|| anyhow!("Task has no artifact identifier"))?;
    info!(logger, "Fetching artifact");
    registry::retrieve_artifact(registry_client, identifier).await
}

async fn execute_artifact(
    logger: Logger,
    _db: Pool<Postgres>,
    _task: &api::task::Task,
    _artifact: Vec<u8>,
) -> Result<(), Error> {
    info!(logger, "Executing artifact");
    // TODO: implement wasmtime execution with task.function and task.input
    Ok(())
}
