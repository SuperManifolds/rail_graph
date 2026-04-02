use petgraph::stable_graph::NodeIndex;
use std::collections::HashMap;

/// 2D vector for position calculations
#[derive(Debug, Clone, Copy, Default)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    #[must_use]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    #[must_use]
    pub fn to_tuple(self) -> (f64, f64) {
        (self.x, self.y)
    }

    #[must_use]
    pub fn dist(&self, other: Vec2) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f64) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl std::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

/// Layout scenario based on available data
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutScenario {
    /// All nodes have geographic data, none have positions - fresh import
    FreshImport,

    /// All nodes have positions, none have geographic data - manual network
    ManualNetwork,

    /// Mix of positioned and unpositioned nodes - incremental import
    Incremental,

    /// Most nodes have both positions and geo data - re-layout or matched import
    MixedMatched,
}

/// Configuration for layout algorithm
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Base spacing between adjacent nodes (grid-aligned)
    pub base_spacing: f64,
    /// Canvas width
    pub canvas_width: f64,
    /// Canvas height
    pub canvas_height: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        use super::constants::GRID_SIZE;
        Self {
            base_spacing: GRID_SIZE * 3.0,
            canvas_width: 2000.0,
            canvas_height: 1000.0,
        }
    }
}

/// Full state for layout computation
pub struct LayoutState {
    /// Current node positions
    pub positions: HashMap<NodeIndex, Vec2>,
}
