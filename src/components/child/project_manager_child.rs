use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::components::text_input_dialog::TextInputDialog;
use crate::models::{Project, ProjectMetadata};
use crate::{storage, storage::format_bytes, tauri_bridge};
use crate::window_protocol::{ProjectManagerInit, ProjectManagerResult};
use leptos::{
    component, create_effect, create_node_ref, create_signal, view, wasm_bindgen, IntoView,
    Signal, SignalGet, SignalSet,
};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

fn emit_result(session: &str, result: &ProjectManagerResult) {
    let json = serde_json::to_string(result).unwrap_or_default();
    let event_name = format!("result:{session}");
    leptos::spawn_local(async move {
        let _ = tauri_bridge::emit_event(&event_name, &json).await;
    });
}

#[component]
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn ProjectManagerChild(init: ProjectManagerInit, session: String) -> impl IntoView {
    let current_project_id = init.current_project_id;

    let (projects, set_projects) = create_signal(Vec::<ProjectMetadata>::new());
    let (error_message, set_error_message) = create_signal(None::<String>);
    let (storage_warning, set_storage_warning) = create_signal(None::<String>);
    let (storage_quota, set_storage_quota) = create_signal(None::<(u64, u64)>);

    // Confirmation dialog state
    let (show_delete_confirm, set_show_delete_confirm) = create_signal(false);
    let (delete_target_id, set_delete_target_id) = create_signal(None::<String>);
    let (delete_target_name, set_delete_target_name) = create_signal(String::new());

    // New Project dialog state
    let (show_new_project_dialog, set_show_new_project_dialog) = create_signal(false);
    let (new_project_name, set_new_project_name) = create_signal(String::new());

    // Import file input
    let import_file_input_ref = create_node_ref::<leptos::html::Input>();

    // Load projects
    let load_projects = move || {
        leptos::spawn_local(async move {
            match tauri_bridge::list_projects().await {
                Ok(loaded) => set_projects.set(loaded),
                Err(e) => set_error_message.set(Some(format!("Failed to load projects: {e}"))),
            }
        });
    };

    // Auto-load on mount
    create_effect(move |ran_once: Option<bool>| {
        if ran_once.is_none() {
            load_projects();
            set_storage_quota.set(None);
            set_storage_warning.set(None);
        }
        true
    });

    // Load project action
    let session_for_load = session.clone();
    let load_project_action = Rc::new(move |project_id: String| {
        let session = session_for_load.clone();
        leptos::spawn_local(async move {
            match tauri_bridge::load_project(&project_id).await {
                Ok(bytes) => {
                    emit_result(&session, &ProjectManagerResult::LoadProject(bytes));
                }
                Err(e) => set_error_message.set(Some(format!("Failed to load project: {e}"))),
            }
        });
    });

    // Duplicate project action
    let duplicate_project_action = move |project_id: String| {
        leptos::spawn_local(async move {
            match tauri_bridge::load_project(&project_id)
                .await
                .and_then(|bytes| Project::from_bytes(&bytes))
            {
                Ok(project) => {
                    let new_name = format!("{} (Copy)", project.metadata.name);
                    let duplicated = project.duplicate_with_name(new_name);
                    match duplicated
                        .serialize_to_bytes()
                        .map(|bytes| (bytes, duplicated.metadata.id.clone()))
                    {
                        Ok((bytes, id)) => match tauri_bridge::save_project(&bytes, &id).await {
                            Ok(()) => load_projects(),
                            Err(e) => {
                                set_error_message
                                    .set(Some(format!("Failed to duplicate project: {e}")));
                            }
                        },
                        Err(e) => {
                            set_error_message
                                .set(Some(format!("Failed to serialize project: {e}")));
                        }
                    }
                }
                Err(e) => {
                    set_error_message
                        .set(Some(format!("Failed to load project for duplication: {e}")));
                }
            }
        });
    };

    // Export project action
    let export_project_action = move |project_id: String, project_name: String| {
        leptos::spawn_local(async move {
            let project = match tauri_bridge::load_project(&project_id)
                .await
                .and_then(|bytes| Project::from_bytes(&bytes))
            {
                Ok(p) => p,
                Err(e) => {
                    set_error_message.set(Some(format!("Failed to load project for export: {e}")));
                    return;
                }
            };

            let bytes = match storage::serialize_project_to_bytes(&project) {
                Ok(b) => b,
                Err(e) => {
                    set_error_message.set(Some(e));
                    return;
                }
            };

            let filename = storage::create_export_filename(&project_name);
            if let Err(e) = storage::trigger_download(&bytes, &filename) {
                set_error_message.set(Some(e));
            }
        });
    };

    // Confirm delete
    let confirm_delete = Rc::new(move || {
        if let Some(id) = delete_target_id.get() {
            leptos::spawn_local(async move {
                match tauri_bridge::delete_project(&id).await {
                    Ok(()) => {
                        set_show_delete_confirm.set(false);
                        load_projects();
                    }
                    Err(e) => {
                        set_error_message.set(Some(format!("Failed to delete project: {e}")));
                    }
                }
            });
        }
    });

    let cancel_delete = Rc::new(move || {
        set_show_delete_confirm.set(false);
        set_delete_target_id.set(None);
    });

    // New project
    let session_for_new = session.clone();
    let handle_new_project = Rc::new(move || {
        let name = new_project_name.get().trim().to_string();
        if name.is_empty() {
            set_error_message.set(Some("Project name cannot be empty".to_string()));
            return;
        }

        let project = Project::new_with_name(name);
        match project.serialize_to_bytes() {
            Ok(bytes) => {
                emit_result(
                    &session_for_new,
                    &ProjectManagerResult::CreateProject(bytes),
                );
            }
            Err(e) => {
                set_error_message.set(Some(format!("Failed to serialize new project: {e}")));
            }
        }
        set_show_new_project_dialog.set(false);
        set_new_project_name.set(String::new());
    });

    let cancel_new_project = Rc::new(move || {
        set_show_new_project_dialog.set(false);
        set_new_project_name.set(String::new());
    });

    // Process imported project file
    let process_import = move |bytes: Vec<u8>, filename: Option<String>| {
        let project = match storage::deserialize_project_from_bytes(&bytes) {
            Ok(p) => p,
            Err(e) => {
                set_error_message.set(Some(e));
                return;
            }
        };

        let project = storage::regenerate_project_ids(project, filename);

        let save_bytes = match project.serialize_to_bytes() {
            Ok(b) => b,
            Err(e) => {
                set_error_message.set(Some(format!(
                    "Failed to serialize imported project: {e}"
                )));
                return;
            }
        };
        let project_id = project.metadata.id.clone();

        leptos::spawn_local(async move {
            if let Err(e) = tauri_bridge::save_project(&save_bytes, &project_id).await {
                set_error_message.set(Some(format!("Failed to save imported project: {e}")));
                return;
            }

            load_projects();
            if let Some(input) = import_file_input_ref.get() {
                input.set_value("");
            }
        });
    };

    // Import project
    let handle_import_file = move |_| {
        let Some(input_elem) = import_file_input_ref.get() else {
            return;
        };
        let input: &web_sys::HtmlInputElement = &input_elem;
        let Some(files) = input.files() else {
            return;
        };
        let Some(file) = files.get(0) else {
            return;
        };

        let filename_without_ext = std::path::Path::new(&file.name())
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from);

        leptos::spawn_local(async move {
            let Ok(reader) = web_sys::FileReader::new() else {
                set_error_message.set(Some("Failed to create FileReader".to_string()));
                return;
            };
            let reader_clone = reader.clone();
            let filename_clone = filename_without_ext.clone();

            let onload = Closure::wrap(Box::new(move |_: web_sys::Event| {
                let Ok(result) = reader_clone.result() else {
                    set_error_message.set(Some("Failed to read file".to_string()));
                    return;
                };

                let Ok(array_buffer) = result.dyn_into::<js_sys::ArrayBuffer>() else {
                    set_error_message.set(Some("Invalid file format".to_string()));
                    return;
                };

                let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                let bytes = uint8_array.to_vec();

                process_import(bytes, filename_clone.clone());
            }) as Box<dyn FnMut(_)>);

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            let _ = reader.read_as_array_buffer(&file);
        });
    };

    let current_project_id_for_rows = current_project_id.clone();
    let load_project_for_rows = load_project_action.clone();

    view! {
        <div class="project-manager">
            {move || error_message.get().map(|msg| view! {
                <div class="error-banner">
                    <i class="fa-solid fa-exclamation-triangle"></i>
                    " "
                    {msg}
                    <button
                        class="error-close"
                        on:click=move |_| set_error_message.set(None)
                    >
                        "x"
                    </button>
                </div>
            })}

            {move || storage_warning.get().map(|msg| view! {
                <div class="storage-warning-banner">
                    <i class="fa-solid fa-database"></i>
                    " "
                    {msg}
                    <button
                        class="error-close"
                        on:click=move |_| set_storage_warning.set(None)
                    >
                        "x"
                    </button>
                </div>
            })}

            <div class="project-manager-actions">
                <button
                    class="primary"
                    on:click=move |_| set_show_new_project_dialog.set(true)
                >
                    <i class="fa-solid fa-file"></i>
                    " New Project"
                </button>
                <input
                    type="file"
                    accept=".rgproject"
                    node_ref=import_file_input_ref
                    on:change=handle_import_file
                    style="display: none;"
                />
                <button
                    on:click=move |_| {
                        if let Some(input) = import_file_input_ref.get() {
                            input.click();
                        }
                    }
                >
                    <i class="fa-solid fa-upload"></i>
                    " Import Project"
                </button>
            </div>

            <div class="project-list">
                <div class="project-list-header">
                    <div class="project-list-col-name">"Name"</div>
                    <div class="project-list-col-date">"Last Modified"</div>
                    <div class="project-list-col-actions">"Actions"</div>
                </div>
                {move || {
                    let project_list = projects.get();
                    let current_id = current_project_id_for_rows.clone();
                    let load_action = load_project_for_rows.clone();
                    if project_list.is_empty() {
                        view! {
                            <div class="project-list-empty">
                                "No saved projects."
                            </div>
                        }.into_view()
                    } else {
                        project_list.into_iter().map(|metadata| {
                            let is_active = metadata.id == current_id;
                            let project_id = Rc::new(metadata.id.clone());
                            let project_name = Rc::new(metadata.name.clone());

                            let date_str = crate::time_ext::format_rfc3339_local(&metadata.updated_at);

                            let row_class = if is_active {
                                "project-list-row active"
                            } else {
                                "project-list-row"
                            };

                            let project_id_for_load = Rc::clone(&project_id);
                            let load_action_for_row = load_action.clone();
                            let project_id_for_dup = Rc::clone(&project_id);
                            let project_id_for_export = Rc::clone(&project_id);
                            let project_name_for_export = Rc::clone(&project_name);
                            let project_id_for_delete = Rc::clone(&project_id);
                            let project_name_for_delete = Rc::clone(&project_name);

                            view! {
                                <div class=row_class>
                                    <div class="project-list-col-name">
                                        <span class="project-name">
                                            {(*project_name).clone()}
                                        </span>
                                    </div>
                                    <div class="project-list-col-date">{date_str}</div>
                                    <div class="project-list-col-actions">
                                        <button
                                            class="action-button"
                                            on:click=move |_| {
                                                load_action_for_row((*project_id_for_load).clone());
                                            }
                                            title="Load project"
                                            prop:disabled=is_active
                                        >
                                            <i class="fa-solid fa-folder-open"></i>
                                        </button>
                                        <button
                                            class="action-button"
                                            on:click=move |_| {
                                                duplicate_project_action((*project_id_for_dup).clone());
                                            }
                                            title="Duplicate project"
                                        >
                                            <i class="fa-solid fa-copy"></i>
                                        </button>
                                        <button
                                            class="action-button"
                                            on:click=move |_| {
                                                export_project_action(
                                                    (*project_id_for_export).clone(),
                                                    (*project_name_for_export).clone(),
                                                );
                                            }
                                            title="Export project"
                                        >
                                            <i class="fa-solid fa-download"></i>
                                        </button>
                                        <button
                                            class="action-button danger"
                                            on:click=move |_| {
                                                set_delete_target_id.set(Some((*project_id_for_delete).clone()));
                                                set_delete_target_name.set((*project_name_for_delete).clone());
                                                set_show_delete_confirm.set(true);
                                            }
                                            title={if is_active { "Cannot delete active project" } else { "Delete project" }}
                                            prop:disabled=is_active
                                        >
                                            <i class="fa-solid fa-trash"></i>
                                        </button>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>().into_view()
                    }
                }}
            </div>

            {move || storage_quota.get().map(|(used, total)| {
                #[allow(clippy::cast_precision_loss)]
                let usage_percent = (used as f64 / total as f64) * 100.0;
                let used_str = format_bytes(used);
                let total_str = format_bytes(total);

                view! {
                    <div class="storage-meter">
                        <div class="storage-meter-label">
                            <span>"Storage Usage"</span>
                            <span class="storage-meter-stats">{format!("{used_str} / {total_str}")}</span>
                        </div>
                        <div class="storage-meter-bar">
                            <div
                                class="storage-meter-fill"
                                style:width=format!("{usage_percent:.1}%")
                                class:warning={usage_percent > 75.0}
                                class:critical={usage_percent > 90.0}
                            ></div>
                        </div>
                        <div class="storage-meter-percent">{format!("{usage_percent:.0}% used")}</div>
                    </div>
                }
            })}
        </div>

        // New Project Dialog (HTML overlay within child window)
        <TextInputDialog
            is_open=show_new_project_dialog.into()
            title=Signal::derive(|| "New Project".to_string())
            label="Project Name:".to_string()
            value=new_project_name
            set_value=set_new_project_name
            on_confirm=handle_new_project
            on_cancel=cancel_new_project
            confirm_text="Create".to_string()
            cancel_text="Cancel".to_string()
        />

        // Delete Confirmation Dialog (HTML overlay within child window)
        <ConfirmationDialog
            is_open=show_delete_confirm.into()
            title=Signal::derive(|| "Delete Project".to_string())
            message=Signal::derive(move || format!("Are you sure you want to delete '{}'? This action cannot be undone.", delete_target_name.get()))
            on_confirm=confirm_delete
            on_cancel=cancel_delete
            confirm_text="Delete".to_string()
            cancel_text="Cancel".to_string()
        />
    }
}
