mod commands;

fn main() {
    env_logger::init();

    tauri::Builder::default()
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
