use leptos::{component, view, IntoView, Signal, create_signal, SignalGet, SignalSet, spawn_local, event_target_value, Callback, Callable, WriteSignal, create_effect};
use crate::components::window::Window;
use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::models::{Project, ProjectMetadata};
use crate::storage::{Storage, IndexedDbStorage, format_bytes};
use std::rc::Rc;

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

    let date_str = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&metadata.updated_at) {
        dt.format("%Y-%m-%d %H:%M").to_string()
    } else {
        metadata.updated_at.clone()
    };

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


    // Save current project with new name
    let handle_save_as = move || {
        let name = save_as_name.get().trim().to_string();
        if name.is_empty() {
            set_error_message.set(Some("Project name cannot be empty".to_string()));
            return;
        }

        spawn_local(async move {
            let mut project = current_project.get();
            project.metadata.name = name;
            project.metadata.id = uuid::Uuid::new_v4().to_string();
            project.metadata.created_at = chrono::Utc::now().to_rfc3339();
            project.metadata.updated_at.clone_from(&project.metadata.created_at);

            match storage.save_project(&project).await {
                Ok(()) => {
                    set_show_save_as_dialog.set(false);
                    set_save_as_name.set(String::new());
                    load_projects();
                }
                Err(e) => set_error_message.set(Some(format!("Failed to save project: {e}"))),
            }
        });
    };

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
    let handle_new_project = {
        let on_close = Rc::clone(&on_close);
        move || {
            let project = Project::empty();
            on_load_project.call(project);
            on_close();
        }
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
                        on:click=move |_| handle_new_project()
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

        // Save As Dialog
        <Window
            is_open=show_save_as_dialog
            title=Signal::derive(|| "Save Project As".to_string())
            on_close=move || set_show_save_as_dialog.set(false)
            max_size=(400.0, 200.0)
        >
            <div class="save-as-dialog">
                <label>"Project Name:"</label>
                <input
                    type="text"
                    class="project-name-input"
                    value=save_as_name
                    on:input=move |ev| set_save_as_name.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            handle_save_as();
                        }
                    }
                    prop:autofocus=true
                />
                <div class="dialog-buttons">
                    <button on:click=move |_| set_show_save_as_dialog.set(false)>
                        "Cancel"
                    </button>
                    <button class="primary" on:click=move |_| handle_save_as()>
                        "Save"
                    </button>
                </div>
            </div>
        </Window>

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
