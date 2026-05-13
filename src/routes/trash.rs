use axum::{http::StatusCode, response::IntoResponse, Json};

use crate::applescript::commands;
use crate::models::ErrorResponse;

#[utoipa::path(
    delete,
    path = "/trash",
    tag = "trash",
    responses(
        (status = 204, description = "Trash emptied"),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
    security(("bearer_auth" = [])),
)]
pub async fn empty_trash() -> impl IntoResponse {
    match commands::empty_trash() {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorResponse { error: e })),
        )
            .into_response(),
    }
}
