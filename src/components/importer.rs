use crate::components::button::Button;
use crate::components::csv_column_mapper::CsvColumnMapper;
use crate::components::geojson_region_selector::{
    GeoJsonRegionSelector, SelectionBounds, StationData,
};
use crate::components::window::Window;
use crate::import::csv::{CsvImport, CsvImportConfig};
use crate::import::geojson::{GeoJsonConfig, GeoJsonImport};
use crate::import::jtraingraph::{JTrainGraphConfig, JTrainGraphImport};
use crate::import::{Import, ImportMode};
use crate::models::{Line, RailwayGraph};
use leptos::{
    component, create_node_ref, create_signal, spawn_local, view, web_sys, Callback, IntoView,
    ReadSignal, Show, Signal, SignalGet, SignalGetUntracked, SignalSet, SignalUpdate, WriteSignal,
};

#[cfg(target_arch = "wasm32")]
use crate::import::geojson::{GeoJsonImportRequest, GraphUpdate};
#[cfg(target_arch = "wasm32")]
use crate::models::{Stations, Track, TrackDirection, Tracks};
#[cfg(target_arch = "wasm32")]
use crate::worker_bridge::GeoJsonImporter;

struct FileProcessorSignals {
    set_file_content: WriteSignal<String>,
    set_graph: WriteSignal<RailwayGraph>,
    set_lines: WriteSignal<Vec<Line>>,
    lines: ReadSignal<Vec<Line>>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_geojson_string: WriteSignal<Option<String>>,
    set_geojson_stations: WriteSignal<Vec<StationData>>,
    set_csv_config: WriteSignal<Option<CsvImportConfig>>,
    set_show_mapper: WriteSignal<bool>,
    set_import_error: WriteSignal<Option<String>>,
}

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

