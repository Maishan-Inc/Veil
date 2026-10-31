use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupRecord {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct CreateGroup {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateGroup {
    pub name: String,
    pub color: Option<String>,
}
