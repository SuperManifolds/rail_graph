use leptos::*;
use crate::models::Line;
use crate::components::line_editor::LineEditor;


#[component]
pub fn LineControls(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
) -> impl IntoView {
    let (is_editor_open, set_is_editor_open) = create_signal(false);
    let (editing_line_id, set_editing_line_id) = create_signal(None::<String>);

    let current_editing_line = Signal::derive(move || {
        editing_line_id.get().and_then(|id| {
            lines.get().into_iter().find(|l| l.id == id)
        })
    });

    view! {
        <div class="controls">
            <h3>"Line Configuration:"</h3>
            <div class="line-controls">
                {move || {
                    lines.get().into_iter().map(|line| {
                        let line_id = line.id.clone();
                        view! {
                            <LineControl
                                line_id=line_id.clone()
                                lines=lines
                                set_lines=set_lines
                                on_edit=move |id: String| {
                                    set_editing_line_id.set(Some(id));
                                    set_is_editor_open.set(true);
                                }
                            />
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </div>

        <LineEditor
            initial_line=current_editing_line
            is_open=is_editor_open
            set_is_open=set_is_editor_open
            on_save={
                move |edited_line: Line| {
                    set_lines.update(|lines_vec| {
                        if let Some(line) = lines_vec.iter_mut().find(|l| l.id == edited_line.id) {
                            *line = edited_line;
                        }
                    });
                }
            }
        />
    }
}

#[component]
pub fn LineControl(
    line_id: String,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    on_edit: impl Fn(String) + 'static,
) -> impl IntoView {
    let id_for_derive = line_id.clone();
    let current_line = Signal::derive(move || {
        lines.get().into_iter().find(|l| l.id == id_for_derive)
    });

    let id_for_edit = line_id.clone();
    let on_edit = store_value(on_edit);

    view! {
        {move || {
            current_line.get().map(|line| {
                let id_for_edit = id_for_edit.clone();
                view! {
                    <div
                        class="line-control"
                        style=format!("border-left: 4px solid {}", line.color)
                    >
                        <div class="line-header">
                            <strong>{line_id.clone()}</strong>
                            <div class="line-header-controls">
                                <button
                                    class="visibility-toggle"
                                    on:click={
                                        let id = line_id.clone();
                                        move |_| {
                                            set_lines.update(|lines_vec| {
                                                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                                    line.visible = !line.visible;
                                                }
                                            });
                                        }
                                    }
                                    title=if line.visible { "Hide line" } else { "Show line" }
                                >
                                    <i class=if line.visible { "fa-solid fa-eye" } else { "fa-solid fa-eye-slash" }></i>
                                </button>
                                <button
                                    class="edit-button"
                                    on:click=move |_| on_edit.with_value(|f| f(id_for_edit.clone()))
                                    title="Edit line"
                                >
                                    <i class="fa-solid fa-pen"></i>
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })
        }}
    }
}