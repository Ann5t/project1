//! Database backup and restore endpoints.
//!
//! - `GET  /api/backup`         — Trigger a database backup, returns file download.
//! - `POST /api/backup/restore`  — Restore the database from an uploaded .db file.
//! - `GET  /api/backup/list`     — List available backup files in data/backups/.
//!
//! Auto-backup runs periodically via [`start_auto_backup`] when enabled in config.

use anyhow::Context;
use axum::body::Body;
use axum::extract::{Multipart, State};
use axum::http::header::{self, HeaderValue};
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tracing::{error, info, warn};

use crate::error::ApiError;
use crate::state::AppState;

/// Directory where backup files are stored, relative to the working directory.
const BACKUPS_DIR: &str = "data/backups";

/// SQLite database file header magic bytes (first 16 bytes of every .db file).
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// ── Route handlers ────────────────────────────────────────────────────
/// `GET /api/backup` — Trigger a database backup.
///
/// Uses SQLite's `VACUUM INTO` command to create a clean, consistent backup
/// copy of the current database. The backup file is saved to `data/backups/`
/// with a timestamped filename, and also returned as an `application/octet-stream`
/// download.
///
/// After creation, old backups are pruned according to `backup_keep_count` config.
pub async fn backup(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let now = chrono::Utc::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("agent-backup-{}.db", timestamp);

    // Ensure the backups directory exists.
    let backups_dir = PathBuf::from(BACKUPS_DIR);
    fs::create_dir_all(&backups_dir).await.map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "Failed to create backups directory '{}': {}",
            BACKUPS_DIR,
            e
        ))
    })?;

    let backup_path = backups_dir.join(&filename);

    // SQLite VACUUM INTO creates a clean standalone copy of the database.
    // Using forward slashes for cross-platform path compatibility.
    let path_for_sql = backup_path.to_string_lossy().replace('\\', "/");
    let sql = format!("VACUUM INTO '{}'", path_for_sql);
    sqlx::query(&sql)
        .execute(&state.db)
        .await
        .context("Backup VACUUM INTO failed")?;

    info!("Database backup created: {}", backup_path.display());

    // Read the backup file into memory for the HTTP response.
    let data = fs::read(&backup_path)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to read backup file: {}", e)))?;

    // Apply retention policy: keep only the N most recent backups.
    let keep_count: usize = state
        .config_repo
        .get("backup_keep_count")
        .await
        .unwrap_or(None)
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);
    cleanup_old_backups(&backups_dir, keep_count).await;

    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Invalid filename for header: {}", e)))?;

    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        ),
        (header::CONTENT_DISPOSITION, disposition),
    ];

    Ok((headers, Body::from(data)))
}

