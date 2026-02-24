use anyhow::{Error, Ok, anyhow};
use sqlx::{Pool, Postgres};
use time::OffsetDateTime;

pub enum TaskStatus {
    Queued,
    Assigned,
    Running,
    Succeeded,
    Failed,
}

impl TaskStatus {
    fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Queued => "QUEUED",
            TaskStatus::Assigned => "ASSIGNED",
            TaskStatus::Running => "RUNNING",
            TaskStatus::Succeeded => "SUCCEEDED",
            TaskStatus::Failed => "FAILED",
        }
    }
}

pub enum LogLevel {
    Debug,
    Info,
    Error,
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

pub async fn update_status(db: Pool<Postgres>, task_id: String, status: TaskStatus) -> Result<(), Error> {
    let result = sqlx::query("UPDATE virtual_tasks SET status = $1 WHERE task_id = $2")
        .bind(status.as_str())
        .bind(task_id)
        .execute(&db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(anyhow!("Task not found"));
    }

    Ok(())
}

pub async fn log(db: Pool<Postgres>, task_id: String, level: LogLevel, issuer: LogIssuer, data: Vec<u8>) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO task_logs (task_id, timestamp, status, issuer, payload) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(task_id)
    .bind(OffsetDateTime::now_utc())
    .bind(level.as_str())
    .bind(issuer.as_str())
    .bind(data)
    .execute(&db)
    .await?;

    Ok(())
}
