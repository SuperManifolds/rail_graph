use crate::data::parse_csv_string;
use crate::models::{Line, RailwayGraph};
use crate::components::duration_input::DurationInput;
use crate::components::window::Window;
use leptos::{wasm_bindgen, component, view, WriteSignal, Props, IntoView, create_node_ref, create_signal, SignalGet, web_sys, spawn_local, SignalSet, Signal, SignalUpdate};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use chrono::Duration;
use std::collections::HashMap;

#[component]
#[must_use]
pub fn Importer(
    set_lines: WriteSignal<Vec<Line>>,
    set_graph: WriteSignal<RailwayGraph>,
) -> impl IntoView {
    let file_input_ref = create_node_ref::<leptos::html::Input>();
    let (show_dialog, set_show_dialog) = create_signal(false);
    let (csv_content, set_csv_content) = create_signal(String::new());
    let (line_ids, set_line_ids) = create_signal(Vec::<String>::new());
    let (wait_times, set_wait_times) = create_signal(HashMap::<String, Duration>::new());

    let handle_file_change = move |_| {
        let Some(input_elem) = file_input_ref.get() else { return };
        let input: &web_sys::HtmlInputElement = &input_elem;
        let Some(files) = input.files() else { return };
        let Some(file) = files.get(0) else { return };

        spawn_local(async move {
            let Ok(reader) = web_sys::FileReader::new() else {
                leptos::logging::error!("Failed to create FileReader");
                return;
            };
            let reader_clone = reader.clone();

            let onload = Closure::wrap(Box::new(move |_: web_sys::Event| {
                let Ok(result) = reader_clone.result() else {
                    return;
                };
                let Some(text) = result.as_string() else {
                    return;
                };

                // Extract line IDs from CSV header
                let mut reader = csv::ReaderBuilder::new()
                    .has_headers(false)
                    .from_reader(text.as_bytes());

                if let Some(Ok(header)) = reader.records().next() {
                    let ids: Vec<String> = header.iter()
                        .skip(1)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();

                    // Initialize wait times with 30 second default
                    let times: HashMap<String, Duration> = ids.iter()
                        .map(|id| (id.clone(), Duration::seconds(30)))
                        .collect();

                    set_line_ids.set(ids);
                    set_wait_times.set(times);
                }

                set_csv_content.set(text);
                set_show_dialog.set(true);
            }) as Box<dyn FnMut(_)>);

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            let _ = reader.read_as_text(&file);
        });
    };

    let handle_import = move |_| {
        let (new_lines, new_graph) = parse_csv_string(&csv_content.get(), wait_times.get());
        set_lines.set(new_lines);
        set_graph.set(new_graph);
        set_show_dialog.set(false);
        // Reset file input
        if let Some(input) = file_input_ref.get() {
            input.set_value("");
        }
    };

    view! {
        <input
            type="file"
            accept=".csv"
            node_ref=file_input_ref
            on:change=handle_file_change
            style="display: none;"
        />
        <button
            class="import-button"
            on:click=move |_| {
                if let Some(input) = file_input_ref.get() {
                    input.click();
                }
            }
            title="Import CSV"
        >
            <i class="fa-solid fa-file-import"></i>
        </button>

        <Window
            is_open=show_dialog
            title=Signal::derive(|| "Import Settings".to_string())
            on_close=move || set_show_dialog.set(false)
        >
            <p>"Set station wait time for each line (deducted from travel time)"</p>
            <div class="import-settings-list">
                {move || {
                    line_ids.get().iter().map(|line_id| {
                        let line_id_clone = line_id.clone();
                        let line_id_for_change = line_id.clone();
                        view! {
                            <div class="import-setting">
                                <label>{line_id.clone()}</label>
                                <DurationInput
                                    duration=Signal::derive(move || {
                                        wait_times.get().get(&line_id_clone).copied().unwrap_or(Duration::seconds(30))
                                    })
                                    on_change=move |new_duration| {
                                        set_wait_times.update(|times| {
                                            times.insert(line_id_for_change.clone(), new_duration);
                                        });
                                    }
                                />
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
            <div class="import-dialog-buttons">
                <button on:click=move |_| set_show_dialog.set(false)>"Cancel"</button>
                <button class="primary" on:click=handle_import>"Import"</button>
            </div>
        </Window>
    }
}
