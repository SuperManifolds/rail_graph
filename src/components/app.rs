use crate::components::alpha_disclaimer::AlphaDisclaimer;
use crate::components::changelog_popup::ChangelogPopup;
use crate::components::infrastructure_view::InfrastructureView;
use crate::components::project_manager::ProjectManager;
use crate::components::report_issue_button::ReportIssueButton;
use crate::components::time_graph::TimeGraph;
use crate::components::toast::{Toast, ToastNotification};
use crate::conflict::Conflict;
#[allow(unused_imports)]
use crate::logging::log;
use crate::models::{GraphView, Legend, Project, RailwayGraph, Routes, ViewportState, UndoManager, UndoSnapshot};
use crate::storage::{IndexedDbStorage, Storage};
use crate::train_journey::TrainJourney;
use crate::worker_bridge::ConflictDetector;
use leptos::{
    component, create_effect, create_signal, event_target_value, provide_context, spawn_local,
    store_value, view, Callback, IntoView, Show, Signal, SignalGet, SignalGetUntracked, SignalSet,
    SignalUpdate, WriteSignal,
};
use wasm_bindgen::JsCast;
use leptos_meta::{provide_meta_context, Title};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub enum AppTab {
    Infrastructure,
    GraphView(Uuid),
}

/// Restore the active tab from saved state
fn restore_active_tab(tab_id: &str, views: &[GraphView], set_active_tab: WriteSignal<AppTab>) {
    if tab_id == "infrastructure" {
        set_active_tab.set(AppTab::Infrastructure);
        return;
    }

    let Ok(uuid) = Uuid::parse_str(tab_id) else {
        return;
    };

    // Verify the view still exists
    if views.iter().any(|v| v.id == uuid) {
        set_active_tab.set(AppTab::GraphView(uuid));
    }
}

