use crate::components::alpha_disclaimer::AlphaDisclaimer;
use crate::components::button::Button;
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
use crate::user_settings_ext::UserSettingsStorage;
use crate::storage::serialize_project_to_bytes;
use crate::sync::{self, SyncEnvelope, SyncKind};
use crate::train_journey::TrainJourney;
use crate::tauri_bridge::ConflictDetector;
use leptos::{
    component, create_effect, create_signal, event_target_value, provide_context, spawn_local,
    store_value, view, Callback, IntoView, Show, Signal, SignalGet, SignalGetUntracked, SignalSet,
    SignalUpdate, WriteSignal,
};
use wasm_bindgen::JsCast;
use leptos_meta::{provide_meta_context, Title};
use std::collections::HashMap;
use uuid::Uuid;

/// Args for conflict detection
type ConflictDetectArgs = (
    Vec<u8>,
    Vec<crate::models::Line>,
    crate::models::ProjectSettings,
    Option<chrono::Weekday>,
    Option<Vec<usize>>,
);

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

/// Load a project from disk via the Tauri backend.
async fn load_project_from_disk() -> Project {
    let project_id = crate::tauri_bridge::get_current_project_id().await.ok().flatten();

    if let Some(id) = project_id {
        match crate::tauri_bridge::load_project(&id).await
            .and_then(|bytes| Project::from_bytes(&bytes))
        {
            Ok(p) => return p,
            Err(e) => {
                web_sys::console::error_1(&format!("Failed to load project: {e}").into());
            }
        }
    }

    Project::empty()
}

#[allow(clippy::too_many_arguments)]
fn handle_sync_event(
    envelope: SyncEnvelope,
    set_is_applying_remote: WriteSignal<bool>,
    shared: SharedWriteSignals,
    set_incoming_drag_tab: WriteSignal<Option<(String, String)>>,
    set_window_tabs: WriteSignal<Vec<String>>,
    set_active_tab: WriteSignal<AppTab>,
    views: leptos::ReadSignal<Vec<GraphView>>,
    is_primary: leptos::ReadSignal<bool>,
    set_is_primary: WriteSignal<bool>,
    on_remote_undo: impl Fn(),
    on_remote_redo: impl Fn(),
) {
    match envelope.kind {
        SyncKind::ProjectSync(bytes) => {
            apply_remote_project(&bytes, set_is_applying_remote, shared);
        }
        SyncKind::TabDragStart { tab_id } => {
            set_incoming_drag_tab.set(Some((tab_id, envelope.source_window.clone())));
        }
        SyncKind::TabDragCancel => {
            set_incoming_drag_tab.set(None);
        }
        SyncKind::TabDrop { tab_id, target_window, insert_index } => {
            let my = crate::tauri_bridge::get_current_window_label().unwrap_or_default();
            if target_window == my {
                set_window_tabs.update(|tabs| {
                    if !tabs.contains(&tab_id) {
                        let idx = insert_index.min(tabs.len());
                        tabs.insert(idx, tab_id.clone());
                    }
                });
                restore_active_tab(&tab_id, &views.get_untracked(), set_active_tab);
            }
            set_incoming_drag_tab.set(None);
        }
        SyncKind::WindowClosing { is_primary: true } => {
            claim_primary_after_delay(set_is_primary);
        }
        SyncKind::PrimaryClaim => {
            set_is_primary.set(false);
        }
        SyncKind::UndoRequest => {
            if is_primary.get_untracked() {
                on_remote_undo();
            }
        }
        SyncKind::RedoRequest => {
            if is_primary.get_untracked() {
                on_remote_redo();
            }
        }
        SyncKind::WindowClosing { is_primary: false }
        | SyncKind::LayoutUpdate { .. } => {}
    }
}

#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)]
struct SharedWriteSignals {
    set_lines: WriteSignal<Vec<crate::models::Line>>,
    set_folders: WriteSignal<Vec<crate::models::LineFolder>>,
    set_graph: WriteSignal<RailwayGraph>,
    set_legend: WriteSignal<Legend>,
    set_settings: WriteSignal<crate::models::ProjectSettings>,
    set_viewport_states: WriteSignal<HashMap<Uuid, ViewportState>>,
    set_views: WriteSignal<Vec<GraphView>>,
    set_current_project: WriteSignal<Project>,
}

