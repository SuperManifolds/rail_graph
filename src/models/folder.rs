use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineFolder {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub sort_index: Option<f64>,
    pub collapsed: bool,
    pub parent_folder_id: Option<Uuid>,
}

impl LineFolder {
    #[must_use]
    pub fn new(name: String, color: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            color,
            icon: None,
            sort_index: None,
            collapsed: false,
            parent_folder_id: None,
        }
    }

    #[must_use]
    pub fn with_parent(name: String, color: String, parent_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            color,
            icon: None,
            sort_index: None,
            collapsed: false,
            parent_folder_id: Some(parent_id),
        }
    }
}
