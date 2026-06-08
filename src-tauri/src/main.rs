mod commands;

pub struct AppState {
    pub project_cache: std::sync::Mutex<Option<Vec<u8>>>,
}

fn main() {
    env_logger::init();

    tauri::Builder::default()
        .manage(AppState {
            project_cache: std::sync::Mutex::new(None),
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::save_project,
            commands::load_project,
            commands::delete_project,
            commands::list_projects,
            commands::get_current_project_id,
            commands::set_current_project_id,
            commands::detect_conflicts,
            commands::compute_auto_layout,
            commands::cache_project_state,
            commands::get_cached_project_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
