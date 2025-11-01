use crate::components::window::Window;
use crate::models::LineFolder;
use leptos::{ReadSignal, Signal, SignalGet, SignalSet, SignalUpdate, WriteSignal, component, view, IntoView, create_signal, event_target_value};

#[component]
#[must_use]
pub fn EditFolderDialog(
    folder_edit_pending: ReadSignal<Option<uuid::Uuid>>,
    set_folder_edit_pending: WriteSignal<Option<uuid::Uuid>>,
    folders: ReadSignal<Vec<LineFolder>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
) -> impl IntoView {
    view! {
        <Window
            is_open=Signal::derive(move || folder_edit_pending.get().is_some())
            title=Signal::derive(|| "Edit Folder".to_string())
            on_close=move || set_folder_edit_pending.set(None)
            max_size=(400.0, 300.0)
        >
            {move || {
                folder_edit_pending.get().and_then(|id| {
                    folders.get().into_iter().find(|f| f.id == id)
                }).map(|folder| {
                    let edit_folder_id = folder.id;
                    let (edit_name, set_edit_name) = create_signal(folder.name.clone());
                    let (edit_color, set_edit_color) = create_signal(folder.color.clone());

                    view! {
                        <div class="add-station-form">
                            <div class="form-field">
                                <label>"Folder Name"</label>
                                <input
                                    type="text"
                                    placeholder="Enter folder name"
                                    value=edit_name
                                    on:input=move |ev| set_edit_name.set(event_target_value(&ev))
                                    on:keydown=move |ev| {
                                        if ev.key() == "Enter" && !edit_name.get().trim().is_empty() {
                                            set_folders.update(|folders_vec| {
                                                if let Some(f) = folders_vec.iter_mut().find(|f| f.id == edit_folder_id) {
                                                    f.name = edit_name.get();
                                                    f.color = edit_color.get();
                                                }
                                            });
                                            set_folder_edit_pending.set(None);
                                        }
                                    }
                                    prop:autofocus=true
                                />
                            </div>

                            <div class="form-field">
                                <label>"Color"</label>
                                <input
                                    type="color"
                                    value=edit_color
                                    on:input=move |ev| set_edit_color.set(event_target_value(&ev))
                                />
                            </div>

                            <div class="form-buttons">
                                <button on:click=move |_| set_folder_edit_pending.set(None)>
                                    "Cancel"
                                </button>
                                <button
                                    class="primary"
                                    on:click=move |_| {
                                        if !edit_name.get().trim().is_empty() {
                                            set_folders.update(|folders_vec| {
                                                if let Some(f) = folders_vec.iter_mut().find(|f| f.id == edit_folder_id) {
                                                    f.name = edit_name.get();
                                                    f.color = edit_color.get();
                                                }
                                            });
                                            set_folder_edit_pending.set(None);
                                        }
                                    }
                                    prop:disabled=move || edit_name.get().trim().is_empty()
                                >
                                    "Save"
                                </button>
                            </div>
                        </div>
                    }
                })
            }}
        </Window>
    }
}
