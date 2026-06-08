use crate::components::native_window::NativeWindow;
use crate::models::Project;
use crate::window_protocol::{ProjectManagerInit, ProjectManagerResult};
use leptos::{component, view, IntoView, Callback, Callable, Signal, SignalGet};

#[component]
pub fn ProjectManager(
    is_open: Signal<bool>,
    on_close: impl Fn() + 'static + Clone,
    on_load_project: Callback<Project>,
    current_project: Signal<Project>,
) -> impl IntoView {
    let on_close_for_window = on_close.clone();
    let on_close_for_result = on_close.clone();

    let result_handler: Box<dyn Fn(String)> = Box::new(move |json: String| {
        match serde_json::from_str::<ProjectManagerResult>(&json) {
            Ok(ProjectManagerResult::LoadProject(bytes)) => {
                match Project::from_bytes(&bytes) {
                    Ok(project) => {
                        on_load_project.call(project);
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to deserialize loaded project: {}", e);
                    }
                }
            }
            Ok(ProjectManagerResult::CreateProject(bytes)) => {
                match Project::from_bytes(&bytes) {
                    Ok(project) => {
                        on_load_project.call(project);
                    }
                    Err(e) => {
                        leptos::logging::error!("Failed to deserialize new project: {}", e);
                    }
                }
            }
            Err(e) => {
                leptos::logging::error!("Failed to parse ProjectManagerResult: {}", e);
            }
        }
        on_close_for_result();
    });

    view! {
        <NativeWindow
            is_open=is_open
            title=Signal::derive(|| "Projects".to_string())
            on_close=move || on_close_for_window()
            window_type="project-manager"
            init_data=Signal::derive(move || {
                if !is_open.get() {
                    return String::new();
                }
                let proj = current_project.get();
                let bytes = proj.serialize_to_bytes().unwrap_or_default();
                serde_json::to_string(&ProjectManagerInit {
                    current_project_id: proj.metadata.id.clone(),
                    current_project_bytes: bytes,
                }).unwrap_or_default()
            })
            on_result=result_handler
            size=(800, 600)
            position_key="project-manager"
        />
    }
}
