use leptos::{component, create_effect, create_signal, IntoView, Show, SignalGet, SignalSet, spawn_local, view};
use leptos_meta::{provide_meta_context, Stylesheet, Title};
use crate::components::time_graph::TimeGraph;
use crate::components::infrastructure_view::InfrastructureView;
use crate::models::{Project, RailwayGraph, Legend};
use crate::storage::{load_project_from_storage, save_project_to_storage};

#[derive(Clone, Copy, PartialEq)]
enum AppView {
    TimeGraph,
    Infrastructure,
}

#[component]
#[must_use]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let (active_view, set_active_view) = create_signal(AppView::TimeGraph);

    // Shared graph and lines state
    let (lines, set_lines) = create_signal(Vec::new());
    let (graph, set_graph) = create_signal(RailwayGraph::new());
    let (legend, set_legend) = create_signal(Legend::default());
    let (is_loading, set_is_loading) = create_signal(true);
    let (initial_load_complete, set_initial_load_complete) = create_signal(false);

    // Auto-load saved project on component mount
    create_effect(move |_| {
        spawn_local(async move {
            match load_project_from_storage().await {
                Ok(project) => {
                    set_lines.set(project.lines);
                    set_graph.set(project.graph);
                    set_legend.set(project.legend);
                }
                Err(_) => {
                    set_lines.set(Vec::new());
                    set_graph.set(RailwayGraph::new());
                    set_legend.set(Legend::default());
                }
            }
            set_initial_load_complete.set(true);
        });
    });

    // Auto-save project whenever lines, graph, or legend change
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_graph = graph.get();
        let current_legend = legend.get();

        if !current_lines.is_empty() || current_graph.graph.node_count() > 0 {
            let project = Project::new(current_lines, current_graph, current_legend);
            spawn_local(async move {
                if let Err(e) = save_project_to_storage(&project).await {
                    web_sys::console::error_1(&format!("Auto-save failed: {}", e).into());
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

    view! {
        <Stylesheet id="leptos" href="/pkg/nimby_graph.css"/>
        <Title text="Railway Time Graph"/>

        <div class="app">
            <div class="app-header">
                <div class="app-tabs">
                    <button
                        class=move || if active_view.get() == AppView::TimeGraph { "tab-button active" } else { "tab-button" }
                        on:click=move |_| set_active_view.set(AppView::TimeGraph)
                    >
                        "Time Graph"
                    </button>
                    <button
                        class=move || if active_view.get() == AppView::Infrastructure { "tab-button active" } else { "tab-button" }
                        on:click=move |_| set_active_view.set(AppView::Infrastructure)
                    >
                        "Infrastructure"
                    </button>
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
                {move || match active_view.get() {
                    AppView::TimeGraph => view! {
                        <TimeGraph lines=lines set_lines=set_lines graph=graph set_graph=set_graph legend=legend set_legend=set_legend />
                    }.into_view(),
                    AppView::Infrastructure => view! {
                        <InfrastructureView graph=graph set_graph=set_graph lines=lines set_lines=set_lines />
                    }.into_view(),
                }}
            </Show>
        </div>
    }
}