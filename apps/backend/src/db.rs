use std::path::Path;

use crate::error::{AppError, AppResult};

#[cfg(all(feature = "db-sqlite", feature = "db-postgres"))]
compile_error!("enable only one of: db-sqlite, db-postgres");

#[cfg(not(any(feature = "db-sqlite", feature = "db-postgres")))]
compile_error!("enable one of: db-sqlite, db-postgres");

#[cfg(feature = "db-sqlite")]
pub type DbPool = sqlx::SqlitePool;

#[cfg(feature = "db-postgres")]
pub type DbPool = sqlx::PgPool;

pub async fn connect(database_url: &str) -> AppResult<DbPool> {
    #[cfg(feature = "db-sqlite")]
    {
        ensure_sqlite_parent_dir(database_url)?;
        return sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| AppError::internal(format!("sqlite connect: {e}")));
    }

    #[cfg(feature = "db-postgres")]
    {
        return sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| AppError::internal(format!("postgres connect: {e}")));
    }
}

pub async fn migrate(pool: &DbPool) -> AppResult<()> {
    #[cfg(feature = "db-sqlite")]
    {
        sqlx::migrate!("./migrations/sqlite")
            .run(pool)
            .await
            .map_err(|e| AppError::internal(format!("sqlite migrate: {e}")))?;
    }

    #[cfg(feature = "db-postgres")]
    {
        sqlx::migrate!("./migrations/postgres")
            .run(pool)
            .await
            .map_err(|e| AppError::internal(format!("postgres migrate: {e}")))?;
    }

    Ok(())
}

#[cfg(feature = "db-sqlite")]
fn ensure_sqlite_parent_dir(database_url: &str) -> AppResult<()> {
    let path = sqlite_file_path(database_url)?;
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::internal(format!("create db dir {}: {e}", parent.display())))?;
        }
    }
    Ok(())
}

#[cfg(feature = "db-sqlite")]
fn sqlite_file_path(database_url: &str) -> AppResult<String> {
    let rest = database_url
        .strip_prefix("sqlite:")
        .ok_or_else(|| AppError::BadRequest(
            "db-sqlite build expects DATABASE_URL like sqlite:./data/jarvis.db?mode=rwc".into(),
        ))?;
    let path = rest.split('?').next().unwrap_or(rest).trim();
    let path = path.strip_prefix("//").unwrap_or(path);
    if path.is_empty() || path == ":memory:" {
        return Ok(path.to_string());
    }
    Ok(path.to_string())
}
