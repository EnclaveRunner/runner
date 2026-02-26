use std::collections::HashMap;

use anyhow::{Error, anyhow};
use asynq::{
    backend::RedisConnectionType,
    serve_mux::ServeMux,
    server::{Server, ServerConfig},
    task::{Task},
};
use prost::Message;
use slog::{Logger, error, info};
use sqlx::{Pool, Postgres};
use tonic::transport::Channel;

use crate::{
    api::{self, registry::registry_service_client::RegistryServiceClient},
    orm::{self, LogIssuer, LogLevel},
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
        let task_id = queue_task
            .clone()
            .result_writer()
            .unwrap()
            .task_id()
            .to_string();
        let task_id_for_error = task_id.clone();
        let logger = logger.new(slog::o!("task_id" => queue_task.options.task_id.clone()));
        let db_pool = db_pool.clone();
        let db_pool_error = db_pool.clone();
        let registry_client = registry_client.clone();
        async move {
            handle_task(logger.clone(), db_pool, registry_client, queue_task)
                .await
                .map_err(move |e| {
                    error!(logger, "Task failed"; "error" => %e);
                    tokio::spawn(orm::log(
                        db_pool_error,
                        task_id_for_error,
                        LogLevel::Fatal,
                        LogIssuer::System,
                        e.to_string(),
                    ));
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
    let task_id = queue_task.result_writer().unwrap().task_id().to_string();

    log_task_assigned(logger.new(slog::o!()), db_pool.clone(), &task_id).await?;
    let artifact = fetch_artifact(logger.new(slog::o!()), registry_client, &task).await?;
    log_task_running(logger.new(slog::o!()), db_pool.clone(), &task_id).await?;
    let result = execute_artifact(
        logger.new(slog::o!()),
        db_pool.clone(),
        &task,
        task_id,
        artifact,
    )
    .await?;

    let result_writer = match queue_task.result_writer() {
        Some(writer) => writer,
        None => {
            error!(logger, "Failed to write result due to writer being none");
            return Ok(());
        }
    };

    match result_writer.write(&result).await {
        Ok(_) => {}
        Err(err) => {
            error!(logger, "Failed to write result"; "error" => %err);
        }
    };
    Ok(())
}

async fn log_task_assigned(logger: Logger, db: Pool<Postgres>, task_id: &str) -> Result<(), Error> {
    info!(logger, "Task assigned");
    orm::log(
        db,
        task_id.to_owned(),
        LogLevel::Info,
        LogIssuer::System,
        "Task assigned".to_string(),
    )
    .await
}

async fn log_task_running(logger: Logger, db: Pool<Postgres>, task_id: &str) -> Result<(), Error> {
    info!(logger, "Task running");
    orm::log(
        db,
        task_id.to_owned(),
        LogLevel::Info,
        LogIssuer::System,
        "Task running".to_string(),
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
    task_id: String,
    artifact: Vec<u8>,
) -> Result<Vec<u8>, Error> {
    wasm_host::execute_wasm(logger, db.clone(), task.clone(), task_id, artifact)
        .await
        .map_err(|err| anyhow!("Execution failed: {}", err))
}
