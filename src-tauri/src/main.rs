mod commands;

use railgraph_core::models::{Project, UndoManager};
use tauri::Manager;

pub struct AppState {
    pub project: std::sync::Mutex<Project>,
    pub undo_manager: std::sync::Mutex<UndoManager>,
}

fn load_initial_project(app: &tauri::AppHandle) -> Project {
    let project_id = commands::get_current_project_id_internal(app);
    if let Some(id) = project_id {
        let Ok(dir) = commands::projects_dir_internal(app) else {
            return Project::empty();
        };
        let path = dir.join(format!("{id}.rgproject"));
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(project) = Project::from_bytes(&bytes) {
                log::info!("Loaded project: {}", project.metadata.name);
                return project;
            }
        }
    }
    log::info!("No saved project found, creating empty project");
    Project::empty()
}

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let project = load_initial_project(app.handle());
            app.manage(AppState {
                project: std::sync::Mutex::new(project),
                undo_manager: std::sync::Mutex::new(UndoManager::default()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::save_project,
            commands::load_project,
            commands::delete_project,
            commands::list_projects,
            commands::get_current_project_id,
            commands::set_current_project_id,
            commands::detect_conflicts,
            commands::compute_auto_layout,
            commands::update_field,
            commands::load_project_state,
            commands::replace_project,
            commands::undo,
            commands::redo,
            commands::save_window_metadata,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
