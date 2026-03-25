use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChecklistItem {
    pub title: String,
    pub completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub area: Option<String>,
    pub tags: Vec<String>,
    pub completed: bool,
    pub canceled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tag {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Area {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TasksQuery {
    pub list: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTask {
    pub title: String,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub list: Option<String>,
    pub tags: Option<Vec<String>>,
    pub project: Option<String>,
    pub checklist_items: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub list: Option<String>,
    pub tags: Option<Vec<String>>,
    pub project: Option<String>,
    pub completed: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
