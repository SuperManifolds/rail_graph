mod file;

pub use file::{serialize_project_to_bytes, deserialize_project_from_bytes, create_export_filename, trigger_download, regenerate_project_ids};

/// Current project file format version
pub const CURRENT_PROJECT_VERSION: u32 = 1;

const GB: f64 = 1_073_741_824.0;
const MB: f64 = 1_048_576.0;
const KB: f64 = 1_024.0;

/// Format bytes into a human-readable string with appropriate units
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let bytes_f = bytes as f64;

    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{bytes} B")
    }
}
