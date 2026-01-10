use leptos::{component, view, IntoView, ReadSignal, WriteSignal, create_signal, create_effect, SignalGet, SignalGetUntracked, SignalSet, SignalUpdate, SignalWith, Callable, Fragment, Signal};
use crate::components::line_controls::LineControls;
use crate::components::line_editor::LineEditor;
use crate::components::button::Button;
use crate::components::importer::Importer;
use crate::components::settings::Settings;
use crate::models::{RailwayGraph, Line, LineFolder, ProjectSettings, GraphView};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;

type ViewFn = Box<dyn Fn() -> Fragment>;

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn Sidebar(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    folders: ReadSignal<Vec<LineFolder>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    settings: ReadSignal<ProjectSettings>,
    set_settings: WriteSignal<ProjectSettings>,
    on_create_view: leptos::Callback<GraphView>,
    on_line_editor_opened: leptos::Callback<uuid::Uuid>,
    on_line_editor_closed: leptos::Callback<uuid::Uuid>,
    sidebar_width: ReadSignal<f64>,
    set_sidebar_width: WriteSignal<f64>,
    #[prop(default = None)]
    on_width_change: Option<leptos::Callback<f64>>,
    #[prop(default = None)]
    on_open_changelog: Option<leptos::Callback<()>>,
    #[prop(default = None)]
    on_open_project_manager: Option<leptos::Callback<()>>,
    #[prop(default = None)]
    header_children: Option<ViewFn>,
    #[prop(default = None)]
    footer_children: Option<ViewFn>,
    #[prop(default = None)]
    open_editor_request: Option<ReadSignal<Option<(uuid::Uuid, String)>>>,
) -> impl IntoView {
    // Resize state
    let (is_resizing_sidebar, set_is_resizing_sidebar) = create_signal(false);
    let (resize_start_x, set_resize_start_x) = create_signal(0.0);

    // Line editor state
    let (new_line_dialog_open, set_new_line_dialog_open) = create_signal(false);
    let (next_line_number, set_next_line_number) = create_signal(1);
    let (resize_start_width, set_resize_start_width) = create_signal(0.0);
    let (is_hovering_resize_edge, set_is_hovering_resize_edge) = create_signal(false);

    // Mouse event handlers for sidebar resize
    let handle_sidebar_mousedown = move |ev: leptos::ev::MouseEvent| {
        let x = f64::from(ev.offset_x());
        let resize_handle_width = 3.0;

        // Check if mouse is near the left edge
        if x < resize_handle_width {
            set_is_resizing_sidebar.set(true);
            set_resize_start_x.set(f64::from(ev.client_x()));
            set_resize_start_width.set(sidebar_width.get());
            ev.prevent_default();
        }
    };

    let handle_sidebar_mousemove = move |ev: leptos::ev::MouseEvent| {
        // Check for hover (only when not resizing)
        if !is_resizing_sidebar.get() {
            let x = f64::from(ev.offset_x());
            let resize_handle_width = 3.0;
            set_is_hovering_resize_edge.set(x < resize_handle_width);
        }
    };

    let handle_sidebar_mouseleave = move |_ev: leptos::ev::MouseEvent| {
        set_is_hovering_resize_edge.set(false);
    };

    // Attach window-level event listeners when resizing starts
    create_effect(move |_| {
        if is_resizing_sidebar.get() {
            let window = leptos::window();

            // Handle mouse move
            let mousemove_closure = Closure::wrap(Box::new(move |ev: MouseEvent| {
                let client_x = f64::from(ev.client_x());
                let delta_x = resize_start_x.get_untracked() - client_x;
                let new_width = (resize_start_width.get_untracked() + delta_x).clamp(200.0, 600.0);
                set_sidebar_width.set(new_width);
            }) as Box<dyn FnMut(_)>);

            let _ = window.add_event_listener_with_callback(
                "mousemove",
                mousemove_closure.as_ref().unchecked_ref()
            );

            // Handle mouse up
            let mouseup_closure = Closure::wrap(Box::new(move |_ev: MouseEvent| {
                // Save sidebar width when resize completes
                if let Some(callback) = on_width_change {
                    callback.call(sidebar_width.get_untracked());
                }
                set_is_resizing_sidebar.set(false);
            }) as Box<dyn FnMut(_)>);

            let _ = window.add_event_listener_with_callback(
                "mouseup",
                mouseup_closure.as_ref().unchecked_ref()
            );

            // Return cleanup function to remove listeners when effect re-runs or component unmounts
            leptos::on_cleanup(move || {
                let window = leptos::window();
                let _ = window.remove_event_listener_with_callback(
                    "mousemove",
                    mousemove_closure.as_ref().unchecked_ref()
                );
                let _ = window.remove_event_listener_with_callback(
                    "mouseup",
                    mouseup_closure.as_ref().unchecked_ref()
                );
            });
        }
    });

    // Cursor style based on resize state
    let sidebar_cursor_style = move || {
        if is_resizing_sidebar.get() || is_hovering_resize_edge.get() {
            "cursor: col-resize;"
        } else {
            ""
        }
    };

    view! {
        <div
            class="sidebar"
            style=move || format!("width: {}px; {}", sidebar_width.get(), sidebar_cursor_style())
            on:mousedown=handle_sidebar_mousedown
            on:mousemove=handle_sidebar_mousemove
            on:mouseleave=handle_sidebar_mouseleave
        >
            <div class="sidebar-header">
                <h2>
                    <img src="/static/railgraph.svg" alt="RailGraph" class="logo-icon" />
                    "railgraph.app"
                </h2>
                {header_children.as_ref().map(|f| f())}
            </div>
            <LineControls
                lines=lines
                set_lines=set_lines
                folders=folders
                set_folders=set_folders
                graph=graph
                on_create_view=on_create_view
                settings=settings
                set_settings=set_settings
                on_line_editor_opened=on_line_editor_opened
                on_line_editor_closed=on_line_editor_closed
                open_editor_request=open_editor_request
            />
            <div class="sidebar-footer">
                <Button
                    class="import-button"
                    on_click=leptos::Callback::new(move |_| {
                        if let Some(callback) = on_open_project_manager {
                            callback.call(());
                        }
                    })
                    shortcut_id="manage_projects"
                    title="Manage Projects"
                >
                    <i class="fa-solid fa-folder"></i>
                </Button>
                <Button
                    class="import-button"
                    on_click=leptos::Callback::new(move |_| {
                        set_new_line_dialog_open.set(true);
                    })
                    shortcut_id="create_line"
                    title="Create new line"
                >
                    <i class="fa-solid fa-plus"></i>
                </Button>
                <Importer lines=lines set_lines=set_lines graph=graph set_graph=set_graph settings=settings />
                {footer_children.as_ref().map(|f| f())}
                <Settings
                    settings=leptos::Signal::derive(move || settings.get())
                    set_settings=move |s| set_settings.set(s)
                    on_open_changelog=move || {
                        if let Some(callback) = on_open_changelog {
                            callback.call(());
                        }
                    }
                />
            </div>

            <LineEditor
                initial_line=Signal::derive(move || {
                    if new_line_dialog_open.get() {
                        let line_num = next_line_number.get();
                        let line_id = format!("Line {line_num}");
                        let existing_line_count = lines.get().len();

                        Some(Line::create_from_ids(&[line_id], existing_line_count)[0].clone())
                    } else {
                        None
                    }
                })
                is_open=Signal::derive(move || new_line_dialog_open.get())
                set_is_open=move |open: bool| {
                    if open {
                        // Find next available line number when opening
                        let current_lines = lines.get();
                        let mut num = 1;
                        loop {
                            let candidate = format!("Line {num}");
                            if !current_lines.iter().any(|l| l.name == candidate) {
                                set_next_line_number.set(num);
                                break;
                            }
                            num += 1;
                        }
                        set_new_line_dialog_open.set(true);
                    } else {
                        set_new_line_dialog_open.set(false);
                    }
                }
                graph=graph
                on_save=move |mut new_line: Line| {
                    set_lines.update(|lines_vec| {
                        // Check if this is a new line or an existing one
                        if let Some(existing) = lines_vec.iter_mut().find(|l| l.id == new_line.id) {
                            // Update existing line
                            *existing = new_line;
                        } else {
                            // Add new line - assign sort_index if in Manual mode
                            if settings.with(|s| s.line_sort_mode == crate::models::LineSortMode::Manual) {
                                #[allow(clippy::cast_precision_loss)]
                                let max_sort_index = lines_vec
                                    .iter()
                                    .filter_map(|l| l.sort_index)
                                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                                    .unwrap_or(-1.0);
                                new_line.sort_index = Some(max_sort_index + 1.0);
                            }
                            lines_vec.push(new_line);
                        }
                    });
                }
                settings=settings
            />
        </div>
    }
}
