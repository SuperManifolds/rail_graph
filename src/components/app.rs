use leptos::{component, create_effect, create_signal, IntoView, Show, SignalGet, SignalSet, spawn_local, view, WriteSignal, Callback, SignalUpdate, event_target_value};
use leptos_meta::{provide_meta_context, Stylesheet, Title};
use uuid::Uuid;
use crate::components::time_graph::TimeGraph;
use crate::components::infrastructure_view::InfrastructureView;
use crate::models::{Project, RailwayGraph, Legend, GraphView};
use crate::storage::{load_project_from_storage, save_project_to_storage};

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

    // Shared graph, lines, and views state
    let (lines, set_lines) = create_signal(Vec::new());
    let (graph, set_graph) = create_signal(RailwayGraph::new());
    let (legend, set_legend) = create_signal(Legend::default());
    let (views, set_views) = create_signal(Vec::new());
    let (is_loading, set_is_loading) = create_signal(true);
    let (initial_load_complete, set_initial_load_complete) = create_signal(false);

    // Auto-load saved project on component mount
    create_effect(move |_| {
        spawn_local(async move {
            let Ok(project) = load_project_from_storage().await else {
                set_lines.set(Vec::new());
                set_graph.set(RailwayGraph::new());
                set_legend.set(Legend::default());
                set_views.set(Vec::new());
                set_initial_load_complete.set(true);
                return;
            };

            set_lines.set(project.lines.clone());
            set_graph.set(project.graph.clone());
            set_legend.set(project.legend);

            // Ensure we have at least one view (create default "Main Line" view)
            let mut views = project.views.clone();
            if views.is_empty() {
                if let Some(default_view) = GraphView::default_main_line(&project.graph) {
                    views.push(default_view);
                }
            }
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

    // Auto-save project whenever lines, graph, legend, views, or active tab change
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_graph = graph.get();
        let current_legend = legend.get();
        let current_views = views.get();
        let current_tab = active_tab.get();

        if !current_lines.is_empty() || current_graph.graph.node_count() > 0 {
            // Convert active tab to string ID
            let active_tab_id = match current_tab {
                AppTab::Infrastructure => Some("infrastructure".to_string()),
                AppTab::GraphView(uuid) => Some(uuid.to_string()),
            };

            let mut project = Project::new(current_lines, current_graph, current_legend);
            project.views = current_views;
            project.active_tab_id = active_tab_id;

            spawn_local(async move {
                if let Err(e) = save_project_to_storage(&project).await {
                    web_sys::console::error_1(&format!("Auto-save failed: {e}").into());
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

    // Callback for creating a new view
    let on_create_view = Callback::new(move |new_view: GraphView| {
        let view_id = new_view.id;
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

        // Remove the view
        set_views.update(|v| v.retain(|view| view.id != view_id));

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

    view! {
        <Stylesheet id="leptos" href="/pkg/nimby_graph.css"/>
        <Title text="Railway Time Graph"/>

        <div class="app">
            <div class="app-header">
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
                        <InfrastructureView graph=graph set_graph=set_graph lines=lines set_lines=set_lines />
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
                                    on_create_view=on_create_view
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
        </div>
    }
}