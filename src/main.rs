use std::process::ExitCode;

use asynq::backend::RedisConnectionType;
use redis::{ConnectionAddr, IntoConnectionInfo, RedisConnectionInfo};
use slog::{error, info};
use sqlx::postgres::PgPoolOptions;

pub mod api {
    pub mod registry {
        tonic::include_proto!("registry");
    }

    pub mod task {
        tonic::include_proto!("task");
    }
}

mod config;
mod orm;
mod queue;
mod registry;
mod wasm_host;

#[tokio::main]
async fn main() -> ExitCode {
    let app_config = &config::load_config();
    let logger = config::configure_logger(&app_config);

    info!(logger, "Initlizied config!");

    let artifact_registry_addr = format!(
        "{}:{}",
        app_config.artifact_registry.host, app_config.artifact_registry.port
    );

    let registry_client =
        match api::registry::registry_service_client::RegistryServiceClient::connect(
            artifact_registry_addr.clone(),
        )
        .await
        {
            Ok(client) => client,
            Err(err) => {
                error!(logger, "Failed to connect to artifact registry"; "error" => %err, "address" => artifact_registry_addr);
                return ExitCode::FAILURE;
            }
        };

    let mut redis_connection_info_details =
        RedisConnectionInfo::default().set_db(app_config.redis.db.into());
    if app_config.redis.username.is_some() {
        redis_connection_info_details = redis_connection_info_details
            .set_username(app_config.redis.username.clone().unwrap().as_str());
    }

    if app_config.redis.password.is_some() {
        redis_connection_info_details =
            redis_connection_info_details.set_password(app_config.redis.password.clone().unwrap());
    }

    let redis_connection_info =
        ConnectionAddr::Tcp(app_config.redis.host.clone(), app_config.redis.port)
            .into_connection_info()
            .unwrap()
            .set_redis_settings(redis_connection_info_details);

    let redis_config = match RedisConnectionType::single(redis_connection_info) {
        Ok(config) => config,
        Err(err) => {
            error!(logger, "Failed to connect to redis"; "error" => %err);
            return ExitCode::FAILURE;
        }
    };

    let pool = match PgPoolOptions::new()
        .connect(&format!(
            "postgres://{}:{}@{}:{}/{}",
            app_config.database.username,
            app_config.database.password,
            app_config.database.host,
            app_config.database.port,
            app_config.database.db
        ))
        .await
    {
        Ok(pool) => pool,
        Err(err) => {
            error!(logger, "Failed to connecto to postgres"; "host" => app_config.database.host.clone(), "port" => app_config.database.port, "db" => app_config.database.db.clone(), "error" => %err);
            return ExitCode::FAILURE;
        }
    };

    match queue::start_processor(logger.clone(), redis_config, pool, registry_client).await {
        Ok(_) => {}
        Err(err) => {
            error!(logger, "Failed to start task processor"; "error" => %err)
        }
    };

    ExitCode::SUCCESS
}
