use crate::data::parse_csv_string;
use crate::jtraingraph::{parse_jtraingraph, import_jtraingraph};
use crate::models::{Line, RailwayGraph};
use crate::components::duration_input::DurationInput;
use crate::components::window::Window;
use leptos::{wasm_bindgen, component, view, WriteSignal, ReadSignal, IntoView, create_node_ref, create_signal, SignalGet, web_sys, spawn_local, SignalSet, Signal, SignalUpdate};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use chrono::Duration;
use std::collections::HashMap;

fn handle_fpl_import(
    text: &str,
    set_graph: WriteSignal<RailwayGraph>,
    set_lines: WriteSignal<Vec<Line>>,
    lines: ReadSignal<Vec<Line>>,
) {
    let Ok(timetable) = parse_jtraingraph(text) else {
        leptos::logging::error!("Failed to parse JTrainGraph file");
        return;
    };

    // Get current line info before updating
    let before_lines_count = lines.get().len();
    let existing_line_ids: Vec<String> = lines.get().iter().map(|l| l.id.clone()).collect();

    // Track results
    let mut new_lines = None;
    let mut before_stations = 0;
    let mut after_stations = 0;

    // Update graph and get new lines to add
    set_graph.update(|graph| {
        before_stations = graph.graph.node_count();

        match import_jtraingraph(&timetable, graph, before_lines_count, &existing_line_ids) {
            Ok(lines_to_add) => {
                after_stations = graph.graph.node_count();
                new_lines = Some(lines_to_add);
            }
            Err(e) => {
                leptos::logging::error!("Failed to import JTrainGraph: {}", e);
                after_stations = before_stations;
            }
        }
    });

    // Add new lines if import succeeded
    if let Some(lines_to_add) = new_lines {
        set_lines.update(|lines| lines.extend(lines_to_add));
    }
}

fn handle_csv_preview(
    text: &str,
    set_line_ids: WriteSignal<Vec<String>>,
    set_wait_times: WriteSignal<HashMap<String, Duration>>,
    set_show_dialog: WriteSignal<bool>,
) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());

    if let Some(Ok(header)) = reader.records().next() {
        let ids: Vec<String> = header.iter()
            .skip(1)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();

        let times: HashMap<String, Duration> = ids.iter()
            .map(|id| (id.clone(), Duration::seconds(30)))
            .collect();

        set_line_ids.set(ids);
        set_wait_times.set(times);
    }

    set_show_dialog.set(true);
}

#[component]
#[must_use]
pub fn Importer(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_graph: WriteSignal<RailwayGraph>,
) -> impl IntoView {
    let file_input_ref = create_node_ref::<leptos::html::Input>();
    let (show_dialog, set_show_dialog) = create_signal(false);
    let (file_content, set_file_content) = create_signal(String::new());
    let (line_ids, set_line_ids) = create_signal(Vec::<String>::new());
    let (wait_times, set_wait_times) = create_signal(HashMap::<String, Duration>::new());

    let handle_file_change = move |_| {
        let Some(input_elem) = file_input_ref.get() else { return };
        let input: &web_sys::HtmlInputElement = &input_elem;
        let Some(files) = input.files() else { return };
        let Some(file) = files.get(0) else { return };

        let filename = file.name();

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

                set_file_content.set(text.clone());

                // Check file type by extension
                let is_fpl = std::path::Path::new(&filename)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("fpl"));

                if is_fpl {
                    handle_fpl_import(&text, set_graph, set_lines, lines);
                } else {
                    handle_csv_preview(&text, set_line_ids, set_wait_times, set_show_dialog);
                }
            }) as Box<dyn FnMut(_)>);

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            let _ = reader.read_as_text(&file);
        });
    };

    let handle_import = move |_| {
        let (new_lines, new_graph) = parse_csv_string(&file_content.get(), &wait_times.get());
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
            accept=".csv,.fpl"
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
            title="Import CSV or JTrainGraph (.fpl)"
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