/// Update a single view based on its type and current state
fn update_view(
    view: &mut GraphView,
    infrastructure_changed: bool,
    current_lines: &[crate::models::Line],
    current_graph: &RailwayGraph,
) {
    // Line-based view: update from current line data
    if let Some(source_line_id) = view.source_line_id {
        let Some(source_line) = current_lines.iter().find(|line| line.id == source_line_id) else {
            return;
        };
        view.update_from_line(source_line, current_graph);
        return;
    }

    // Non-line, non-main-line view: recalculate edge_path from station_range when infrastructure changes
    if !infrastructure_changed || view.name == "Main Line" {
        return;
    }

    let Some((from, to)) = view.station_range else {
        return;
    };

    let Some(edge_path) = current_graph.find_path_between_nodes(from, to) else {
        return;
    };

    let edge_indices: Vec<usize> = edge_path.iter().map(|e| e.index()).collect();
    if !edge_indices.is_empty() {
        view.edge_path = Some(edge_indices);
    }
}

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn App() -> impl IntoView {
    provide_meta_context();

    // Register service worker for PWA functionality
    create_effect(move |_| {
        if let Some(window) = web_sys::window() {
            let navigator = window.navigator().service_worker();
            spawn_local(async move {
                match wasm_bindgen_futures::JsFuture::from(
                    navigator.register("/service_worker.js")
                ).await {
                    Ok(_) => {
                        log!("[PWA] Service worker registered");
                    }
                    Err(e) => {
                        web_sys::console::error_2(
                            &"[PWA] Service worker registration failed:".into(),
                            &e,
                        );
                    }
                }
            });
        }
    });

    let (active_tab, set_active_tab) = create_signal(AppTab::Infrastructure);

    // Storage implementation
    let storage = IndexedDbStorage;

    // Shared graph, lines, and views state
    let (lines, set_lines) = create_signal(Vec::new());
    let (folders, set_folders) = create_signal(Vec::new());
    let (graph, set_graph) = create_signal(RailwayGraph::new());
    let (legend, set_legend) = create_signal(Legend::default());
    let (settings, set_settings) = create_signal(crate::models::ProjectSettings::default());
    let (views, set_views) = create_signal(Vec::new());
    let (is_loading, set_is_loading) = create_signal(true);
    let (initial_load_complete, set_initial_load_complete) = create_signal(false);

    // Store viewport states separately to avoid triggering view updates
    let (viewport_states, set_viewport_states) =
        create_signal(HashMap::<Uuid, ViewportState>::new());
    let (infrastructure_viewport, set_infrastructure_viewport) =
        create_signal(ViewportState::default());

    // Compute train journeys at app level
    let (train_journeys, set_train_journeys) =
        create_signal(std::collections::HashMap::<uuid::Uuid, TrainJourney>::new());
    let (selected_day, set_selected_day) = create_signal(None::<chrono::Weekday>);

    // Project manager state
    let (show_project_manager, set_show_project_manager) = create_signal(false);
    let (current_project, set_current_project) = create_signal(Project::empty());

    // User settings (persists across projects)
    let (user_settings, set_user_settings) = create_signal(crate::models::UserSettings::default());

    // Track when we're capturing keyboard shortcuts in the editor
    let (is_capturing_shortcut, set_is_capturing_shortcut) = create_signal(false);

    // Signal for manually opening changelog from About button
    let (manual_open_changelog, set_manual_open_changelog) = create_signal(false);

    // Toast notification
    let (toast, set_toast) = create_signal(Toast::default());

    // Helper to show toast with auto-hide
    let show_toast = move |message: String| {
        set_toast.set(Toast::new(message));

        // Hide after 2 seconds
        if let Some(window) = web_sys::window() {
            let callback = wasm_bindgen::closure::Closure::once(move || {
                set_toast.update(|t| t.visible = false);
            });
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                2000,
            );
            callback.forget();
        }
    };

    // Undo/redo management
    let undo_manager = store_value(UndoManager::default());
    let (is_performing_undo_redo, set_is_performing_undo_redo) = create_signal(false);

    // Create debounced function for capturing snapshots
    let record_snapshot = store_value(leptos::leptos_dom::helpers::debounce(
        std::time::Duration::from_millis(300),
        move |snapshot: UndoSnapshot| {
            // Check flag again when the debounced callback actually fires
            // in case an undo/redo happened while we were waiting
            if is_performing_undo_redo.get_untracked() {
                return;
            }

            undo_manager.update_value(|manager| {
                manager.push_snapshot(snapshot);
            });
        },
    ));

    // Record state changes for undo with debouncing
    create_effect(move |_| {
        let current_graph = graph.get();
        let current_lines = lines.get();

        // Skip during initial load
        if !initial_load_complete.get() {
            return;
        }

        // Skip during undo/redo operations (use untracked to avoid re-running when flag changes)
        if is_performing_undo_redo.get_untracked() {
            return;
        }

        let snapshot = UndoSnapshot::new(current_graph, current_lines);
        record_snapshot.update_value(|f| f(snapshot));
    });

    // Load user settings on mount
    create_effect(move |_| {
        spawn_local(async move {
            match crate::models::UserSettings::load().await {
                Ok(settings) => {
                    set_user_settings.set(settings);
                }
                Err(e) => {
                    leptos::logging::warn!("Failed to load user settings: {}", e);
                    // Use defaults
                }
            }
        });
    });

    // Auto-load saved project on component mount
    create_effect(move |_| {
        spawn_local(async move {
            // Try to load the last used project
            let project_id = storage.get_current_project_id().await.ok().flatten();

            let project = if let Some(id) = project_id {
                match storage.load_project(&id).await {
                    Ok(p) => {
                        log!("Project loaded successfully");
                        Some(p)
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to load project: {e}").into());
                        None
                    }
                }
            } else {
                log!("No previous project found");
                None
            };

            let project = project.unwrap_or_else(|| {
                log!("Creating empty project");
                Project::empty()
            });
            let empty_graph = project.graph.clone();

            set_current_project.set(project.clone());
            set_lines.set(project.lines.clone());
            set_folders.set(project.folders.clone());
            set_graph.set(project.graph.clone());
            set_legend.set(project.legend);
            set_settings.set(project.settings);

            // Ensure we have at least one view (create default "Main Line" view)
            let mut views = project.views.clone();
            if views.is_empty() {
                views.push(GraphView::default_main_line(&empty_graph));
            }

            // Extract viewport states into separate signal
            let viewports: HashMap<Uuid, ViewportState> = views
                .iter()
                .map(|v| (v.id, v.viewport_state.clone()))
                .collect();
            set_viewport_states.set(viewports);
            set_infrastructure_viewport.set(project.infrastructure_viewport.clone());

            set_views.set(views.clone());

            // Restore active tab, or default to first view
            if let Some(tab_id) = &project.active_tab_id {
                restore_active_tab(tab_id, &views, set_active_tab);
            } else if let Some(first_view) = views.first() {
                set_active_tab.set(AppTab::GraphView(first_view.id));
            }

            set_initial_load_complete.set(true);
        });
    });

    // Regenerate "Main Line" view when infrastructure changes (after initial load)
    create_effect(move |prev_counts: Option<(usize, usize)>| {
        let current_graph = graph.get();
        let node_count = current_graph.graph.node_count();
        let edge_count = current_graph.graph.edge_count();

        // Skip during initial load
        if !initial_load_complete.get() {
            return (node_count, edge_count);
        }

        let counts_changed = prev_counts.is_some_and(|(prev_nodes, prev_edges)| {
            node_count != prev_nodes || edge_count != prev_edges
        });

        // Only regenerate if node or edge count changed (new station/junction/track added)
        if !counts_changed {
            return (node_count, edge_count);
        }

        set_views.update(|v| {
            // Find and regenerate the Main Line view
            for view in v.iter_mut() {
                if view.name != "Main Line" {
                    continue;
                }
                let regenerated = GraphView::default_main_line(&current_graph);
                // Preserve the view ID and viewport state
                view.station_range = regenerated.station_range;
                view.edge_path = regenerated.edge_path;
                break;
            }
        });

        (node_count, edge_count)
    });

    // Regenerate all views when infrastructure changes
    create_effect(move |prev_counts: Option<(usize, usize)>| {
        let current_graph = graph.get();
        let node_count = current_graph.graph.node_count();
        let edge_count = current_graph.graph.edge_count();

        // Skip during initial load
        if !initial_load_complete.get() {
            return (node_count, edge_count);
        }

        let infrastructure_changed = prev_counts.is_some_and(|(prev_nodes, prev_edges)| {
            node_count != prev_nodes || edge_count != prev_edges
        });

        if infrastructure_changed {
            let current_lines = lines.get_untracked();
            set_views.update(|views_vec| {
                for view in views_vec.iter_mut() {
                    update_view(view, infrastructure_changed, &current_lines, &current_graph);
                }
            });
        }

        (node_count, edge_count)
    });

    // Auto-save project whenever lines, folders, graph, legend, settings, views, viewport states, or active tab change
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_folders = folders.get();
        let current_graph = graph.get();
        let current_legend = legend.get();
        let current_settings = settings.get();
        let current_views = views.get();
        let current_viewports = viewport_states.get();
        let current_infrastructure_viewport = infrastructure_viewport.get();
        let current_tab = active_tab.get();
        let mut proj = current_project.get();

        if !current_lines.is_empty() || current_graph.graph.node_count() > 0 {
            // Convert active tab to string ID
            let active_tab_id = match current_tab {
                AppTab::Infrastructure => Some("infrastructure".to_string()),
                AppTab::GraphView(uuid) => Some(uuid.to_string()),
            };

            // Merge viewport states back into views for saving
            let views_with_viewports: Vec<GraphView> = current_views
                .into_iter()
                .map(|mut v| {
                    if let Some(viewport) = current_viewports.get(&v.id) {
                        v.viewport_state = viewport.clone();
                    }
                    v
                })
                .collect();

            // Update project with current data, preserving metadata
            proj.lines = current_lines;
            proj.folders = current_folders;
            proj.graph = current_graph;
            proj.legend = current_legend;
            proj.settings = current_settings;
            proj.views = views_with_viewports;
            proj.active_tab_id = active_tab_id;
            proj.infrastructure_viewport = current_infrastructure_viewport;
            proj.touch_updated_at();

            // Update current_project signal to keep it synchronized
            set_current_project.set(proj.clone());

            let project_id = proj.metadata.id.clone();
            spawn_local(async move {
                if let Err(e) = storage.save_project(&proj).await {
                    web_sys::console::error_1(&format!("Auto-save failed: {e}").into());
                    return;
                }
                if let Err(e) = storage.set_current_project_id(&project_id).await {
                    web_sys::console::error_1(
                        &format!("Failed to set current project ID: {e}").into(),
                    );
                }
            });
        }
    });

    // Mark loading complete once initial data is loaded
    create_effect(move |_| {
        if initial_load_complete.get() {
            set_is_loading.set(false);
        }
    });

    // Generate train journeys when lines or graph change
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_graph = graph.get();
        let day_filter = selected_day.get();

        // Filter to only visible lines
        let visible_lines: Vec<_> = current_lines
            .into_iter()
            .filter(|line| line.visible)
            .collect();

        // Generate journeys for the full day
        let new_journeys =
            TrainJourney::generate_journeys(&visible_lines, &current_graph, day_filter);
        set_train_journeys.set(new_journeys);
    });

    // Compute conflicts at app level using worker
    let (conflicts, set_conflicts) = create_signal(Vec::new());

    let detector = store_value(ConflictDetector::new(set_conflicts));

    // Create debounced conflict detection to avoid excessive recomputation
    let debounced_detect_conflicts = store_value(leptos::leptos_dom::helpers::debounce(
        std::time::Duration::from_millis(300),
        move |(journeys_vec, current_graph, current_settings): (Vec<TrainJourney>, RailwayGraph, crate::models::ProjectSettings)| {
            detector.update_value(|d| {
                d.detect(journeys_vec, current_graph, current_settings);
            });
        },
    ));

    create_effect(move |_| {
        let journeys = train_journeys.get();
        let journeys_vec: Vec<_> = journeys.values().cloned().collect();
        let current_graph = graph.get();
        let current_settings = settings.get();

        debounced_detect_conflicts.update_value(|f| {
            f((journeys_vec, current_graph, current_settings));
        });
    });

    let raw_conflicts: Signal<Vec<Conflict>> = conflicts.into();

    // Callback for creating a new view
    let on_create_view = Callback::new(move |new_view: GraphView| {
        let view_id = new_view.id;
        let viewport = new_view.viewport_state.clone();
        set_viewport_states.update(|vs| {
            vs.insert(view_id, viewport);
        });
        set_views.update(|v| v.push(new_view));
        set_active_tab.set(AppTab::GraphView(view_id));
    });

    // Callback for closing a view
    let on_close_view = move |view_id: Uuid| {
        // Check if we're closing the active tab
        let is_active = match active_tab.get() {
            AppTab::GraphView(id) => id == view_id,
            AppTab::Infrastructure => false,
        };

        // Remove the view and its viewport state
        set_views.update(|v| v.retain(|view| view.id != view_id));
        set_viewport_states.update(|vs| {
            vs.remove(&view_id);
        });

        // If we closed the active tab, switch to another tab
        if is_active {
            let remaining_views = views.get();
            if let Some(first_view) = remaining_views.first() {
                set_active_tab.set(AppTab::GraphView(first_view.id));
            } else {
                set_active_tab.set(AppTab::Infrastructure);
            }
        }
    };

    // State for renaming views
    let (editing_view_id, set_editing_view_id) = create_signal(None::<Uuid>);
    let (edit_name_value, set_edit_name_value) = create_signal(String::new());

    // State for drag-and-drop reordering
    let (dragged_view_id, set_dragged_view_id) = create_signal(None::<Uuid>);
    let (drag_over_view_id, set_drag_over_view_id) = create_signal(None::<Uuid>);
    let (drag_timer_id, set_drag_timer_id) = create_signal(None::<i32>);

    // Callback for renaming a view
    let on_rename_view = move |view_id: Uuid, new_name: String| {
        if !new_name.trim().is_empty() {
            set_views.update(|v| {
                if let Some(view) = v.iter_mut().find(|view| view.id == view_id) {
                    view.set_name(new_name.trim().to_string());
                }
            });
        }
        set_editing_view_id.set(None);
    };

    // Callback for updating viewport state of a view
    // Update separate viewport signal to avoid triggering view updates and re-rendering TimeGraph
    let on_viewport_change = move |view_id: Uuid, viewport_state: ViewportState| {
        set_viewport_states.update(|vs| {
            vs.insert(view_id, viewport_state);
        });
    };

    // Callback for loading a project from project manager
    let on_load_project = Callback::new(move |project: Project| {
        let project_id = project.metadata.id.clone();

        // Handle views
        let mut project_views = project.views.clone();
        if project_views.is_empty() {
            project_views.push(GraphView::default_main_line(&project.graph));
        }

        // Extract viewport states
        let viewports: HashMap<Uuid, ViewportState> = project_views
            .iter()
            .map(|v| (v.id, v.viewport_state.clone()))
            .collect();

        // Batch all signal updates to prevent auto-save from triggering with partial state
        leptos::batch(move || {
            set_current_project.set(project.clone());
            set_lines.set(project.lines.clone());
            set_folders.set(project.folders.clone());
            set_graph.set(project.graph.clone());
            set_legend.set(project.legend.clone());
            set_settings.set(project.settings.clone());
            set_viewport_states.set(viewports);
            set_infrastructure_viewport.set(project.infrastructure_viewport.clone());
            set_views.set(project_views.clone());

            // Set active tab
            if let Some(tab_id) = &project.active_tab_id {
                restore_active_tab(tab_id, &project_views, set_active_tab);
            } else if let Some(first_view) = project_views.first() {
                set_active_tab.set(AppTab::GraphView(first_view.id));
            }
        });

        // Set this as the current project
        spawn_local(async move {
            if let Err(e) = storage.set_current_project_id(&project_id).await {
                web_sys::console::error_1(&format!("Failed to set current project ID: {e}").into());
            }
        });
    });

    // Provide user settings via context
    provide_context((user_settings, set_user_settings));
    provide_context((is_capturing_shortcut, set_is_capturing_shortcut));

    // Setup tab switching keyboard shortcuts
    crate::components::tab_shortcuts::setup_tab_switching(
        is_capturing_shortcut,
        views,
        set_active_tab,
    );

    // Helper to restore snapshot state
    let restore_snapshot = move |snapshot: UndoSnapshot| {
        set_graph.set(snapshot.graph);
        set_lines.set(snapshot.lines);
    };

    // Setup undo/redo keyboard shortcuts
    leptos::leptos_dom::helpers::window_event_listener(leptos::ev::keydown, move |ev| {
        // Don't handle shortcuts when capturing in the shortcuts editor
        if is_capturing_shortcut.get() {
            return;
        }

        // Don't handle keyboard shortcuts when typing in input fields
        let Some(target) = ev.target() else { return };
        let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() else { return };
        let tag_name = element.tag_name().to_lowercase();
        if tag_name == "input" || tag_name == "textarea" {
            return;
        }

        // Ignore repeat events
        if ev.repeat() {
            return;
        }

        // Find matching action
        let current_shortcuts = user_settings.get().keyboard_shortcuts;
        let action = current_shortcuts.find_action(
            &ev.code(),
            ev.ctrl_key(),
            ev.shift_key(),
            ev.alt_key(),
            ev.meta_key(),
        );

        match action {
            Some("undo") => {
                ev.prevent_default();

                if !undo_manager.get_value().can_undo() {
                    show_toast("Nothing to undo".to_string());
                    return;
                }

                set_is_performing_undo_redo.set(true);

                spawn_local(async move {
                    let current_snapshot = UndoSnapshot::new(
                        graph.get_untracked(),
                        lines.get_untracked(),
                    );

                    let snapshot_opt = std::cell::RefCell::new(None);
                    undo_manager.update_value(|manager| {
                        *snapshot_opt.borrow_mut() = manager.undo(current_snapshot);
                    });

                    if let Some(snapshot) = snapshot_opt.into_inner() {
                        restore_snapshot(snapshot);
                        show_toast("Undoing last change".to_string());

                        // Wait longer than the debounce delay to ensure pending debounced
                        // calls don't record the restored state
                        gloo_timers::future::TimeoutFuture::new(400).await;
                    }

                    set_is_performing_undo_redo.set(false);
                });
            }
            Some("redo") => {
                ev.prevent_default();

                if !undo_manager.get_value().can_redo() {
                    show_toast("Nothing to redo".to_string());
                    return;
                }

                set_is_performing_undo_redo.set(true);

                spawn_local(async move {
                    let current_snapshot = UndoSnapshot::new(
                        graph.get_untracked(),
                        lines.get_untracked(),
                    );

                    let snapshot_opt = std::cell::RefCell::new(None);
                    undo_manager.update_value(|manager| {
                        *snapshot_opt.borrow_mut() = manager.redo(current_snapshot);
                    });

                    if let Some(snapshot) = snapshot_opt.into_inner() {
                        restore_snapshot(snapshot);
                        show_toast("Redoing last change".to_string());

                        // Wait longer than the debounce delay to ensure pending debounced
                        // calls don't record the restored state
                        gloo_timers::future::TimeoutFuture::new(400).await;
                    }
                    set_is_performing_undo_redo.set(false);
                });
            }
            _ => {}
        }
    });

    view! {
        <Title text="RailGraph"/>

        <div class="app">
            <div class="app-header">
                <div class="app-header-content">
                    <div class="app-tabs">
                    <button
                        class=move || if active_tab.get() == AppTab::Infrastructure { "tab-button active" } else { "tab-button" }
                        on:click=move |_| set_active_tab.set(AppTab::Infrastructure)
                    >
                        "Infrastructure"
                    </button>
                    {move || {
                        let current_views = views.get();
                        current_views.iter().map(|view| {
                            let view_id = view.id;
                            view! {
                                <div class="tab-button-container">
                                    {move || {
                                        if editing_view_id.get() == Some(view_id) {
                                            view! {
                                                <input
                                                    type="text"
                                                    class="tab-rename-input"
                                                    value=edit_name_value
                                                    on:input=move |ev| set_edit_name_value.set(event_target_value(&ev))
                                                    on:keydown=move |ev| {
                                                        if ev.key() == "Enter" {
                                                            on_rename_view(view_id, edit_name_value.get());
                                                        } else if ev.key() == "Escape" {
                                                            set_editing_view_id.set(None);
                                                        }
                                                    }
                                                    on:blur=move |_| on_rename_view(view_id, edit_name_value.get())
                                                    prop:autofocus=true
                                                />
                                            }.into_view()
                                        } else {
                                            let current_name = views.get().iter()
                                                .find(|v| v.id == view_id)
                                                .map(|v| v.name.clone())
                                                .unwrap_or_default();
                                            let is_dragging = move || dragged_view_id.get() == Some(view_id);
                                            let is_drag_over = move || drag_over_view_id.get() == Some(view_id);

                                            view! {
                                                <button
                                                    class=move || {
                                                        let mut classes = vec!["tab-button"];
                                                        if active_tab.get() == AppTab::GraphView(view_id) {
                                                            classes.push("active");
                                                        }
                                                        if is_dragging() {
                                                            classes.push("dragging");
                                                        }
                                                        if is_drag_over() {
                                                            classes.push("drag-over");
                                                        }
                                                        classes.join(" ")
                                                    }
                                                    draggable="false"
                                                    on:mousedown=move |_| {
                                                        // Start a timer to enable dragging after 300ms
                                                        let window = web_sys::window().expect("window");
                                                        let set_draggable = move || {
                                                            set_dragged_view_id.set(Some(view_id));
                                                        };
                                                        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(set_draggable) as Box<dyn FnMut()>);
                                                        let timer_id = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                                                            closure.as_ref().unchecked_ref(),
                                                            300
                                                        ).expect("set_timeout");
                                                        closure.forget();
                                                        set_drag_timer_id.set(Some(timer_id));
                                                    }
                                                    on:mouseup=move |_| {
                                                        // Cancel the timer if mouse is released before 300ms
                                                        if let Some(timer_id) = drag_timer_id.get() {
                                                            web_sys::window().expect("window").clear_timeout_with_handle(timer_id);
                                                            set_drag_timer_id.set(None);
                                                        }
                                                        // Clear drag state if released without dragging
                                                        set_dragged_view_id.set(None);
                                                        set_drag_over_view_id.set(None);
                                                    }
                                                    on:mouseleave=move |_| {
                                                        // Cancel the timer if mouse leaves before 300ms
                                                        if let Some(timer_id) = drag_timer_id.get() {
                                                            web_sys::window().expect("window").clear_timeout_with_handle(timer_id);
                                                            set_drag_timer_id.set(None);
                                                        }
                                                    }
                                                    on:click=move |_| {
                                                        // Only handle click if not dragging
                                                        if dragged_view_id.get().is_none() {
                                                            set_active_tab.set(AppTab::GraphView(view_id));
                                                        }
                                                    }
                                                    on:dragstart=move |ev| {
                                                        if let Some(dt) = ev.data_transfer() {
                                                            let _ = dt.set_data("text/plain", &view_id.to_string());
                                                            dt.set_effect_allowed("move");
                                                        }
                                                    }
                                                    on:dragover=move |ev| {
                                                        if dragged_view_id.get().is_some() {
                                                            ev.prevent_default();
                                                            if let Some(dt) = ev.data_transfer() {
                                                                dt.set_drop_effect("move");
                                                            }
                                                            set_drag_over_view_id.set(Some(view_id));
                                                        }
                                                    }
                                                    on:dragleave=move |_| {
                                                        set_drag_over_view_id.set(None);
                                                    }
                                                    on:drop=move |ev| {
                                                        ev.prevent_default();
                                                        ev.stop_propagation();

                                                        if let Some(dragged_id) = dragged_view_id.get() {
                                                            if dragged_id != view_id {
                                                                // Reorder the views array
                                                                set_views.update(|views_vec| {
                                                                    let dragged_idx = views_vec.iter().position(|v| v.id == dragged_id);
                                                                    let target_idx = views_vec.iter().position(|v| v.id == view_id);

                                                                    if let (Some(from), Some(to)) = (dragged_idx, target_idx) {
                                                                        let item = views_vec.remove(from);
                                                                        views_vec.insert(to, item);
                                                                    }
                                                                });
                                                            }
                                                        }

                                                        set_dragged_view_id.set(None);
                                                        set_drag_over_view_id.set(None);
                                                    }
                                                    on:dragend=move |_| {
                                                        set_dragged_view_id.set(None);
                                                        set_drag_over_view_id.set(None);
                                                        if let Some(timer_id) = drag_timer_id.get() {
                                                            web_sys::window().expect("window").clear_timeout_with_handle(timer_id);
                                                            set_drag_timer_id.set(None);
                                                        }
                                                    }
                                                    on:dblclick=move |e| {
                                                        e.stop_propagation();
                                                        let name = views.get().iter()
                                                            .find(|v| v.id == view_id)
                                                            .map(|v| v.name.clone())
                                                            .unwrap_or_default();
                                                        set_edit_name_value.set(name);
                                                        set_editing_view_id.set(Some(view_id));
                                                    }
                                                    prop:draggable=move || dragged_view_id.get() == Some(view_id)
                                                >
                                                    {current_name}
                                                </button>
                                                <button
                                                    class="tab-close-button"
                                                    on:click=move |e| {
                                                        e.stop_propagation();
                                                        on_close_view(view_id);
                                                    }
                                                    title="Close view"
                                                >
                                                    <i class="fa-solid fa-times"></i>
                                                </button>
                                            }.into_view()
                                        }
                                    }}
                                </div>
                            }
                        }).collect::<Vec<_>>()
                    }}
                    </div>
                    <div class="app-header-actions">
                        <ReportIssueButton />
                    </div>
                </div>
            </div>

            <Show
                when=move || !is_loading.get()
                fallback=|| view! {
                    <div class="loading-overlay">
                        <div class="loading-spinner"></div>
                        <p>"Loading project..."</p>
                    </div>
                }
            >
                {move || match active_tab.get() {
                    AppTab::Infrastructure => view! {
                        <InfrastructureView
                            graph=graph
                            set_graph=set_graph
                            lines=lines
                            set_lines=set_lines
                            folders=folders
                            set_folders=set_folders
                            on_create_view=on_create_view
                            settings=settings
                            set_settings=set_settings
                            initial_viewport=infrastructure_viewport.get_untracked()
                            on_viewport_change=Callback::new(move |viewport_state: ViewportState| {
                                set_infrastructure_viewport.set(viewport_state);
                            })
                            on_open_project_manager=Callback::new(move |()| {
                                set_show_project_manager.set(true);
                            })
                        />
                    }.into_view(),
                    AppTab::GraphView(view_id) => {
                        // Find the view with matching ID
                        if let Some(view) = views.get().iter().find(|v| v.id == view_id).cloned() {
                            view! {
                                <TimeGraph
                                    lines=lines
                                    set_lines=set_lines
                                    folders=folders
                                    set_folders=set_folders
                                    graph=graph
                                    set_graph=set_graph
                                    legend=legend
                                    set_legend=set_legend
                                    settings=settings
                                    set_settings=set_settings
                                    view=view
                                    train_journeys=train_journeys
                                    selected_day=selected_day
                                    set_selected_day=set_selected_day
                                    raw_conflicts=raw_conflicts
                                    on_create_view=on_create_view
                                    on_viewport_change=Callback::new(move |viewport_state: ViewportState| {
                                        on_viewport_change(view_id, viewport_state);
                                    })
                                    on_open_changelog=Callback::new(move |()| {
                                        set_manual_open_changelog.set(true);
                                    })
                                    on_open_project_manager=Callback::new(move |()| {
                                        set_show_project_manager.set(true);
                                    })
                                />
                            }.into_view()
                        } else {
                            // View not found, switch back to Infrastructure
                            set_active_tab.set(AppTab::Infrastructure);
                            view! {
                                <div>"View not found"</div>
                            }.into_view()
                        }
                    }
                }}
            </Show>

            <ProjectManager
                is_open=show_project_manager.into()
                on_close=move || set_show_project_manager.set(false)
                on_load_project=on_load_project
                current_project=current_project.into()
            />

            <AlphaDisclaimer />
            <ChangelogPopup
                manual_open=Signal::derive(move || manual_open_changelog.get())
                set_manual_open=move |v| set_manual_open_changelog.set(v)
            />
            <ToastNotification toast=toast />
        </div>
    }
}

