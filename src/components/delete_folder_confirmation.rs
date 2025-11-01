use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::models::{Line, LineFolder};
use leptos::{ReadSignal, Signal, SignalGet, SignalSet, SignalUpdate, WriteSignal, component, view, IntoView};
use std::rc::Rc;

#[component]
#[must_use]
pub fn DeleteFolderConfirmation(
    folder_delete_pending: ReadSignal<Option<uuid::Uuid>>,
    set_folder_delete_pending: WriteSignal<Option<uuid::Uuid>>,
    folders: ReadSignal<Vec<LineFolder>>,
    set_folders: WriteSignal<Vec<LineFolder>>,
    set_lines: WriteSignal<Vec<Line>>,
) -> impl IntoView {
    view! {
        <ConfirmationDialog
            is_open=Signal::derive(move || folder_delete_pending.get().is_some())
            title=Signal::derive(|| "Delete Folder".to_string())
            message=Signal::derive(move || {
                folder_delete_pending.get()
                    .and_then(|id| folders.get().into_iter().find(|f| f.id == id))
                    .map(|folder| format!("Are you sure you want to delete folder \"{}\"? Lines and subfolders will be moved to the parent folder.", folder.name))
                    .unwrap_or_default()
            })
            on_confirm=Rc::new(move || {
                if let Some(id) = folder_delete_pending.get() {
                    let parent_folder_id = folders.get().into_iter()
                        .find(|f| f.id == id)
                        .and_then(|f| f.parent_folder_id);

                    // Move lines in this folder to parent
                    set_lines.update(|lines_vec| {
                        for line in lines_vec.iter_mut() {
                            if line.folder_id == Some(id) {
                                line.folder_id = parent_folder_id;
                            }
                        }
                    });

                    // Move subfolders to parent
                    set_folders.update(|folders_vec| {
                        for folder in folders_vec.iter_mut() {
                            if folder.parent_folder_id == Some(id) {
                                folder.parent_folder_id = parent_folder_id;
                            }
                        }
                        folders_vec.retain(|f| f.id != id);
                    });

                    set_folder_delete_pending.set(None);
                }
            })
            on_cancel=Rc::new(move || set_folder_delete_pending.set(None))
            confirm_text="Delete".to_string()
        />
    }
}
