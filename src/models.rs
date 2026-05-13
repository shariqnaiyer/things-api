use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub list: Option<String>,
    pub project: Option<String>,
    pub area: Option<String>,
    pub tags: Vec<String>,
    pub checklist_items: Vec<ChecklistItem>,
    pub completed: bool,
    pub canceled: bool,
    pub creation_date: Option<String>,
    pub completion_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ChecklistItem {
    pub title: String,
    pub completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub area: Option<String>,
    pub tags: Vec<String>,
    pub completed: bool,
    pub canceled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Tag {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Area {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TasksQuery {
    /// Which Things 3 list to read from. One of `inbox`, `today`, `upcoming`, `anytime`, `someday`, `logbook`, `trash`. Defaults to `inbox`.
    pub list: Option<String>,
    /// Maximum number of tasks to return.
    pub limit: Option<usize>,
    /// Number of tasks to skip from the start of the list.
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTask {
    pub title: String,
    pub notes: Option<String>,
    /// Date string parseable by AppleScript, e.g. `"March 25, 2026"`.
    pub due_date: Option<String>,
    /// One of `inbox`, `today`, `upcoming`, `anytime`, `someday`.
    pub list: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Exact project name. Takes priority over `list`.
    pub project: Option<String>,
    pub checklist_items: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub notes: Option<String>,
    /// New due date. Empty string clears it.
    pub due_date: Option<String>,
    /// Move to list: `inbox`, `today`, `upcoming`, `anytime`, `someday`.
    pub list: Option<String>,
    pub tags: Option<Vec<String>>,
    /// Move to project (by name).
    pub project: Option<String>,
    pub completed: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}
