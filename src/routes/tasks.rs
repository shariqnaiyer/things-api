use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::applescript::commands;
use crate::models::{CreateTask, ErrorResponse, TasksQuery, UpdateTask};

pub async fn list_tasks(Query(query): Query<TasksQuery>) -> impl IntoResponse {
    match commands::get_tasks(query.list.as_deref(), query.limit, query.offset) {
        Ok(tasks) => (StatusCode::OK, Json(serde_json::json!(tasks))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorResponse { error: e })),
        )
            .into_response(),
    }
}

pub async fn get_task(Path(id): Path<String>) -> impl IntoResponse {
    match commands::get_task_by_id(&id) {
        Ok(task) => (StatusCode::OK, Json(serde_json::json!(task))).into_response(),
        Err(e) => {
            let status = if e.contains("Can't get") || e.contains("doesn't understand") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(serde_json::json!(ErrorResponse { error: e }))).into_response()
        }
    }
}

pub async fn create_task(Json(payload): Json<CreateTask>) -> impl IntoResponse {
    match commands::create_task(&payload) {
        Ok(task) => (StatusCode::CREATED, Json(serde_json::json!(task))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!(ErrorResponse { error: e })),
        )
            .into_response(),
    }
}

pub async fn update_task(
    Path(id): Path<String>,
    Json(payload): Json<UpdateTask>,
) -> impl IntoResponse {
    match commands::update_task(&id, &payload) {
        Ok(task) => (StatusCode::OK, Json(serde_json::json!(task))).into_response(),
        Err(e) => {
            let status = if e.contains("Can't get") || e.contains("doesn't understand") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(serde_json::json!(ErrorResponse { error: e }))).into_response()
        }
    }
}

pub async fn complete_task(Path(id): Path<String>) -> impl IntoResponse {
    match commands::complete_task(&id) {
        Ok(task) => (StatusCode::OK, Json(serde_json::json!(task))).into_response(),
        Err(e) => {
            let status = if e.contains("Can't get") || e.contains("doesn't understand") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(serde_json::json!(ErrorResponse { error: e }))).into_response()
        }
    }
}

pub async fn delete_task(Path(id): Path<String>) -> impl IntoResponse {
    match commands::delete_task(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let status = if e.contains("Can't get") || e.contains("doesn't understand") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, Json(serde_json::json!(ErrorResponse { error: e }))).into_response()
        }
    }
}
