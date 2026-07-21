use axum::{extract::State, http::StatusCode, Json};
use redb::Database;
use report_common::{now_unix_millis, verify_report, ReportError, ReportRequest, ReportRow};

use crate::{reports_table::encode_report_row, AppState, REPORTS_TABLE};

/// Handle `POST /report`: verify the reporter's signature and the timestamp
/// freshness, then persist one row per reported device.
pub async fn report(
    State(state): State<AppState>,
    Json(payload): Json<ReportRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let rows = verify_report(&payload, now_unix_millis()).map_err(report_error_status)?;

    let db = state.db.clone();
    // redb's begin_write() blocks for exclusive access, so keep it off the async
    // worker threads (mirrors store_blips).
    tokio::task::spawn_blocking(move || store_reports(&db, &rows))
        .await
        .map_err(|e| {
            tracing::error!("Task join error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?
        .map_err(|e| {
            tracing::error!("{}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        })?;

    Ok(StatusCode::CREATED)
}

fn report_error_status(err: ReportError) -> (StatusCode, String) {
    let status = match err {
        ReportError::StaleTimestamp => StatusCode::BAD_REQUEST,
        ReportError::SignatureVerificationFailed => StatusCode::UNAUTHORIZED,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, err.to_string())
}

fn store_reports(db: &Database, rows: &[ReportRow]) -> Result<(), String> {
    let write_txn = db
        .begin_write()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;
    {
        let mut table = write_txn
            .open_table(REPORTS_TABLE)
            .map_err(|e| format!("Failed to open reports table: {}", e))?;
        for row in rows {
            let key = uuid::Uuid::now_v7().as_u128();
            table
                .insert(key, encode_report_row(row).as_slice())
                .map_err(|e| format!("Failed to insert report: {}", e))?;
        }
    }
    write_txn
        .commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;
    Ok(())
}
