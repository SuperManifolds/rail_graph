use crate::import::{Import, ImportMode};
use crate::import::jtraingraph::{JTrainGraphImport, JTrainGraphConfig};
use crate::import::csv::{CsvImport, CsvImportConfig};
use crate::models::{Line, RailwayGraph};
use crate::components::button::Button;
use crate::components::csv_column_mapper::CsvColumnMapper;
use crate::components::window::Window;
use leptos::{component, view, WriteSignal, ReadSignal, IntoView, create_node_ref, create_signal, SignalGet, SignalGetUntracked, web_sys, spawn_local, SignalSet, Signal, SignalUpdate, Callback, Show};

fn handle_fpl_import(
    text: &str,
    set_graph: WriteSignal<RailwayGraph>,
    set_lines: WriteSignal<Vec<Line>>,
    lines: ReadSignal<Vec<Line>>,
    handedness: crate::models::TrackHandedness,
) {
    let Ok(parsed) = JTrainGraphImport::parse(text) else {
        leptos::logging::error!("Failed to parse JTrainGraph file");
        return;
    };

    // Get current line info before updating
    let before_lines_count = lines.get().len();
    let existing_line_ids: Vec<String> = lines.get().iter().map(|l| l.name.clone()).collect();

    // Track results
    let mut import_result = None;

    // Update graph and get new lines to add
    set_graph.update(|graph| {
        let config = JTrainGraphConfig;
        match JTrainGraphImport::import(
            &parsed,
            &config,
            ImportMode::CreateInfrastructure,
            graph,
            before_lines_count,
            &existing_line_ids,
            handedness,
        ) {
            Ok(result) => {
                leptos::logging::log!(
                    "JTrainGraph import successful: {} lines, {} stations added, {} edges added",
                    result.lines.len(),
                    result.stations_added,
                    result.edges_added
                );
                import_result = Some(result);
            }
            Err(e) => {
                leptos::logging::error!("Failed to import JTrainGraph: {}", e);
            }
        }
    });

    // Add new lines if import succeeded
    if let Some(result) = import_result {
        set_lines.update(|lines| lines.extend(result.lines));
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

    if let Some(config) = CsvImport::analyze(text, filename_without_ext) {
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
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    settings: ReadSignal<crate::models::ProjectSettings>,
) -> impl IntoView {
    let file_input_ref = create_node_ref::<leptos::html::Input>();
    let (show_mapper, set_show_mapper) = create_signal(false);
    let (file_content, set_file_content) = create_signal(String::new());
    let (csv_config, set_csv_config) = create_signal(None::<CsvImportConfig>);
    let (import_error, set_import_error) = create_signal(None::<String>);

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
                let handedness = settings.get_untracked().track_handedness;
                handle_fpl_import(&text, set_graph, set_lines, lines, handedness);
            } else {
                handle_csv_analysis(&text, filename.clone(), set_csv_config, set_show_mapper, set_import_error);
            }
        });
    };

    let handle_import = move |config: CsvImportConfig| {
        // Clear any previous import errors
        set_import_error.set(None);

        // Get existing line count and IDs
        let existing_line_count = lines.get().len();
        let existing_line_ids: Vec<String> = lines.get().iter().map(|l| l.name.clone()).collect();
        let handedness = settings.get().track_handedness;

        // Get owned copy of graph, mutate it, then set it back (triggers reactivity)
        let mut current_graph = graph.get();

        // Determine import mode from config
        let mode = if config.disable_infrastructure {
            ImportMode::UseExisting
        } else {
            ImportMode::CreateInfrastructure
        };

        // Import using trait-based API
        match CsvImport::import_from_content(
            &file_content.get(),
            &config,
            mode,
            &mut current_graph,
            existing_line_count,
            &existing_line_ids,
            handedness,
        ) {
            Ok(result) => {
                leptos::logging::log!(
                    "CSV import successful: {} lines, {} stations added, {} edges added",
                    result.lines.len(),
                    result.stations_added,
                    result.edges_added
                );

                // Set modified graph back to signal
                set_graph.set(current_graph);

                // Add new lines to existing lines
                set_lines.update(|existing_lines| {
                    existing_lines.extend(result.lines);
                });

                set_show_mapper.set(false);
                // Reset file input
                if let Some(input) = file_input_ref.get() {
                    input.set_value("");
                }
            }
            Err(e) => {
                leptos::logging::error!("CSV import failed: {}", e);
                set_import_error.set(Some(e));
            }
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
            shortcut_id="import_data"
            title="Import CSV or JTrainGraph (.fpl)"
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
    }
}
