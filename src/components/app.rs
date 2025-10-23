use leptos::{component, create_effect, create_signal, IntoView, Show, SignalGet, SignalSet, spawn_local, view, WriteSignal, Callback, SignalUpdate, event_target_value, Signal, store_value, SignalGetUntracked};
use std::collections::HashMap;
use leptos_meta::{provide_meta_context, Title};
use uuid::Uuid;
use crate::components::time_graph::TimeGraph;
use crate::components::infrastructure_view::InfrastructureView;
use crate::components::project_manager::ProjectManager;
use crate::components::alpha_disclaimer::AlphaDisclaimer;
use crate::components::changelog_popup::ChangelogPopup;
use crate::components::report_issue_button::ReportIssueButton;
use crate::models::{Project, RailwayGraph, Legend, GraphView, ViewportState};
use crate::storage::{IndexedDbStorage, Storage};
use crate::train_journey::TrainJourney;
use crate::conflict::Conflict;
use crate::worker_bridge::ConflictDetector;

#[derive(Clone, PartialEq)]
enum AppTab {
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

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let (active_tab, set_active_tab) = create_signal(AppTab::Infrastructure);

    // Storage implementation
    let storage = IndexedDbStorage;

    // Shared graph, lines, and views state
    let (lines, set_lines) = create_signal(Vec::new());
    let (graph, set_graph) = create_signal(RailwayGraph::new());
    let (legend, set_legend) = create_signal(Legend::default());
    let (views, set_views) = create_signal(Vec::new());
    let (is_loading, set_is_loading) = create_signal(true);
    let (initial_load_complete, set_initial_load_complete) = create_signal(false);

    // Store viewport states separately to avoid triggering view updates
    let (viewport_states, set_viewport_states) = create_signal(HashMap::<Uuid, ViewportState>::new());
    let (infrastructure_viewport, set_infrastructure_viewport) = create_signal(ViewportState::default());

    // Compute train journeys at app level
    let (train_journeys, set_train_journeys) = create_signal(std::collections::HashMap::<uuid::Uuid, TrainJourney>::new());
    let (selected_day, set_selected_day) = create_signal(None::<chrono::Weekday>);

    // Project manager state
    let (show_project_manager, set_show_project_manager) = create_signal(false);
    let (current_project, set_current_project) = create_signal(Project::empty());

