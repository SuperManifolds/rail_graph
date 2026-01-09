use crate::import::jtraingraph::{parse_jtraingraph, import_jtraingraph};
use crate::import::nimby::{parse_nimby_json, import_nimby_lines, NimbyImportData, NimbyImportConfig};
use crate::models::{Line, RailwayGraph};
use crate::components::button::Button;
use crate::components::csv_column_mapper::CsvColumnMapper;
use crate::components::nimby_line_selector::NimbyLineSelector;
use crate::components::window::Window;
use crate::import::csv::{analyze_csv, parse_csv_with_mapping, parse_csv_with_existing_infrastructure, CsvImportConfig};
use leptos::{component, view, WriteSignal, ReadSignal, IntoView, create_node_ref, create_signal, SignalGet, SignalGetUntracked, web_sys, spawn_local, SignalSet, Signal, SignalUpdate, Callback, Show};

fn handle_fpl_import(
    text: &str,
    set_graph: WriteSignal<RailwayGraph>,
    set_lines: WriteSignal<Vec<Line>>,
    lines: ReadSignal<Vec<Line>>,
    handedness: crate::models::TrackHandedness,
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

        match import_jtraingraph(&timetable, graph, before_lines_count, &existing_line_ids, handedness) {
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
    filename: String,
    set_csv_config: WriteSignal<Option<CsvImportConfig>>,
    set_show_mapper: WriteSignal<bool>,
    set_import_error: WriteSignal<Option<String>>,
) {
    leptos::logging::log!("Analyzing CSV file, length: {}", text.len());

    // Clear any previous import errors
    set_import_error.set(None);

    // Extract filename without extension
    let filename_without_ext = std::path::Path::new(&filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(String::from);

    if let Some(config) = analyze_csv(text, filename_without_ext) {
        leptos::logging::log!("CSV analysis successful, {} columns detected", config.columns.len());
        set_csv_config.set(Some(config));
        set_show_mapper.set(true);
    } else {
        leptos::logging::error!("Failed to analyze CSV file");
    }
}

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn Importer(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    settings: ReadSignal<crate::models::ProjectSettings>,
) -> impl IntoView {
    let file_input_ref = create_node_ref::<leptos::html::Input>();
    let (show_mapper, set_show_mapper) = create_signal(false);
    let (file_content, set_file_content) = create_signal(String::new());
    let (csv_config, set_csv_config) = create_signal(None::<CsvImportConfig>);
    let (import_error, set_import_error) = create_signal(None::<String>);

    // NIMBY Rails import state
    let (nimby_data, set_nimby_data) = create_signal(None::<NimbyImportData>);
    let (show_nimby_selector, set_show_nimby_selector) = create_signal(false);

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
            let extension = std::path::Path::new(&filename)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_ascii_lowercase);

            leptos::logging::log!("File type: {:?}", extension);

            match extension.as_deref() {
                Some("fpl") => {
                    let handedness = settings.get_untracked().track_handedness;
                    handle_fpl_import(&text, set_graph, set_lines, lines, handedness);
                }
                Some("json") => {
                    // NIMBY Rails JSON import
                    match parse_nimby_json(&text) {
                        Ok(data) => {
                            leptos::logging::log!("NIMBY JSON parsed: {} stations, {} lines", data.stations.len(), data.lines.len());
                            set_nimby_data.set(Some(data));
                            set_show_nimby_selector.set(true);
                            set_import_error.set(None);
                        }
                        Err(e) => {
                            leptos::logging::error!("Failed to parse NIMBY JSON: {}", e);
                            set_import_error.set(Some(e));
                        }
                    }
                }
                _ => {
                    // Default to CSV
                    handle_csv_analysis(&text, filename.clone(), set_csv_config, set_show_mapper, set_import_error);
                }
            }
        });
    };

    let handle_import = move |config: CsvImportConfig| {
        // Clear any previous import errors
        set_import_error.set(None);

        let mut new_lines = None;
        let mut error_msg = None;

        // Get existing line count for color offset
        let existing_line_count = lines.get().len();
        let handedness = settings.get().track_handedness;

        // Get owned copy of graph, mutate it, then set it back (triggers reactivity)
        let mut current_graph = graph.get();

        if config.disable_infrastructure {
            // Use pathfinding mode
            match parse_csv_with_existing_infrastructure(&file_content.get(), &config, &mut current_graph, existing_line_count, handedness) {
                Ok(lines) => new_lines = Some(lines),
                Err(e) => error_msg = Some(e),
            }
        } else {
            // Use normal mode (creates infrastructure)
            let lines = parse_csv_with_mapping(&file_content.get(), &config, &mut current_graph, existing_line_count, handedness);
            new_lines = Some(lines);
        }

        // Handle errors
        if let Some(error) = error_msg {
            leptos::logging::error!("CSV import failed: {}", error);
            set_import_error.set(Some(error));
            return;
        }

        // Set modified graph back to signal
        set_graph.set(current_graph);

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

    let handle_nimby_import = move |config: NimbyImportConfig| {
        set_import_error.set(None);

        let Some(data) = nimby_data.get() else {
            set_import_error.set(Some("No NIMBY data available".to_string()));
            return;
        };

        let existing_line_count = lines.get().len();
        let mut current_graph = graph.get();

        // If update_existing mode, pass existing lines for in-place modification
        if config.update_existing {
            let mut current_lines = lines.get();
            match import_nimby_lines(&data, &config, &mut current_graph, existing_line_count, Some(&mut current_lines)) {
                Ok(new_lines) => {
                    leptos::logging::log!("Updated lines, {} new lines created", new_lines.len());
                    set_graph.set(current_graph);
                    // Replace all lines (updated ones are modified in place)
                    current_lines.extend(new_lines);
                    set_lines.set(current_lines);
                    set_show_nimby_selector.set(false);
                    set_nimby_data.set(None);
                    if let Some(input) = file_input_ref.get() {
                        input.set_value("");
                    }
                }
                Err(e) => {
                    leptos::logging::error!("NIMBY import failed: {}", e);
                    set_import_error.set(Some(e));
                }
            }
        } else {
            match import_nimby_lines(&data, &config, &mut current_graph, existing_line_count, None) {
                Ok(imported_lines) => {
                    leptos::logging::log!("Imported {} lines from NIMBY JSON", imported_lines.len());
                    set_graph.set(current_graph);
                    set_lines.update(|existing| existing.extend(imported_lines));
                    set_show_nimby_selector.set(false);
                    set_nimby_data.set(None);
                    if let Some(input) = file_input_ref.get() {
                        input.set_value("");
                    }
                }
                Err(e) => {
                    leptos::logging::error!("NIMBY import failed: {}", e);
                    set_import_error.set(Some(e));
                }
            }
        }
    };

    view! {
        <input
            type="file"
            accept=".csv,.fpl,.json"
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
            shortcut_id="import_data"
            title="Import CSV, JTrainGraph (.fpl), or NIMBY Rails (.json)"
        >
            <i class="fa-solid fa-file-import"></i>
        </Button>

        <Show when=move || csv_config.get().is_some()>
            <Window
                is_open=show_mapper
                title=Signal::derive(|| "CSV Column Mapping".to_string())
                on_close=move || set_show_mapper.set(false)
                position_key="importer"
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
                            filename: None,
                            disable_infrastructure: false,
                        }
                    }))
                    on_cancel=Callback::new(move |()| {
                        set_show_mapper.set(false);
                        set_import_error.set(None);
                    })
                    on_import=Callback::new(handle_import)
                    import_error=import_error
                />
            </Window>
        </Show>

        <Show when=move || nimby_data.get().is_some()>
            <Window
                is_open=show_nimby_selector
                title=Signal::derive(|| "Import NIMBY Rails Schedules".to_string())
                on_close=move || {
                    set_show_nimby_selector.set(false);
                    set_nimby_data.set(None);
                    set_import_error.set(None);
                }
                position_key="nimby_importer"
            >
                <NimbyLineSelector
                    data=Signal::derive(move || nimby_data.get().unwrap_or_default())
                    handedness=Signal::derive(move || settings.get().track_handedness)
                    station_spacing=Signal::derive(move || {
                        const GRID_SIZE: f64 = 30.0;
                        settings.get().default_node_distance_grid_squares * GRID_SIZE
                    })
                    on_cancel=Callback::new(move |()| {
                        set_show_nimby_selector.set(false);
                        set_nimby_data.set(None);
                        set_import_error.set(None);
                    })
                    on_import=Callback::new(handle_nimby_import)
                    import_error=import_error
                />
            </Window>
        </Show>
    }
}
