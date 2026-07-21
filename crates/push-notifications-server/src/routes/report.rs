use axum::{Json, extract::State, http::StatusCode};

use report_common::{ReportRequest, now_unix_millis, verify_report};

use crate::{AppState, error::AppError};

pub(crate) async fn report(
    State(state): State<AppState>,
    Json(req): Json<ReportRequest>,
) -> Result<StatusCode, AppError> {
    let rows = verify_report(&req, now_unix_millis())?;
    let count = rows.len();
    state.db.store_reports(rows).await?;
    tracing::info!(reports = count, "stored device reports");
    Ok(StatusCode::NO_CONTENT)
}
