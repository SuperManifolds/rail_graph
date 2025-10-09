use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub name: String,
}

pub fn default_platforms() -> Vec<Platform> {
    vec![
        Platform { name: "1".to_string() },
        Platform { name: "2".to_string() },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationNode {
    pub name: String,
    #[serde(default)]
    pub position: Option<(f64, f64)>,
    #[serde(default)]
    pub passing_loop: bool,
    #[serde(default = "default_platforms")]
    pub platforms: Vec<Platform>,
}