    // Auto-load saved project on component mount
    create_effect(move |_| {
        spawn_local(async move {
            // Try to load the last used project
            let project_id = storage.get_current_project_id().await.ok().flatten();

            let project = if let Some(id) = project_id {
                match storage.load_project(&id).await {
                    Ok(p) => {
                        web_sys::console::log_1(&"Project loaded successfully".into());
                        Some(p)
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to load project: {e}").into());
                        None
                    }
                }
            } else {
                web_sys::console::log_1(&"No previous project found".into());
                None
            };

            let project = project.unwrap_or_else(|| {
                web_sys::console::log_1(&"Creating empty project".into());
                Project::empty()
            });
            let empty_graph = project.graph.clone();

            set_current_project.set(project.clone());
            set_lines.set(project.lines.clone());
            set_graph.set(project.graph.clone());
            set_legend.set(project.legend);

            // Ensure we have at least one view (create default "Main Line" view)
            let mut views = project.views.clone();
            if views.is_empty() {
                views.push(GraphView::default_main_line(&empty_graph));
            }

            // Extract viewport states into separate signal
            let viewports: HashMap<Uuid, ViewportState> = views.iter()
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

        let counts_changed = prev_counts.is_some_and(|(prev_nodes, prev_edges)|
            node_count != prev_nodes || edge_count != prev_edges
        );

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

    // Auto-save project whenever lines, graph, legend, views, viewport states, or active tab change
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_graph = graph.get();
        let current_legend = legend.get();
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
            let views_with_viewports: Vec<GraphView> = current_views.into_iter()
                .map(|mut v| {
                    if let Some(viewport) = current_viewports.get(&v.id) {
                        v.viewport_state = viewport.clone();
                    }
                    v
                })
                .collect();

            // Update project with current data, preserving metadata
            proj.lines = current_lines;
            proj.graph = current_graph;
            proj.legend = current_legend;
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
                    web_sys::console::error_1(&format!("Failed to set current project ID: {e}").into());
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
        let visible_lines: Vec<_> = current_lines.into_iter()
            .filter(|line| line.visible)
            .collect();

        // Generate journeys for the full day
        let new_journeys = TrainJourney::generate_journeys(&visible_lines, &current_graph, day_filter);
        set_train_journeys.set(new_journeys);
    });

    // Compute conflicts at app level using worker
    let (conflicts, set_conflicts) = create_signal(Vec::new());

    let detector = store_value(ConflictDetector::new(set_conflicts));

    create_effect(move |_| {
        let journeys = train_journeys.get();
        let journeys_vec: Vec<_> = journeys.values().cloned().collect();
        let current_graph = graph.get();

        detector.update_value(|d| {
            d.detect(journeys_vec, current_graph);
        });
    });

    let raw_conflicts: Signal<Vec<Conflict>> = conflicts.into();

    // Callback for creating a new view
    let on_create_view = Callback::new(move |new_view: GraphView| {
        let view_id = new_view.id;
        let viewport = new_view.viewport_state.clone();
        set_viewport_states.update(|vs| { vs.insert(view_id, viewport); });
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
        set_viewport_states.update(|vs| { vs.remove(&view_id); });

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

        set_current_project.set(project.clone());
        set_lines.set(project.lines.clone());
        set_graph.set(project.graph.clone());
        set_legend.set(project.legend.clone());

        // Handle views
        let mut project_views = project.views.clone();
        if project_views.is_empty() {
            project_views.push(GraphView::default_main_line(&project.graph));
        }

        // Extract viewport states
        let viewports: HashMap<Uuid, ViewportState> = project_views.iter()
            .map(|v| (v.id, v.viewport_state.clone()))
            .collect();
        set_viewport_states.set(viewports);
        set_infrastructure_viewport.set(project.infrastructure_viewport.clone());
        set_views.set(project_views.clone());

        // Set active tab
        if let Some(tab_id) = &project.active_tab_id {
            restore_active_tab(tab_id, &project_views, set_active_tab);
        } else if let Some(first_view) = project_views.first() {
            set_active_tab.set(AppTab::GraphView(first_view.id));
        }

        // Set this as the current project
        spawn_local(async move {
            if let Err(e) = storage.set_current_project_id(&project_id).await {
                web_sys::console::error_1(&format!("Failed to set current project ID: {e}").into());
            }
        });
    });

    view! {
        <Title text="Railway Time Graph"/>

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
                                            view! {
                                                <button
                                                    class=move || if active_tab.get() == AppTab::GraphView(view_id) { "tab-button active" } else { "tab-button" }
                                                    on:click=move |_| set_active_tab.set(AppTab::GraphView(view_id))
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
                            on_create_view=on_create_view
                            initial_viewport=infrastructure_viewport.get_untracked()
                            on_viewport_change=Callback::new(move |viewport_state: ViewportState| {
                                set_infrastructure_viewport.set(viewport_state);
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
                                    graph=graph
                                    set_graph=set_graph
                                    legend=legend
                                    set_legend=set_legend
                                    view=view
                                    train_journeys=train_journeys
                                    selected_day=selected_day
                                    set_selected_day=set_selected_day
                                    raw_conflicts=raw_conflicts
                                    on_create_view=on_create_view
                                    on_viewport_change=Callback::new(move |viewport_state: ViewportState| {
                                        on_viewport_change(view_id, viewport_state);
                                    })
                                    set_show_project_manager=set_show_project_manager
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
            <ChangelogPopup />
        </div>
    }
}