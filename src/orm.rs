use anyhow::Error;
use sqlx::{Pool, Postgres};
use time::OffsetDateTime;
use uuid::Uuid;

pub enum LogLevel {
    #[allow(dead_code)]
    Debug,
    Info,
    Error,
    #[allow(dead_code)]
    Fatal,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }
}

pub enum LogIssuer {
    System,
    Artifact,
}

impl LogIssuer {
    fn as_str(&self) -> &'static str {
        match self {
            LogIssuer::System => "SYSTEM",
            LogIssuer::Artifact => "ARTIFACT",
        }
    }
}

pub async fn log(
    db: Pool<Postgres>,
    task_id: String,
    level: LogLevel,
    issuer: LogIssuer,
    data: Vec<u8>,
) -> Result<(), Error> {
    let uuid = Uuid::parse_str(&task_id)?;
    sqlx::query(
        "INSERT INTO task_logs (task_id, timestamp, status, issuer, payload) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(uuid)
    .bind(OffsetDateTime::now_utc())
    .bind(level.as_str())
    .bind(issuer.as_str())
    .bind(data)
    .execute(&db)
    .await?;

    Ok(())
}
