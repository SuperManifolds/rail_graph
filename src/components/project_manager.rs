use leptos::{component, view, IntoView, Signal, create_signal, SignalGet, SignalSet, spawn_local, Callback, Callable, WriteSignal, create_effect, wasm_bindgen, create_node_ref};
use crate::components::window::Window;
use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::components::text_input_dialog::TextInputDialog;
use crate::models::{Project, ProjectMetadata};
use crate::storage::{self, Storage, IndexedDbStorage, format_bytes};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

fn load_project_action(
    project_id: String,
    storage: IndexedDbStorage,
    on_load_project: Callback<Project>,
    on_close: Rc<impl Fn() + 'static>,
    set_error: WriteSignal<Option<String>>,
) {
    spawn_local(async move {
        match storage.load_project(&project_id).await {
            Ok(project) => {
                on_load_project.call(project);
                on_close();
            }
            Err(e) => set_error.set(Some(format!("Failed to load project: {e}"))),
        }
    });
}

fn duplicate_project_action(
    project_id: String,
    storage: IndexedDbStorage,
    load_projects: impl Fn() + 'static,
    set_error: WriteSignal<Option<String>>,
) {
    spawn_local(async move {
        match storage.load_project(&project_id).await {
            Ok(project) => {
                let new_name = format!("{} (Copy)", project.metadata.name);
                let duplicated = project.duplicate_with_name(new_name);
                match storage.save_project(&duplicated).await {
                    Ok(()) => load_projects(),
                    Err(e) => set_error.set(Some(format!("Failed to duplicate project: {e}"))),
                }
            }
            Err(e) => set_error.set(Some(format!("Failed to load project for duplication: {e}"))),
        }
    });
}

