use crate::models::{Line, SegmentState};
use leptos::*;
use wasm_bindgen::prelude::*;

const LINES_STORAGE_KEY: &str = "nimby_graph_lines";
const SEGMENT_STATE_STORAGE_KEY: &str = "nimby_graph_segment_state";

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = localStorage)]
    fn getItem(key: &str) -> Option<String>;

    #[wasm_bindgen(js_namespace = localStorage)]
    fn setItem(key: &str, value: &str);

    #[wasm_bindgen(js_namespace = localStorage)]
    fn removeItem(key: &str);
}

pub fn save_lines_to_storage(lines: &[Line]) -> Result<(), String> {
    match serde_json::to_string(lines) {
        Ok(json) => {
            setItem(LINES_STORAGE_KEY, &json);
            Ok(())
        }
        Err(e) => Err(format!("Failed to serialize lines: {}", e))
    }
}

pub fn load_lines_from_storage() -> Result<Vec<Line>, String> {
    match getItem(LINES_STORAGE_KEY) {
        Some(json) => {
            match serde_json::from_str(&json) {
                Ok(lines) => Ok(lines),
                Err(e) => Err(format!("Failed to parse stored lines: {}", e))
            }
        }
        None => Err("No saved configuration found".to_string())
    }
}

pub fn clear_lines_storage() {
    removeItem(LINES_STORAGE_KEY);
}

pub fn save_segment_state_to_storage(segment_state: &SegmentState) -> Result<(), String> {
    match serde_json::to_string(segment_state) {
        Ok(json) => {
            setItem(SEGMENT_STATE_STORAGE_KEY, &json);
            Ok(())
        }
        Err(e) => Err(format!("Failed to serialize segment state: {}", e))
    }
}

pub fn load_segment_state_from_storage() -> Result<SegmentState, String> {
    match getItem(SEGMENT_STATE_STORAGE_KEY) {
        Some(json) => {
            match serde_json::from_str(&json) {
                Ok(segment_state) => Ok(segment_state),
                Err(e) => Err(format!("Failed to parse stored segment state: {}", e))
            }
        }
        None => Ok(SegmentState { double_tracked_segments: std::collections::HashSet::new() })
    }
}

pub fn clear_segment_state_storage() {
    removeItem(SEGMENT_STATE_STORAGE_KEY);
}