use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use push_notifications_client::ValidationError;

pub enum AppError {
    Validation(ValidationError),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Validation(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            AppError::Internal(e) => {
                tracing::error!("{:#}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()).into_response()
            }
        }
    }
}

impl From<ValidationError> for AppError {
    fn from(e: ValidationError) -> Self {
        AppError::Validation(e)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e)
    }
}