fn handle_geojson_analysis(
    text: &str,
    set_geojson_string: WriteSignal<Option<String>>,
    set_geojson_stations: WriteSignal<Vec<StationData>>,
) {
    leptos::logging::log!("Parsing GeoJSON for preview...");

    let Ok(parsed) = GeoJsonImport::parse(text) else {
        leptos::logging::error!("Failed to parse GeoJSON file");
        return;
    };

    leptos::logging::log!("GeoJSON parsed successfully");

    // Store raw string (not parsed, to avoid serialization overhead later)
    set_geojson_string.set(Some(text.to_string()));

    leptos::logging::log!("Extracting stations from GeoJSON...");

    match GeoJsonImport::extract_stations(&parsed) {
        Ok(stations) => {
            leptos::logging::log!("GeoJSON contains {} stations", stations.len());

            // Convert to StationData for the selector component
            let station_data: Vec<StationData> = stations
                .into_iter()
                .map(|s| StationData {
                    name: s.name,
                    lat: s.lat,
                    lng: s.lng,
                })
                .collect();

            // Update signal with loaded stations
            set_geojson_stations.set(station_data);
        }
        Err(e) => {
            leptos::logging::error!("Failed to extract stations from GeoJSON: {}", e);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn handle_geojson_import(
    geojson_string: String,
    bounds: SelectionBounds,
    set_graph: WriteSignal<RailwayGraph>,
    set_show_geojson_selector: WriteSignal<bool>,
) {
    let config = GeoJsonConfig {
        create_infrastructure: true,
        bounds: Some((
            bounds.min_lat,
            bounds.min_lng,
            bounds.max_lat,
            bounds.max_lng,
        )),
    };

    let request = GeoJsonImportRequest {
        geojson_string,
        config,
    };

    let mut worker = GeoJsonImporter::new(move |response| {
        match response.result {
            Ok(()) => {
                leptos::logging::log!(
                    "GeoJSON import completed: {} stations, {} edges",
                    response.stations_added,
                    response.edges_added
                );

                use leptos::SignalUpdateUntracked;
                set_graph.update_untracked(|g| {
                    apply_graph_updates(g, &response.updates);
                });

                set_show_geojson_selector.set(false);
            }
            Err(e) => {
                leptos::logging::error!("GeoJSON import failed: {}", e);
            }
        }
    });

    worker.import(request);

    // Keep worker alive until it responds
    std::mem::forget(worker);
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_geojson_import(
    geojson_string: String,
    bounds: SelectionBounds,
    set_graph: WriteSignal<RailwayGraph>,
    set_show_geojson_selector: WriteSignal<bool>,
) {
    // Fallback for non-WASM (tests) - parse directly
    let Ok(parsed) = GeoJsonImport::parse(&geojson_string) else {
        leptos::logging::error!("Failed to parse GeoJSON");
        return;
    };

    let config = GeoJsonConfig {
        create_infrastructure: true,
        bounds: Some((
            bounds.min_lat,
            bounds.min_lng,
            bounds.max_lat,
            bounds.max_lng,
        )),
    };

    set_graph.update(|graph| {
        match GeoJsonImport::import(
            &parsed,
            &config,
            ImportMode::CreateInfrastructure,
            graph,
            0,
            &[],
            crate::models::TrackHandedness::RightHand,
        ) {
            Ok(result) => {
                leptos::logging::log!(
                    "GeoJSON import successful: {} stations added, {} edges added",
                    result.stations_added,
                    result.edges_added
                );
                set_show_geojson_selector.set(false);
            }
            Err(e) => {
                leptos::logging::error!("GeoJSON import failed: {}", e);
            }
        }
    });
}

/// Apply graph updates received from the worker
#[cfg(target_arch = "wasm32")]
fn apply_graph_updates(graph: &mut RailwayGraph, updates: &[GraphUpdate]) {
    use std::collections::HashMap;

    // Build station ID -> NodeIndex map first (add all stations)
    let mut station_map = HashMap::new();

    for update in updates {
        if let GraphUpdate::AddStation { id, name, position } = update {
            let idx = graph.add_or_get_station(id.clone());
            if let Some(crate::models::Node::Station(ref mut station)) =
                graph.graph.node_weight_mut(idx)
            {
                station.name = name.clone();
            }
            graph.set_station_position(idx, *position);
            station_map.insert(id.clone(), idx);
        }
    }

    // Now apply track updates using the map (avoid repeated searches)
    for update in updates {
        match update {
            GraphUpdate::AddStation { .. } => {
                // Already handled above
            }
            GraphUpdate::AddTrack {
                start_id,
                end_id,
                bidirectional,
            } => {
                let start_idx = *station_map.get(start_id).expect("Station should exist");
                let end_idx = *station_map.get(end_id).expect("Station should exist");

                let direction = if *bidirectional {
                    TrackDirection::Bidirectional
                } else {
                    TrackDirection::Bidirectional
                };

                let track = Track { direction };
                graph.add_track(start_idx, end_idx, vec![track]);
            }
            GraphUpdate::AddParallelTrack {
                start_id,
                end_id,
                bidirectional,
            } => {
                let start_idx = *station_map.get(start_id).expect("Station should exist");
                let end_idx = *station_map.get(end_id).expect("Station should exist");

                if let Some(edge_idx) = graph.graph.find_edge(start_idx, end_idx) {
                    if let Some(edge_weight) = graph.graph.edge_weight_mut(edge_idx) {
                        let direction = if *bidirectional {
                            TrackDirection::Bidirectional
                        } else {
                            TrackDirection::Bidirectional
                        };

                        let track = Track { direction };
                        edge_weight.tracks.push(track);
                    }
                }
            }
        }
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
        leptos::logging::log!(
            "CSV analysis successful, {} columns detected",
            config.columns.len()
        );
        set_csv_config.set(Some(config));
        set_show_mapper.set(true);
    } else {
        leptos::logging::error!("Failed to analyze CSV file");
    }
}

async fn process_file(file: web_sys::File, signals: FileProcessorSignals) {
    let filename = file.name();
    leptos::logging::log!("Reading file: {filename}");

    let text = match wasm_bindgen_futures::JsFuture::from(file.text()).await {
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

    signals.set_file_content.set(text.clone());

    // Check file type by extension
    let extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase);

    let file_type = match extension.as_deref() {
        Some("fpl") => "FPL",
        Some("geojson" | "json") => "GeoJSON",
        _ => "CSV",
    };

    leptos::logging::log!("File type: {file_type}");

    match file_type {
        "FPL" => {
            let handedness = signals.settings.get_untracked().track_handedness;
            handle_fpl_import(
                &text,
                signals.set_graph,
                signals.set_lines,
                signals.lines,
                handedness,
            );
        }
        "GeoJSON" => {
            // Parse in background (dialog already shown in handle_file_change)
            let set_geojson_string = signals.set_geojson_string;
            let set_geojson_stations = signals.set_geojson_stations;

            spawn_local(async move {
                handle_geojson_analysis(&text, set_geojson_string, set_geojson_stations);
            });
        }
        _ => {
            handle_csv_analysis(
                &text,
                filename.clone(),
                signals.set_csv_config,
                signals.set_show_mapper,
                signals.set_import_error,
            );
        }
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

    // GeoJSON import state
    let (show_geojson_selector, set_show_geojson_selector) = create_signal(false);
    let (geojson_string, set_geojson_string) = create_signal(None::<String>);
    let (geojson_stations, set_geojson_stations) = create_signal(Vec::<StationData>::new());

    let handle_file_change = move |_| {
        let Some(input_elem) = file_input_ref.get() else {
            return;
        };
        let input: &web_sys::HtmlInputElement = &input_elem;
        let Some(files) = input.files() else { return };
        let Some(file) = files.get(0) else { return };

        // Check file extension to determine type
        let filename = file.name();
        let extension = std::path::Path::new(&filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_lowercase);

        let file_type = match extension.as_deref() {
            Some("fpl") => "FPL",
            Some("geojson" | "json") => "GeoJSON",
            _ => "CSV",
        };

        // For GeoJSON, show dialog immediately before reading file
        if file_type == "GeoJSON" {
            set_show_geojson_selector.set(true);
            set_geojson_stations.set(Vec::new());
        }

        let signals = FileProcessorSignals {
            set_file_content,
            set_graph,
            set_lines,
            lines,
            settings,
            set_geojson_string,
            set_geojson_stations,
            set_csv_config,
            set_show_mapper,
            set_import_error,
        };

        spawn_local(process_file(file, signals));
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

    let handle_geojson_confirm = move |bounds: SelectionBounds| {
        if let Some(geojson_str) = geojson_string.get() {
            // Spawn import in a local task so dialog stays alive and logs can flush
            spawn_local(async move {
                handle_geojson_import(geojson_str, bounds, set_graph, set_show_geojson_selector);
            });
        }
    };

    let handle_geojson_cancel = move |()| {
        set_show_geojson_selector.set(false);
    };

    view! {
        <input
            type="file"
            accept=".csv,.fpl,.geojson,.json"
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

        <Show when=move || show_geojson_selector.get()>
            <Window
                is_open=show_geojson_selector
                title=Signal::derive(|| "Select Region to Import".to_string())
                on_close=move || set_show_geojson_selector.set(false)
                position_key="geojson_selector"
                max_size=(1200.0, 900.0)
            >
                <GeoJsonRegionSelector
                    stations=geojson_stations
                    on_import=Callback::new(handle_geojson_confirm)
                    on_cancel=Callback::new(handle_geojson_cancel)
                />
            </Window>
        </Show>
    }
}