fn export_project_action(
    project_id: String,
    project_name: String,
    storage_backend: IndexedDbStorage,
    set_error: WriteSignal<Option<String>>,
) {
    spawn_local(async move {
        let project = match storage_backend.load_project(&project_id).await {
            Ok(p) => p,
            Err(e) => {
                set_error.set(Some(format!("Failed to load project for export: {e}")));
                return;
            }
        };

        let bytes = match storage::serialize_project_to_bytes(&project) {
            Ok(b) => b,
            Err(e) => {
                set_error.set(Some(e));
                return;
            }
        };

        let filename = storage::create_export_filename(&project_name);

        if let Err(e) = storage::trigger_download(&bytes, &filename) {
            set_error.set(Some(e));
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn render_project_row(
    metadata: ProjectMetadata,
    current_project_id: String,
    storage: IndexedDbStorage,
    on_load_project: Callback<Project>,
    on_close: Rc<impl Fn() + 'static>,
    load_projects: impl Fn() + 'static + Clone,
    set_error_message: WriteSignal<Option<String>>,
    set_delete_target_id: WriteSignal<Option<String>>,
    set_delete_target_name: WriteSignal<String>,
    set_show_delete_confirm: WriteSignal<bool>,
) -> impl IntoView {
    let is_active = metadata.id == current_project_id;
    let project_id = Rc::new(metadata.id.clone());
    let project_name = Rc::new(metadata.name.clone());
    let project_id_for_dup = Rc::new(metadata.id.clone());

    let date_str = crate::time::format_rfc3339_local(&metadata.updated_at);

    let row_class = if is_active {
        "project-list-row active"
    } else {
        "project-list-row"
    };

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
                    on:click={
                        let on_close = Rc::clone(&on_close);
                        let project_id = Rc::clone(&project_id);
                        move |_| {
                            load_project_action(
                                (*project_id).clone(),
                                storage,
                                on_load_project,
                                Rc::clone(&on_close),
                                set_error_message,
                            );
                        }
                    }
                    title="Load project"
                    prop:disabled=is_active
                >
                    <i class="fa-solid fa-folder-open"></i>
                </button>
                <button
                    class="action-button"
                    on:click={
                        let load_projects = load_projects.clone();
                        let project_id_for_dup = Rc::clone(&project_id_for_dup);
                        move |_| {
                            duplicate_project_action(
                                (*project_id_for_dup).clone(),
                                storage,
                                load_projects.clone(),
                                set_error_message,
                            );
                        }
                    }
                    title="Duplicate project"
                >
                    <i class="fa-solid fa-copy"></i>
                </button>
                <button
                    class="action-button"
                    on:click={
                        let project_id = Rc::clone(&project_id);
                        let project_name = Rc::clone(&project_name);
                        move |_| {
                            export_project_action(
                                (*project_id).clone(),
                                (*project_name).clone(),
                                storage,
                                set_error_message,
                            );
                        }
                    }
                    title="Export project"
                >
                    <i class="fa-solid fa-download"></i>
                </button>
                <button
                    class="action-button danger"
                    on:click={
                        let project_id = Rc::clone(&project_id);
                        let project_name = Rc::clone(&project_name);
                        move |_| {
                            set_delete_target_id.set(Some((*project_id).clone()));
                            set_delete_target_name.set((*project_name).clone());
                            set_show_delete_confirm.set(true);
                        }
                    }
                    title={if is_active { "Cannot delete active project" } else { "Delete project" }}
                    prop:disabled=is_active
                >
                    <i class="fa-solid fa-trash"></i>
                </button>
            </div>
        </div>
    }
}

#[component]
#[allow(clippy::too_many_lines)]
pub fn ProjectManager(
    is_open: Signal<bool>,
    on_close: impl Fn() + 'static + Clone,
    on_load_project: Callback<Project>,
    current_project: Signal<Project>,
) -> impl IntoView {
    let storage = IndexedDbStorage;
    let on_close = Rc::new(on_close);

    let (projects, set_projects) = create_signal(Vec::<ProjectMetadata>::new());
    let (error_message, set_error_message) = create_signal(None::<String>);
    let (storage_warning, set_storage_warning) = create_signal(None::<String>);
    let (storage_quota, set_storage_quota) = create_signal(None::<(u64, u64)>);

    // Confirmation dialog state
    let (show_delete_confirm, set_show_delete_confirm) = create_signal(false);
    let (delete_target_id, set_delete_target_id) = create_signal(None::<String>);
    let (delete_target_name, set_delete_target_name) = create_signal(String::new());

    // Save As dialog state
    let (show_save_as_dialog, set_show_save_as_dialog) = create_signal(false);
    let (save_as_name, set_save_as_name) = create_signal(String::new());

    // Save As overwrite confirmation state
    let (show_save_as_overwrite_confirm, set_show_save_as_overwrite_confirm) = create_signal(false);
    let (save_as_overwrite_target_id, set_save_as_overwrite_target_id) = create_signal(String::new());
    let (save_as_overwrite_target_name, set_save_as_overwrite_target_name) = create_signal(String::new());

    // New Project dialog state
    let (show_new_project_dialog, set_show_new_project_dialog) = create_signal(false);
    let (new_project_name, set_new_project_name) = create_signal(String::new());

    // Import file input
    let import_file_input_ref = create_node_ref::<leptos::html::Input>();

    // Load projects when dialog opens
    let load_projects = move || {
        spawn_local(async move {
            match storage.list_projects().await {
                Ok(loaded) => set_projects.set(loaded),
                Err(e) => set_error_message.set(Some(format!("Failed to load projects: {e}"))),
            }
        });
    };

    // Check storage quota
    let check_storage_quota = move || {
        spawn_local(async move {
            if let Ok(Some((used, total))) = storage.get_storage_quota().await {
                set_storage_quota.set(Some((used, total)));
                #[allow(clippy::cast_precision_loss)]
                let usage_percent = (used as f64 / total as f64) * 100.0;
                if usage_percent > 90.0 {
                    let used_str = format_bytes(used);
                    let total_str = format_bytes(total);
                    set_storage_warning.set(Some(format!(
                        "Storage critically low: {used_str} / {total_str} ({usage_percent:.0}% used)"
                    )));
                } else if usage_percent > 75.0 {
                    let used_str = format_bytes(used);
                    let total_str = format_bytes(total);
                    set_storage_warning.set(Some(format!(
                        "Storage usage warning: {used_str} / {total_str} ({usage_percent:.0}% used)"
                    )));
                } else {
                    set_storage_warning.set(None);
                }
            } else {
                set_storage_quota.set(None);
                set_storage_warning.set(None);
            }
        });
    };

    // Auto-load projects and check quota when dialog opens
    create_effect(move |_| {
        if is_open.get() {
            load_projects();
            check_storage_quota();
        }
    });


    // Perform the actual save-as operation
    let perform_save_as = Rc::new(move |existing_project_id: Option<String>| {
        let name = save_as_name.get().trim().to_string();

        spawn_local(async move {
            let mut project = current_project.get();
            project.metadata.name = name;

            // Use existing ID if overwriting, otherwise create new ID
            if let Some(id) = existing_project_id {
                project.metadata.id = id;
                project.metadata.updated_at = chrono::Utc::now().to_rfc3339();
            } else {
                project.metadata.id = uuid::Uuid::new_v4().to_string();
                project.metadata.created_at = chrono::Utc::now().to_rfc3339();
                project.metadata.updated_at.clone_from(&project.metadata.created_at);
            }

            let project_id = project.metadata.id.clone();

            match storage.save_project(&project).await {
                Ok(()) => {
                    // Switch to the newly saved project
                    on_load_project.call(project);

                    if let Err(e) = storage.set_current_project_id(&project_id).await {
                        set_error_message.set(Some(format!("Failed to set current project: {e}")));
                        return;
                    }

                    set_show_save_as_dialog.set(false);
                    set_save_as_name.set(String::new());
                    load_projects();
                }
                Err(e) => set_error_message.set(Some(format!("Failed to save project: {e}"))),
            }
        });
    });

    // Save current project with new name - check for duplicates first
    let perform_save_as_clone = perform_save_as.clone();
    let handle_save_as = Rc::new(move || {
        let name = save_as_name.get().trim().to_string();
        if name.is_empty() {
            set_error_message.set(Some("Project name cannot be empty".to_string()));
            return;
        }

        // Check if a project with this name already exists
        let existing = projects.get().iter().find(|p| p.name == name).cloned();

        if let Some(existing_project) = existing {
            // Show confirmation dialog
            set_save_as_overwrite_target_id.set(existing_project.id);
            set_save_as_overwrite_target_name.set(existing_project.name);
            set_show_save_as_overwrite_confirm.set(true);
        } else {
            // No conflict, proceed with new project
            perform_save_as_clone(None);
        }
    });

    let cancel_save_as = Rc::new(move || {
        set_show_save_as_dialog.set(false);
        set_save_as_name.set(String::new());
    });

    // Confirm overwrite in save-as
    let confirm_save_as_overwrite = Rc::new(move || {
        let existing_id = save_as_overwrite_target_id.get();
        set_show_save_as_overwrite_confirm.set(false);
        perform_save_as(Some(existing_id));
    });

    // Cancel overwrite in save-as
    let cancel_save_as_overwrite = Rc::new(move || {
        set_show_save_as_overwrite_confirm.set(false);
    });

    // Load a project - inline in the view to avoid move issues
    // Duplicate a project - inline in the view

    // Confirm delete
    let confirm_delete = Rc::new(move || {
        if let Some(id) = delete_target_id.get() {
            spawn_local(async move {
                match storage.delete_project(&id).await {
                    Ok(()) => {
                        set_show_delete_confirm.set(false);
                        load_projects();
                    }
                    Err(e) => set_error_message.set(Some(format!("Failed to delete project: {e}"))),
                }
            });
        }
    });

    // Cancel delete
    let cancel_delete = Rc::new(move || {
        set_show_delete_confirm.set(false);
        set_delete_target_id.set(None);
    });

    // New project
    let handle_new_project = Rc::new({
        let on_close = Rc::clone(&on_close);
        move || {
            let name = new_project_name.get().trim().to_string();
            if name.is_empty() {
                set_error_message.set(Some("Project name cannot be empty".to_string()));
                return;
            }

            let project = Project::new_with_name(name);
            on_load_project.call(project);
            set_show_new_project_dialog.set(false);
            set_new_project_name.set(String::new());
            on_close();
        }
    });

    let cancel_new_project = Rc::new(move || {
        set_show_new_project_dialog.set(false);
        set_new_project_name.set(String::new());
    });

    // Process imported project file
    let process_import = move |bytes: Vec<u8>| {
        let project = match storage::deserialize_project_from_bytes(&bytes) {
            Ok(p) => p,
            Err(e) => {
                set_error_message.set(Some(e));
                return;
            }
        };

        let project = storage::regenerate_project_ids(project);

        spawn_local(async move {
            if let Err(e) = storage.save_project(&project).await {
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
        let Some(input_elem) = import_file_input_ref.get() else { return };
        let input: &web_sys::HtmlInputElement = &input_elem;
        let Some(files) = input.files() else { return };
        let Some(file) = files.get(0) else { return };

        spawn_local(async move {
            let Ok(reader) = web_sys::FileReader::new() else {
                set_error_message.set(Some("Failed to create FileReader".to_string()));
                return;
            };
            let reader_clone = reader.clone();

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

                process_import(bytes);
            }) as Box<dyn FnMut(_)>);

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            let _ = reader.read_as_array_buffer(&file);
        });
    };

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Projects".to_string())
            on_close={
                let on_close = Rc::clone(&on_close);
                move || {
                    on_close();
                    set_error_message.set(None);
                }
            }
            max_size=(800.0, 600.0)
        >
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
                            "×"
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
                            "×"
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
                    <button
                        on:click=move |_| {
                            set_save_as_name.set(current_project.get().metadata.name);
                            set_show_save_as_dialog.set(true);
                        }
                    >
                        <i class="fa-solid fa-save"></i>
                        " Save As..."
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
                        let current_id = current_project.get().metadata.id;
                        if project_list.is_empty() {
                            view! {
                                <div class="project-list-empty">
                                    "No saved projects. Click 'Save As...' to create one."
                                </div>
                            }.into_view()
                        } else {
                            project_list.into_iter().map(|project| {
                                render_project_row(
                                    project,
                                    current_id.clone(),
                                    storage,
                                    on_load_project,
                                    Rc::clone(&on_close),
                                    load_projects,
                                    set_error_message,
                                    set_delete_target_id,
                                    set_delete_target_name,
                                    set_show_delete_confirm,
                                )
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
        </Window>

        // New Project Dialog
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

        // Save As Dialog
        <TextInputDialog
            is_open=show_save_as_dialog.into()
            title=Signal::derive(|| "Save Project As".to_string())
            label="Project Name:".to_string()
            value=save_as_name
            set_value=set_save_as_name
            on_confirm=handle_save_as
            on_cancel=cancel_save_as
            confirm_text="Save".to_string()
            cancel_text="Cancel".to_string()
        />

        // Save As Overwrite Confirmation Dialog
        <ConfirmationDialog
            is_open=show_save_as_overwrite_confirm.into()
            title=Signal::derive(|| "Overwrite Project?".to_string())
            message=Signal::derive(move || format!("A project named '{}' already exists. Do you want to overwrite it?", save_as_overwrite_target_name.get()))
            on_confirm=confirm_save_as_overwrite
            on_cancel=cancel_save_as_overwrite
            confirm_text="Overwrite".to_string()
            cancel_text="Cancel".to_string()
        />

        // Delete Confirmation Dialog
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