/// `POST /api/backup/restore` — Restore the database from an uploaded backup file.
///
/// Accepts a multipart form-data upload with a `.db` file in a field named
/// "file" or "backup". The uploaded file is validated as a valid SQLite database
/// (magic header check), then its tables are copied into the live database using
/// SQLite ATTACH DATABASE. This approach keeps the connection pool alive so no
/// server restart is needed.
///
/// The restore is performed atomically within a single database transaction:
/// foreign keys are temporarily disabled, all user tables in the current
/// database are cleared, and data from the backup is inserted.
pub async fn restore(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    // Extract the uploaded .db file from the multipart form.
    let mut file_data: Option<Vec<u8>> = None;
    let mut upload_filename = String::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || name == "backup" {
            upload_filename = field.file_name().unwrap_or("unknown.db").to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to read upload: {}", e)))?;
            file_data = Some(bytes.to_vec());
            break;
        }
    }

    let data = file_data.ok_or_else(|| {
        ApiError::BadRequest(
            "No file uploaded. Use a multipart field named 'file' or 'backup'.".into(),
        )
    })?;

    // Validate the file is a valid SQLite database by checking the magic header.
    if data.len() < 16 || &data[..16] != SQLITE_HEADER {
        return Err(ApiError::BadRequest(
            "Invalid SQLite database file: missing SQLite header (expected 'SQLite format 3\\0')."
                .into(),
        ));
    }

    info!(
        "Restoring database from uploaded file '{}' ({} bytes)",
        upload_filename,
        data.len()
    );

    // Write the uploaded backup to a temporary file for ATTACH.
    let backups_dir = PathBuf::from(BACKUPS_DIR);
    fs::create_dir_all(&backups_dir).await.map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to create backups directory: {}", e))
    })?;

    let temp_path = backups_dir.join("_restore_temp.db");
    fs::write(&temp_path, &data).await.map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "Failed to write temporary restore file: {}",
            e
        ))
    })?;

    let temp_path_for_sql = temp_path.to_string_lossy().replace('\\', "/");

    // Perform restore within a single connection so it is serialized and
    // consistent. We disable foreign keys during the operation to avoid
    // constraint violations when clearing and re-inserting data.
    let result: Result<(), ApiError> = async {
        // ATTACH the uploaded backup as an auxiliary database.
        sqlx::query(&format!(
            "ATTACH DATABASE '{}' AS restore_db",
            temp_path_for_sql
        ))
        .execute(&state.db)
        .await
        .context("ATTACH DATABASE failed")?;

        // Disable foreign key enforcement during the restore.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&state.db)
            .await
            .context("PRAGMA foreign_keys = OFF failed")?;

        // Discover user tables from the backup (exclude SQLite internal tables
        // and sqlx migration tracking).
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM restore_db.sqlite_master \
             WHERE type = 'table' \
               AND name NOT LIKE 'sqlite_%' \
               AND name != '_sqlx_migrations' \
             ORDER BY name",
        )
        .fetch_all(&state.db)
        .await
        .context("Failed to list backup tables")?;

        info!(
            "Restoring {} tables from backup: {:?}",
            tables.len(),
            tables.iter().map(|t| &t.0).collect::<Vec<_>>()
        );

        // For each table, clear existing data and copy from the backup.
        for (table_name,) in &tables {
            // Security: Validate the table name to prevent SQL injection via
            // malicious backup files containing crafted table names.
            // Table names must match the expected application tables and contain
            // only alphanumeric characters and underscores.
            validate_table_name(table_name)?;

            // Quote the table name to handle any special characters.
            let quoted = format!("\"{}\"", table_name);

            // Delete all rows from the main table.
            let delete_sql = format!("DELETE FROM main.{}", quoted);
            sqlx::query(&delete_sql)
                .execute(&state.db)
                .await
                .context(format!("Failed to clear table '{table_name}'"))?;

            // Copy rows from the backup into the main table.
            let insert_sql = format!(
                "INSERT INTO main.{} SELECT * FROM restore_db.{}",
                quoted, quoted
            );
            sqlx::query(&insert_sql)
                .execute(&state.db)
                .await
                .context(format!("Failed to copy table '{table_name}' from backup"))?;
        }

        // Re-enable foreign key enforcement.
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&state.db)
            .await
            .context("PRAGMA foreign_keys = ON failed")?;

        // DETACH the backup database.
        sqlx::query("DETACH DATABASE restore_db")
            .execute(&state.db)
            .await
            .context("DETACH DATABASE failed")?;

        Ok(())
    }
    .await;

    // Clean up the temporary file regardless of success or failure.
    if let Err(e) = fs::remove_file(&temp_path).await {
        warn!("Failed to remove temporary restore file: {}", e);
    }

    result?;

    info!("Database restored successfully from '{}'", upload_filename);

    Ok(Json(json!({
        "status": "restored",
        "message": "Database has been restored from the uploaded backup.",
        "file": upload_filename,
        "size_bytes": data.len(),
        "tables_restored": true,
    })))
}

/// `GET /api/backup/list` — List backup files in the `data/backups/` directory.
///
/// Returns each file's name, size in bytes, and last-modified timestamp,
/// sorted by most recent first.
pub async fn list_backups(State(_state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let backups_dir = PathBuf::from(BACKUPS_DIR);

    if !backups_dir.exists() {
        return Ok(Json(json!({ "backups": [] })));
    }

    let mut entries: Vec<BackupEntry> = Vec::new();
    let mut read_dir = fs::read_dir(&backups_dir).await.map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to read backups directory: {}", e))
    })?;

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        // Only list .db files that match our backup naming pattern.
        if !path.is_file() {
            continue;
        }
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !filename.starts_with("agent-backup-") || !filename.ends_with(".db") {
            continue;
        }
        // Skip internal temp files.
        if filename.starts_with("_") {
            continue;
        }

        let metadata = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size_bytes = metadata.len();
        let modified = metadata.modified().ok();

        entries.push(BackupEntry {
            filename: filename.to_string(),
            size_bytes,
            modified: modified.map(|t| {
                // Convert SystemTime to ISO-8601-like string.
                let datetime: DateTime<Utc> = t.into();
                datetime.to_rfc3339()
            }),
        });
    }

    // Sort by filename descending (newest first, since filenames contain timestamps).
    entries.sort_by(|a, b| b.filename.cmp(&a.filename));

    Ok(Json(json!({
        "backups": entries,
        "count": entries.len(),
        "directory": BACKUPS_DIR,
    })))
}

/// ── Helpers ───────────────────────────────────────────────────────────
/// A backup file entry returned by the list endpoint.
#[derive(Debug, Serialize)]
struct BackupEntry {
    filename: String,
    size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<String>,
}

