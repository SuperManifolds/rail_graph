use crate::import::jtraingraph::{parse_jtraingraph, import_jtraingraph};
use crate::models::{Line, RailwayGraph};
use crate::components::button::Button;
use crate::components::csv_column_mapper::CsvColumnMapper;
use crate::components::window::Window;
use crate::import::csv::{analyze_csv, parse_csv_with_mapping, CsvImportConfig};
use leptos::{component, view, WriteSignal, ReadSignal, IntoView, create_node_ref, create_signal, SignalGet, web_sys, spawn_local, SignalSet, Signal, SignalUpdate, Callback, Show};

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
    let existing_line_ids: Vec<String> = lines.get().iter().map(|l| l.name.clone()).collect();

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

fn handle_csv_analysis(
    text: &str,
    set_csv_config: WriteSignal<Option<CsvImportConfig>>,
    set_show_mapper: WriteSignal<bool>,
) {
    leptos::logging::log!("Analyzing CSV file, length: {}", text.len());
    if let Some(config) = analyze_csv(text) {
        leptos::logging::log!("CSV analysis successful, {} columns detected", config.columns.len());
        set_csv_config.set(Some(config));
        set_show_mapper.set(true);
    } else {
        leptos::logging::error!("Failed to analyze CSV file");
    }
}

#[component]
#[must_use]
pub fn Importer(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_graph: WriteSignal<RailwayGraph>,
) -> impl IntoView {
    let file_input_ref = create_node_ref::<leptos::html::Input>();
    let (show_mapper, set_show_mapper) = create_signal(false);
    let (file_content, set_file_content) = create_signal(String::new());
    let (csv_config, set_csv_config) = create_signal(None::<CsvImportConfig>);

    let handle_file_change = move |_| {
        let Some(input_elem) = file_input_ref.get() else { return };
        let input: &web_sys::HtmlInputElement = &input_elem;
        let Some(files) = input.files() else { return };
        let Some(file) = files.get(0) else { return };

        let filename = file.name();
        let file_clone = file.clone();

        spawn_local(async move {
            leptos::logging::log!("Reading file: {}", filename);
            let text = match wasm_bindgen_futures::JsFuture::from(file_clone.text()).await {
                Ok(val) => {
                    if let Some(s) = val.as_string() {
                        s
                    } else {
                        leptos::logging::error!("Failed to convert file content to string");
                        return;
                    }
                }
                Err(e) => {
                    leptos::logging::error!("Failed to read file: {:?}", e);
                    return;
                }
            };

            set_file_content.set(text.clone());

            // Check file type by extension
            let is_fpl = std::path::Path::new(&filename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("fpl"));

            leptos::logging::log!("File type: {}", if is_fpl { "FPL" } else { "CSV" });

            if is_fpl {
                handle_fpl_import(&text, set_graph, set_lines, lines);
            } else {
                handle_csv_analysis(&text, set_csv_config, set_show_mapper);
            }
        });
    };

    let handle_import = move |config: CsvImportConfig| {
        let mut new_lines = None;

        // Get existing line count for color offset
        let existing_line_count = lines.get().len();

        // Parse CSV into existing graph
        set_graph.update(|graph| {
            let lines = parse_csv_with_mapping(&file_content.get(), &config, graph, existing_line_count);
            new_lines = Some(lines);
        });

        // Add new lines to existing lines
        if let Some(lines) = new_lines {
            set_lines.update(|existing_lines| {
                existing_lines.extend(lines);
            });
        }

        set_show_mapper.set(false);
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
        <Button
            class="import-button"
            on_click=Callback::new(move |_| {
                spawn_local(async move {
                    if let Some(input) = file_input_ref.get() {
                        input.set_value("");
                        input.click();
                    }
                });
            })
            shortcut="O"
            title="Import CSV or JTrainGraph (.fpl)"
        >
            <i class="fa-solid fa-file-import"></i>
        </Button>

        <Show when=move || csv_config.get().is_some()>
            <Window
                is_open=show_mapper
                title=Signal::derive(|| "CSV Column Mapping".to_string())
                on_close=move || set_show_mapper.set(false)
            >
                <CsvColumnMapper
                    config=Signal::derive(move || csv_config.get().unwrap_or_else(|| {
                        use std::collections::HashMap;
                        CsvImportConfig {
                            columns: Vec::new(),
                            has_headers: false,
                            defaults: crate::import::csv::ImportDefaults::default(),
                            pattern_repeat: None,
                            group_line_names: HashMap::new(),
                        }
                    }))
                    on_cancel=Callback::new(move |()| set_show_mapper.set(false))
                    on_import=Callback::new(handle_import)
                />
            </Window>
        </Show>
    }
}