fn apply_remote_project(
    bytes: &[u8],
    set_is_applying_remote: WriteSignal<bool>,
    s: SharedWriteSignals,
) {
    let Ok(project) = Project::from_bytes(bytes) else {
        leptos::logging::error!("Failed to deserialize sync payload");
        return;
    };

    set_is_applying_remote.set(true);
    leptos::batch(move || {
        s.set_lines.set(project.lines.clone());
        s.set_folders.set(project.folders.clone());
        s.set_graph.set(project.graph.clone());
        s.set_legend.set(project.legend.clone());
        s.set_settings.set(project.settings.clone());

        let mut project_views = project.views.clone();
        if project_views.is_empty() {
            project_views.push(GraphView::default_main_line(&project.graph));
        }
        let viewports: HashMap<Uuid, ViewportState> = project_views
            .iter()
            .map(|v| (v.id, v.viewport_state.clone()))
            .collect();
        s.set_viewport_states.set(viewports);
        s.set_views.set(project_views);
        s.set_current_project.set(project);
    });
    set_is_applying_remote.set(false);
}

async fn save_and_cache_project(bytes: Vec<u8>, project_id: String) {
    let _ = crate::tauri_bridge::cache_project_state(&bytes).await;
    if let Err(e) = crate::tauri_bridge::save_project(&bytes, &project_id).await {
        web_sys::console::error_1(&format!("Auto-save failed: {e}").into());
        return;
    }
    if let Err(e) = crate::tauri_bridge::set_current_project_id(&project_id).await {
        web_sys::console::error_1(
            &format!("Failed to set current project ID: {e}").into(),
        );
    }
}

