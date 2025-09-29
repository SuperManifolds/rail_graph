use leptos::*;
use leptos_meta::*;
use crate::components::time_graph::TimeGraph;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/nimby_graph.css"/>
        <Title text="Railway Time Graph"/>

        <div class="app">
            <TimeGraph />
        </div>
    }
}