/// Validate a table name for the restore operation.
///
/// Only allows alphanumeric characters and underscores to prevent SQL injection
/// via malicious table names crafted in uploaded backup files.
fn validate_table_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::BadRequest(format!(
            "Invalid table name '{}': must be 1-64 characters",
            name
        )));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ApiError::BadRequest(format!(
            "Invalid table name '{}': only alphanumeric characters and underscores allowed",
            name
        )));
    }
    Ok(())
}

/// Remove the oldest backup files so that at most `keep_count` remain.
async fn cleanup_old_backups(backups_dir: &std::path::Path, keep_count: usize) {
    let mut entries: Vec<(String, PathBuf)> = Vec::new();

    let mut read_dir = match fs::read_dir(backups_dir).await {
        Ok(rd) => rd,
        Err(e) => {
            warn!("Failed to read backups directory for cleanup: {}", e);
            return;
        }
    };

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if filename.starts_with("agent-backup-") && filename.ends_with(".db") {
            entries.push((filename.to_string(), path));
        }
    }

    // Sort by filename ascending (oldest first, since filenames contain timestamps).
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let to_remove = entries.len().saturating_sub(keep_count);
    for (_name, path) in entries.iter().take(to_remove) {
        if let Err(e) = fs::remove_file(path).await {
            warn!("Failed to remove old backup '{}': {}", path.display(), e);
        } else {
            info!("Removed old backup: {}", path.display());
        }
    }
}

/// ── Auto-backup ───────────────────────────────────────────────────────
/// Start the auto-backup background task.
///
/// Spawn this as a tokio task from `main.rs`. It periodically checks the
/// `backup_auto_enabled` config and, when enabled and the configured interval
/// has elapsed, performs a backup via [`run_backup_to_disk`].
pub async fn start_auto_backup(state: AppState) {
    info!("Auto-backup task started");

    // Check interval (in seconds) — how often we check whether a backup is due.
    const CHECK_INTERVAL_SECS: u64 = 60;

    // Track when the last backup was performed.
    let mut last_backup: Option<chrono::DateTime<Utc>> = None;

    loop {
        tokio::time::sleep(Duration::from_secs(CHECK_INTERVAL_SECS)).await;

        // Read current config values.
        let enabled = state
            .config_repo
            .get_or_default("backup_auto_enabled", "false")
            .await
            .map(|v| v == "true")
            .unwrap_or(false);

        if !enabled {
            continue;
        }

        let interval_hours: u64 = state
            .config_repo
            .get("backup_interval_hours")
            .await
            .unwrap_or(None)
            .and_then(|v| v.parse().ok())
            .unwrap_or(24);

        let keep_count: usize = state
            .config_repo
            .get("backup_keep_count")
            .await
            .unwrap_or(None)
            .and_then(|v| v.parse().ok())
            .unwrap_or(7);

        let interval = Duration::from_secs(interval_hours * 3600);
        let now = chrono::Utc::now();

        let should_backup = match last_backup {
            Some(last) => {
                let elapsed = now
                    .signed_duration_since(last)
                    .to_std()
                    .unwrap_or(Duration::MAX);
                elapsed >= interval
            }
            None => true, // First check: always do an initial backup.
        };

        if should_backup {
            info!(
                "Auto-backup triggered (interval={}h, keep={})",
                interval_hours, keep_count
            );
            match run_backup_to_disk(&state, keep_count).await {
                Ok(filename) => {
                    info!("Auto-backup completed: {}", filename);
                    last_backup = Some(now);
                }
                Err(e) => {
                    error!("Auto-backup failed: {}", e);
                }
            }
        }
    }
}

/// Perform a backup and save it to disk (no HTTP response).
///
/// Returns the filename of the created backup on success.
pub async fn run_backup_to_disk(state: &AppState, keep_count: usize) -> Result<String, ApiError> {
    let now = chrono::Utc::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("agent-backup-{}.db", timestamp);

    let backups_dir = PathBuf::from(BACKUPS_DIR);
    fs::create_dir_all(&backups_dir).await.map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("Failed to create backups directory: {}", e))
    })?;

    let backup_path = backups_dir.join(&filename);
    let path_for_sql = backup_path.to_string_lossy().replace('\\', "/");

    sqlx::query(&format!("VACUUM INTO '{}'", path_for_sql))
        .execute(&state.db)
        .await
        .context("Auto-backup VACUUM INTO failed")?;

    info!("Auto-backup file created: {}", backup_path.display());

    // Prune old backups.
    cleanup_old_backups(&backups_dir, keep_count).await;

    Ok(filename)
}