fn claim_primary_after_delay(set_is_primary: WriteSignal<bool>) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let delay = (js_sys::Math::random() * 100.0) as i32;
    let label = crate::tauri_bridge::get_current_window_label().unwrap_or_default();
    let closure = wasm_bindgen::closure::Closure::once(move || {
        sync::broadcast_primary_claim(&label);
        set_is_primary.set(true);
    });
    let _ = web_sys::window()
        .expect("window")
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            delay,
        );
    closure.forget();
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
pub fn App(
    #[prop(optional)] window_id: Option<String>,
    #[prop(default = false)] is_secondary: bool,
    #[prop(optional)] initial_tab: Option<String>,
) -> impl IntoView {
    provide_meta_context();

    let window_label = crate::tauri_bridge::get_current_window_label()
        .unwrap_or_else(|| "main".to_string());
    let window_id_stored = store_value(window_id.unwrap_or_else(|| Uuid::new_v4().to_string()));

    let (is_primary, set_is_primary) = create_signal(!is_secondary);
    let (is_applying_remote, set_is_applying_remote) = create_signal(false);
    // Per-window tab list: "infrastructure" or view UUID strings
    let (window_tabs, set_window_tabs) = create_signal(Vec::<String>::new());
    // Cross-window tab drag state (tab_id, source_window_label)
    let (incoming_drag_tab, set_incoming_drag_tab) = create_signal(None::<(String, String)>);

    let (active_tab, set_active_tab) = create_signal(AppTab::Infrastructure);

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

    // Sidebar visibility (global across all views)
    let (sidebar_visible, set_sidebar_visible) = create_signal(true);

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
            if is_performing_undo_redo.get_untracked() || is_applying_remote.get_untracked() {
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

        // Skip during undo/redo operations or remote state application
        if is_performing_undo_redo.get_untracked() || is_applying_remote.get_untracked() {
            return;
        }

        let snapshot = UndoSnapshot::new(current_graph, current_lines);
        record_snapshot.update_value(|f| f(snapshot));
    });

    // Load user settings on mount
    create_effect(move |_| {
        match crate::models::UserSettings::load() {
            Ok(settings) => {
                set_user_settings.set(settings);
            }
            Err(e) => {
                leptos::logging::warn!("Failed to load user settings: {}", e);
                // Use defaults
            }
        }
    });

    // Auto-load saved project on component mount
    let is_secondary_mount = is_secondary;
    let initial_tab_mount = initial_tab.clone();
    create_effect(move |_| {
        let initial_tab_val = initial_tab_mount.clone();
        spawn_local(async move {
            let project = if is_secondary_mount {
                // Secondary window: load from backend cache
                match crate::tauri_bridge::get_cached_project_state().await
                    .and_then(|bytes| Project::from_bytes(&bytes))
                {
                    Ok(p) => p,
                    Err(e) => {
                        log!("Failed to load cached state, falling back to disk: {}", e);
                        load_project_from_disk().await
                    }
                }
            } else {
                load_project_from_disk().await
            };

            let empty_graph = project.graph.clone();

            set_current_project.set(project.clone());
            set_lines.set(project.lines.clone());
            set_folders.set(project.folders.clone());
            set_graph.set(project.graph.clone());
            set_legend.set(project.legend.clone());
            set_settings.set(project.settings.clone());

            // Ensure we have at least one view (create default "Main Line" view)
            let mut project_views = project.views.clone();
            if project_views.is_empty() {
                project_views.push(GraphView::default_main_line(&empty_graph));
            }

            // Extract viewport states into separate signal
            let viewports: HashMap<Uuid, ViewportState> = project_views
                .iter()
                .map(|v| (v.id, v.viewport_state.clone()))
                .collect();
            set_viewport_states.set(viewports);
            set_infrastructure_viewport.set(project.infrastructure_viewport.clone());

            set_views.set(project_views.clone());

            // Populate per-window tabs
            let tabs = if is_secondary_mount {
                // Secondary window: show only the initial_tab (or Infrastructure)
                if let Some(ref tab_id) = initial_tab_val {
                    vec![tab_id.clone()]
                } else {
                    vec!["infrastructure".to_string()]
                }
            } else {
                // Primary window: restore from saved window_layouts or default to all tabs
                let saved_layout = project.window_layouts.first();
                if let Some(layout) = saved_layout {
                    layout.tab_ids.clone()
                } else {
                    // Legacy: build tab list from active_tab_id + all views
                    let mut tabs: Vec<String> = vec!["infrastructure".to_string()];
                    tabs.extend(project_views.iter().map(|v| v.id.to_string()));
                    tabs
                }
            };
            set_window_tabs.set(tabs);

            // Restore active tab
            if is_secondary_mount {
                if let Some(ref tab_id) = initial_tab_val {
                    restore_active_tab(tab_id, &project_views, set_active_tab);
                }
            } else {
                let saved_active = project.window_layouts.first()
                    .and_then(|l| l.active_tab_id.clone())
                    .or(project.active_tab_id.clone());
                if let Some(tab_id) = saved_active {
                    restore_active_tab(&tab_id, &project_views, set_active_tab);
                } else if let Some(first_view) = project_views.first() {
                    set_active_tab.set(AppTab::GraphView(first_view.id));
                }
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

    // Broadcast shared state to other windows when it changes (debounced)
    let sync_window_label = window_label.clone();
    let debounced_broadcast_sync = store_value(leptos::leptos_dom::helpers::debounce(
        std::time::Duration::from_millis(50),
        move |bytes: Vec<u8>| {
            sync::broadcast_project_sync(&sync_window_label, bytes);
        },
    ));

    // Auto-save project whenever shared state changes (primary only) + broadcast sync
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

        // Skip during remote state application to avoid re-broadcast loops
        if is_applying_remote.get_untracked() {
            return;
        }

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
            proj.active_tab_id.clone_from(&active_tab_id);
            proj.infrastructure_viewport = current_infrastructure_viewport;

            // Persist this window's tab layout
            let current_tabs = window_tabs.get_untracked();
            let layout = crate::models::WindowLayout {
                window_id: Uuid::parse_str(&window_id_stored.get_value()).unwrap_or_else(|_| Uuid::new_v4()),
                tab_ids: current_tabs,
                active_tab_id,
                bounds: None,
            };
            proj.window_layouts = vec![layout];

            proj.touch_updated_at();

            // Update current_project signal to keep it synchronized
            set_current_project.set(proj.clone());

            // Serialize for saving and syncing
            let bytes = match serialize_project_to_bytes(&proj) {
                Ok(b) => b,
                Err(e) => {
                    web_sys::console::error_1(&format!("Serialization failed: {e}").into());
                    return;
                }
            };

            // Broadcast to other windows
            let sync_bytes = bytes.clone();
            debounced_broadcast_sync.update_value(|f| f(sync_bytes));

            if is_primary.get_untracked() {
                let project_id = proj.metadata.id.clone();
                spawn_local(save_and_cache_project(bytes, project_id));
            }
        }
    });

    // Sync listener is set up below, after undo_manager is available

    // Broadcast window-closing when this window is about to close
    let close_label = window_label.clone();
    leptos::leptos_dom::helpers::window_event_listener(leptos::ev::beforeunload, move |_| {
        sync::broadcast_window_closing(&close_label, is_primary.get_untracked());
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
    let (is_calculating_conflicts, set_is_calculating_conflicts) = create_signal(false);

    let detector = store_value(ConflictDetector::new(set_conflicts, set_is_calculating_conflicts));

    // Create debounced conflict detection to avoid excessive recomputation
    let debounced_detect_conflicts = store_value(leptos::leptos_dom::helpers::debounce(
        std::time::Duration::from_millis(300),
        move |(project_bytes, visible_lines, current_settings, day_filter, view_edge_filter): ConflictDetectArgs| {
            detector.update_value(|d| {
                d.detect(project_bytes, visible_lines, current_settings, day_filter, view_edge_filter);
            });
        },
    ));

    create_effect(move |_| {
        if !initial_load_complete.get() {
            return;
        }

        let current_lines = lines.get();
        let current_settings = settings.get();
        let day_filter = selected_day.get();

        let view_edge_filter = match active_tab.get() {
            AppTab::GraphView(view_id) => {
                views.get_untracked()
                    .iter()
                    .find(|v| v.id == view_id)
                    .and_then(|v| v.edge_path.clone())
            }
            AppTab::Infrastructure => None,
        };

        let visible_lines: Vec<_> = current_lines
            .into_iter()
            .filter(|line| line.visible)
            .collect();

        // Serialize project to bytes for conflict detection
        let proj = current_project.get_untracked();
        let project_bytes = match serialize_project_to_bytes(&proj) {
            Ok(b) => b,
            Err(e) => {
                log!("Failed to serialize project for conflict detection: {}", e);
                return;
            }
        };

        debounced_detect_conflicts.update_value(|f| {
            f((project_bytes, visible_lines, current_settings, day_filter, view_edge_filter));
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
        // Add to this window's tabs and activate
        let tab_id = view_id.to_string();
        set_window_tabs.update(|tabs| {
            if !tabs.contains(&tab_id) {
                tabs.push(tab_id);
            }
        });
        set_active_tab.set(AppTab::GraphView(view_id));
    });

    // Close a tab from this window (does NOT delete the view)
    let on_close_tab = move |tab_id: String| {
        let is_active = match active_tab.get() {
            AppTab::Infrastructure => tab_id == "infrastructure",
            AppTab::GraphView(id) => tab_id == id.to_string(),
        };

        set_window_tabs.update(|tabs| {
            tabs.retain(|t| *t != tab_id);
        });

        if is_active {
            let remaining_tabs = window_tabs.get();
            if let Some(first_tab) = remaining_tabs.first() {
                restore_active_tab(first_tab, &views.get(), set_active_tab);
            } else {
                set_active_tab.set(AppTab::Infrastructure);
            }
        }
    };

    // State for renaming views
    let (editing_view_id, set_editing_view_id) = create_signal(None::<Uuid>);
    let (edit_name_value, set_edit_name_value) = create_signal(String::new());

    // State for drag-and-drop reordering (within same window)
    let (dragged_view_id, set_dragged_view_id) = create_signal(None::<Uuid>);
    let (drag_over_view_id, set_drag_over_view_id) = create_signal(None::<Uuid>);

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

            // Set window tabs to all views + infrastructure
            let mut tabs: Vec<String> = vec!["infrastructure".to_string()];
            tabs.extend(project_views.iter().map(|v| v.id.to_string()));
            set_window_tabs.set(tabs);

            // Set active tab
            if let Some(tab_id) = &project.active_tab_id {
                restore_active_tab(tab_id, &project_views, set_active_tab);
            } else if let Some(first_view) = project_views.first() {
                set_active_tab.set(AppTab::GraphView(first_view.id));
            }
        });

        // Set this as the current project
        spawn_local(async move {
            if let Err(e) = crate::tauri_bridge::set_current_project_id(&project_id).await {
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

    // Listen for sync events from other windows (must be after undo_manager + restore_snapshot)
    let sync_my_label = window_label.clone();
    spawn_local(async move {
        let my_label = sync_my_label;
        let _ = sync::listen(move |envelope: SyncEnvelope| {
            if envelope.source_window == my_label {
                return;
            }
            let shared = SharedWriteSignals {
                set_lines, set_folders, set_graph, set_legend, set_settings,
                set_viewport_states, set_views, set_current_project,
            };
            handle_sync_event(
                envelope, set_is_applying_remote, shared,
                set_incoming_drag_tab, set_window_tabs, set_active_tab,
                views, is_primary, set_is_primary,
                // on_remote_undo
                move || {
                    if !undo_manager.get_value().can_undo() { return; }
                    set_is_performing_undo_redo.set(true);
                    let current = UndoSnapshot::new(graph.get_untracked(), lines.get_untracked());
                    let snap = std::cell::RefCell::new(None);
                    undo_manager.update_value(|m| { *snap.borrow_mut() = m.undo(current); });
                    if let Some(s) = snap.into_inner() { restore_snapshot(s); }
                    set_is_performing_undo_redo.set(false);
                },
                // on_remote_redo
                move || {
                    if !undo_manager.get_value().can_redo() { return; }
                    set_is_performing_undo_redo.set(true);
                    let current = UndoSnapshot::new(graph.get_untracked(), lines.get_untracked());
                    let snap = std::cell::RefCell::new(None);
                    undo_manager.update_value(|m| { *snap.borrow_mut() = m.redo(current); });
                    if let Some(s) = snap.into_inner() { restore_snapshot(s); }
                    set_is_performing_undo_redo.set(false);
                },
            );
        }).await;
    });

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

        // New window: Cmd+Shift+N (Mac) or Ctrl+Shift+N
        if ev.shift_key() && (ev.meta_key() || ev.ctrl_key()) && ev.code() == "KeyN" {
            ev.prevent_default();
            spawn_local(async {
                if let Err(e) = crate::tauri_bridge::create_main_window(None).await {
                    leptos::logging::error!("Failed to create new window: {e}");
                }
            });
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

                if !is_primary.get_untracked() {
                    let label = crate::tauri_bridge::get_current_window_label()
                        .unwrap_or_default();
                    sync::broadcast_undo_request(&label);
                    return;
                }

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

                        gloo_timers::future::TimeoutFuture::new(400).await;
                    }

                    set_is_performing_undo_redo.set(false);
                });
            }
            Some("redo") => {
                ev.prevent_default();

                if !is_primary.get_untracked() {
                    let label = crate::tauri_bridge::get_current_window_label()
                        .unwrap_or_default();
                    sync::broadcast_redo_request(&label);
                    return;
                }

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
                    <div class="app-tabs"
                        on:dragover=move |ev| {
                            // Accept drops from cross-window tab drags
                            if incoming_drag_tab.get().is_some() {
                                ev.prevent_default();
                            }
                        }
                        on:drop=move |ev| {
                            ev.prevent_default();
                            if let Some((tab_id, _source)) = incoming_drag_tab.get() {
                                let my_label = crate::tauri_bridge::get_current_window_label()
                                    .unwrap_or_default();
                                let tabs = window_tabs.get();
                                let insert_idx = tabs.len();
                                sync::broadcast_tab_drop(&my_label, &tab_id, &my_label, insert_idx);
                                // Add the tab to this window
                                set_window_tabs.update(|tabs| {
                                    if !tabs.contains(&tab_id) {
                                        tabs.push(tab_id.clone());
                                    }
                                });
                                restore_active_tab(&tab_id, &views.get_untracked(), set_active_tab);
                                set_incoming_drag_tab.set(None);
                            }
                        }
                    >
                    {move || {
                        let tabs = window_tabs.get();
                        let current_views = views.get();
                        tabs.iter().map(|tab_id| {
                            let tab_id = tab_id.clone();
                            if tab_id == "infrastructure" {
                                view! {
                                    <div class="tab-button-container">
                                        <button
                                            class=move || if active_tab.get() == AppTab::Infrastructure { "tab-button active" } else { "tab-button" }
                                            on:click=move |_| set_active_tab.set(AppTab::Infrastructure)
                                        >
                                            "Infrastructure"
                                        </button>
                                        <button
                                            class="tab-close-button"
                                            on:click=move |e| {
                                                e.stop_propagation();
                                                on_close_tab("infrastructure".to_string());
                                            }
                                            title="Close tab"
                                        >
                                            <i class="fa-solid fa-times"></i>
                                        </button>
                                    </div>
                                }.into_view()
                            } else {
                                let Ok(view_uuid) = Uuid::parse_str(&tab_id) else {
                                    return view! { <div /> }.into_view();
                                };
                                let view_id = view_uuid;
                                let tab_id_for_close = tab_id.clone();
                                let tab_id_for_drag = tab_id.clone();

                                // Check if this view exists
                                if !current_views.iter().any(|v| v.id == view_id) {
                                    return view! { <div /> }.into_view();
                                }

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
                                                let tab_id_for_dragstart = tab_id_for_drag.clone();
                                                let tab_id_for_tearoff = tab_id_for_drag.clone();

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
                                                        draggable="true"
                                                        on:click=move |_| {
                                                            set_active_tab.set(AppTab::GraphView(view_id));
                                                        }
                                                        on:dragstart=move |ev| {
                                                            set_dragged_view_id.set(Some(view_id));
                                                            if let Some(dt) = ev.data_transfer() {
                                                                let _ = dt.set_data("text/plain", &view_id.to_string());
                                                                dt.set_effect_allowed("move");
                                                            }
                                                            let my_label = crate::tauri_bridge::get_current_window_label()
                                                                .unwrap_or_default();
                                                            sync::broadcast_tab_drag_start(&my_label, &tab_id_for_dragstart);
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
                                                                    set_window_tabs.update(|tabs| {
                                                                        let dragged_str = dragged_id.to_string();
                                                                        let target_str = view_id.to_string();
                                                                        let dragged_idx = tabs.iter().position(|t| *t == dragged_str);
                                                                        let target_idx = tabs.iter().position(|t| *t == target_str);
                                                                        if let (Some(from), Some(to)) = (dragged_idx, target_idx) {
                                                                            let item = tabs.remove(from);
                                                                            tabs.insert(to, item);
                                                                        }
                                                                    });
                                                                }
                                                            }

                                                            set_dragged_view_id.set(None);
                                                            set_drag_over_view_id.set(None);
                                                        }
                                                        on:dragend=move |ev| {
                                                            let drop_effect = ev.data_transfer()
                                                                .map(|dt| dt.drop_effect())
                                                                .unwrap_or_default();
                                                            set_dragged_view_id.set(None);
                                                            set_drag_over_view_id.set(None);
                                                            let my_label = crate::tauri_bridge::get_current_window_label()
                                                                .unwrap_or_default();
                                                            sync::broadcast_tab_drag_cancel(&my_label);

                                                            // Tear-off: if drag ended with no drop, open tab in new window
                                                            if drop_effect == "none" && window_tabs.get_untracked().len() > 1 {
                                                                let tid = tab_id_for_tearoff.clone();
                                                                on_close_tab(tid.clone());
                                                                spawn_local(async move {
                                                                    if let Err(e) = crate::tauri_bridge::create_main_window(Some(&tid)).await {
                                                                        leptos::logging::error!("Failed to create tear-off window: {e}");
                                                                    }
                                                                });
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
                                                    >
                                                        {current_name}
                                                    </button>
                                                }.into_view()
                                            }
                                        }}
                                        <button
                                            class="tab-close-button"
                                            on:click=move |e| {
                                                e.stop_propagation();
                                                on_close_tab(tab_id_for_close.clone());
                                            }
                                            title="Close tab"
                                        >
                                            <i class="fa-solid fa-times"></i>
                                        </button>
                                    </div>
                                }.into_view()
                            }
                        }).collect::<Vec<_>>()
                    }}
                    </div>
                    <div class="app-header-actions">
                        <Button
                            class="button-icon-only"
                            on_click=leptos::Callback::new(move |_| set_sidebar_visible.update(|v| *v = !*v))
                            active=Signal::derive(move || sidebar_visible.get())
                            title="Toggle sidebar"
                        >
                            <i class="fa-solid fa-bars-staggered"></i>
                        </Button>
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
                            sidebar_visible=sidebar_visible
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
                                    is_calculating_conflicts=is_calculating_conflicts
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
                                    sidebar_visible=sidebar_visible
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

