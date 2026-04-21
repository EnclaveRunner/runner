use std::collections::HashMap;

use anyhow::{Error, anyhow};
use asynq::{
    backend::RedisConnectionType,
    serve_mux::ServeMux,
    server::{Server, ServerConfig},
    task::Task,
};
use prost::Message;
use slog::{Logger, error, info, warn};
use sqlx::{Pool, Postgres};

use crate::{
    api,
    docker_exec,
    orm::{self, LogIssuer, LogLevel},
};

const TASK_TYPE_NORMAL: &str = "job:normal";

pub async fn start_processor(
    logger: Logger,
    redis_config: RedisConnectionType,
    db_pool: Pool<Postgres>,
    always_pull: bool,
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
        let logger = logger.new(slog::o!("task_id" => task_id.clone()));
        let db_pool = db_pool.clone();
        let db_pool_error = db_pool.clone();
        async move {
            handle_task(logger.clone(), db_pool, always_pull, queue_task)
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
    always_pull: bool,
    queue_task: Task,
) -> Result<(), Error> {
    let task = api::task::Task::decode(queue_task.payload.as_slice())?;
    let task_id = queue_task.result_writer().unwrap().task_id().to_string();

    let measurement = extract_measurement(&task);

    log_task_assigned(logger.new(slog::o!()), db_pool.clone(), &task_id).await?;
    send_measurement(&logger, &measurement, "assigned").await;
    send_measurement(&logger, &measurement, "pulled").await;
    log_task_running(logger.new(slog::o!()), db_pool.clone(), &task_id).await?;

    let result = docker_exec::execute(logger.new(slog::o!()), &task, always_pull)
        .await
        .map_err(|err| anyhow!("Execution failed: {}", err))?;

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

    send_measurement(&logger, &measurement, "cleanup").await;
    Ok(())
}

struct Measurement {
    id: String,
    server: String,
}

fn extract_measurement(task: &api::task::Task) -> Option<Measurement> {
    let mut id = None;
    let mut server = None;
    for env in &task.environment_variables {
        match env.key.as_str() {
            "MEASUREMENT_ID" => id = Some(env.value.clone()),
            "MEASUREMENT_SERVER" => server = Some(env.value.clone()),
            _ => {}
        }
    }
    match (id, server) {
        (Some(id), Some(server)) => Some(Measurement { id, server }),
        _ => None,
    }
}

async fn send_measurement(logger: &Logger, measurement: &Option<Measurement>, endpoint: &str) {
    let Some(m) = measurement else { return };
    let url = format!(
        "{}/benchmarks/{}?request={}",
        m.server.trim_end_matches('/'),
        endpoint,
        m.id,
    );
    match reqwest::Client::new().get(&url).send().await {
        Ok(_) => {}
        Err(err) => warn!(logger, "Failed to send measurement"; "endpoint" => endpoint, "error" => %err),
    }
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
