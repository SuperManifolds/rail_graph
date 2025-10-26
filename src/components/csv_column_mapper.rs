use leptos::{component, view, IntoView, Signal, SignalGet, SignalSet, SignalUpdate, create_signal, For, Callback, Callable, Show, ReadSignal};
use crate::import::csv::{CsvImportConfig, ColumnType, ColumnMapping, extract_line_identifier};
use crate::components::duration_input::DurationInput;
use chrono::Duration;

/// Helper function to apply column type to all columns in the same group position
fn apply_column_type_to_group(cfg: &mut CsvImportConfig, col_idx: usize, new_type: ColumnType) {
    let Some(pattern_len) = cfg.pattern_repeat else { return };

    let station_idx = cfg.columns.iter()
        .position(|c| c.column_type == ColumnType::StationName)
        .unwrap_or(0);

    let data_cols: Vec<(usize, usize)> = cfg.columns.iter()
        .filter(|c| {
            c.column_index > station_idx
            && c.column_type != ColumnType::Skip
            && c.column_type != ColumnType::TrackDistance
            && c.column_type != ColumnType::DistanceOffset
        })
        .map(|c| (c.column_index, c.group_index.unwrap_or(0)))
        .collect();

    let Some(target_pos) = data_cols.iter().position(|(idx, _)| *idx == col_idx) else { return };
    let position_in_group = target_pos % pattern_len;

    for (col_idx_to_update, _) in &data_cols {
        let Some(pos) = data_cols.iter().position(|(i, _)| i == col_idx_to_update) else { continue };
        if pos % pattern_len != position_in_group {
            continue;
        }
        let Some(col) = cfg.columns.iter_mut().find(|c| c.column_index == *col_idx_to_update) else { continue };
        col.column_type = new_type;
    }
}

/// Helper function to assign group indices based on pattern length
fn assign_group_indices(cfg: &mut CsvImportConfig, pattern_len: usize) {
    let station_idx = cfg.columns.iter()
        .position(|c| c.column_type == ColumnType::StationName)
        .unwrap_or(0);

    let mut data_col_idx = 0;
    for col in &mut cfg.columns {
        let is_data_col = col.column_index > station_idx
            && col.column_type != ColumnType::Skip
            && col.column_type != ColumnType::TrackDistance
            && col.column_type != ColumnType::DistanceOffset;
        col.group_index = if is_data_col {
            let group_idx = data_col_idx / pattern_len;
            data_col_idx += 1;
            Some(group_idx)
        } else {
            None
        };
    }
}

/// Helper function to extract line names from group headers
fn extract_group_line_names(cfg: &mut CsvImportConfig, pattern_len: usize) {
    let station_idx = cfg.columns.iter()
        .position(|c| c.column_type == ColumnType::StationName)
        .unwrap_or(0);

    cfg.group_line_names.clear();
    let data_columns: Vec<&ColumnMapping> = cfg.columns.iter()
        .filter(|c| {
            c.column_index > station_idx
            && c.column_type != ColumnType::Skip
            && c.column_type != ColumnType::TrackDistance
            && c.column_type != ColumnType::DistanceOffset
        })
        .collect();

    let num_groups = data_columns.len() / pattern_len;
    for group_idx in 0..num_groups {
        extract_and_insert_line_name(&mut cfg.group_line_names, &data_columns, group_idx, pattern_len);
    }
}

fn extract_and_insert_line_name(
    group_line_names: &mut std::collections::HashMap<usize, String>,
    data_columns: &[&ColumnMapping],
    group_idx: usize,
    pattern_len: usize,
) {
    let group_headers: Vec<Option<&str>> = (0..pattern_len)
        .filter_map(|pos| {
            let col_idx = group_idx * pattern_len + pos;
            Some(data_columns.get(col_idx)?.header.as_deref())
        })
        .collect();

    let Some(line_name) = extract_line_identifier(&group_headers) else { return };
    group_line_names.insert(group_idx, line_name);
}

#[component]
#[must_use]
pub fn CsvColumnMapper(
    config: Signal<CsvImportConfig>,
    on_cancel: Callback<()>,
    on_import: Callback<CsvImportConfig>,
    #[prop(optional)] import_error: Option<ReadSignal<Option<String>>>,
) -> impl IntoView {
    let (local_config, set_local_config) = create_signal(config.get());
    let (error_message, set_error_message) = create_signal(None::<String>);

    // Update local config when prop changes (detect new file by comparing all sample values)
    let extract_samples = |cfg: &CsvImportConfig| -> Vec<Vec<String>> {
        cfg.columns.iter().map(|c| c.sample_values.clone()).collect()
    };

    let (prev_samples, set_prev_samples) = create_signal(extract_samples(&config.get()));
    leptos::create_effect(move |_| {
        let new_config = config.get();
        let new_samples = extract_samples(&new_config);
        if new_samples != prev_samples.get() {
            set_local_config.set(new_config);
            set_prev_samples.set(new_samples);
            set_error_message.set(None);
        }
    });

    let update_column_type = move |col_idx: usize, new_type: ColumnType| {
        set_error_message.set(None);
        set_local_config.update(|cfg| {
            // Update the selected column
            if let Some(col) = cfg.columns.get_mut(col_idx) {
                col.column_type = new_type;
            }

            // If we're in grouped mode, apply this type to all columns in the same position within their groups
            apply_column_type_to_group(cfg, col_idx, new_type);
        });
    };

    let update_line_wait_time = move |line_id: String, new_duration: Duration| {
        set_local_config.update(|cfg| {
            cfg.defaults.per_line_wait_times.insert(line_id, new_duration);
        });
    };

    let update_group_name = move |group_idx: usize, name: String| {
        set_local_config.update(|cfg| {
            if name.trim().is_empty() {
                cfg.group_line_names.remove(&group_idx);
            } else {
                cfg.group_line_names.insert(group_idx, name);
            }
        });
    };

    let update_pattern_repeat = move |new_pattern: Option<usize>| {
        set_local_config.update(|cfg| {
            cfg.pattern_repeat = new_pattern;
            // Recalculate group assignments
            if let Some(pattern_len) = new_pattern {
                assign_group_indices(cfg, pattern_len);
                extract_group_line_names(cfg, pattern_len);
            } else {
                // Clear all group assignments
                for col in &mut cfg.columns {
                    col.group_index = None;
                }
                cfg.group_line_names.clear();
            }
        });
    };

    let update_disable_infrastructure = move |disabled: bool| {
        set_local_config.update(|cfg| {
            cfg.disable_infrastructure = disabled;
        });
    };

    let validate_config = move || -> Result<(), String> {
        let cfg = local_config.get();

        // Check for at least one station name column
        let has_station = cfg.columns.iter().any(|c| c.column_type == ColumnType::StationName);
        if !has_station {
            return Err("At least one column must be marked as Station Name".to_string());
        }

        Ok(())
    };

    let handle_import = move |_| {
        match validate_config() {
            Ok(()) => {
                set_error_message.set(None);
                on_import.call(local_config.get());
            }
            Err(msg) => {
                set_error_message.set(Some(msg));
            }
        }
    };

    view! {
        <div class="csv-column-mapper">
            <div class="mapper-help">
                <a href="https://github.com/SuperManifolds/rail_graph/blob/main/docs/csv-import-guide.md" target="_blank" rel="noopener noreferrer">
                    <i class="fa-solid fa-circle-question"></i>
                    " CSV Import Guide"
                </a>
            </div>

            <GroupingControls
                local_config=local_config
                update_pattern_repeat=update_pattern_repeat
                update_disable_infrastructure=update_disable_infrastructure
            />

            <ColumnMappingTable
                local_config=local_config
                update_column_type=update_column_type
            />

            <LineGroupingSection
                local_config=local_config
                update_group_name=update_group_name
            />

            <DefaultValuesSection
                local_config=local_config
                update_line_wait_time=update_line_wait_time
            />

            <Show when=move || error_message.get().is_some() || import_error.and_then(|e| e.get()).is_some()>
                <div class="mapper-error">
                    {move || {
                        error_message.get()
                            .or_else(|| import_error.and_then(|e| e.get()))
                            .unwrap_or_default()
                    }}
                </div>
            </Show>

            <div class="mapper-actions">
                <button on:click=move |_| on_cancel.call(())>"Cancel"</button>
                <button class="primary" on:click=handle_import>"Import"</button>
            </div>
        </div>
    }
}

fn column_type_to_string(col_type: ColumnType) -> String {
    col_type.as_str().to_string()
}

fn parse_column_type(s: &str) -> Option<ColumnType> {
    match s {
        "Station Name" => Some(ColumnType::StationName),
        "Platform" => Some(ColumnType::Platform),
        "Track Distance" => Some(ColumnType::TrackDistance),
        "Distance Offset" => Some(ColumnType::DistanceOffset),
        "Track Number" => Some(ColumnType::TrackNumber),
        "Arrival Time" => Some(ColumnType::ArrivalTime),
        "Departure Time" => Some(ColumnType::DepartureTime),
        "Travel Time" => Some(ColumnType::TravelTime),
        "Wait Time" => Some(ColumnType::WaitTime),
        "Offset" => Some(ColumnType::Offset),
        "Skip" => Some(ColumnType::Skip),
        _ => None,
    }
}

const COLUMN_TYPE_OPTIONS: [ColumnType; 11] = [
    ColumnType::StationName,
    ColumnType::Platform,
    ColumnType::TrackDistance,
    ColumnType::DistanceOffset,
    ColumnType::TrackNumber,
    ColumnType::ArrivalTime,
    ColumnType::DepartureTime,
    ColumnType::TravelTime,
    ColumnType::WaitTime,
    ColumnType::Offset,
    ColumnType::Skip,
];

fn detect_line_ids(config: &CsvImportConfig) -> Vec<String> {
    // If using grouped format, use group names
    if config.pattern_repeat.is_some() {
        let num_groups = count_groups(config);
        return (0..num_groups)
            .map(|group_idx| {
                config.group_line_names
                    .get(&group_idx)
                    .cloned()
                    .filter(|name| !name.trim().is_empty())
                    .or_else(|| {
                        // For single-group imports, use filename as fallback
                        if num_groups == 1 {
                            config.filename.clone()
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| format!("Line {}", group_idx + 1))
            })
            .collect();
    }

    // Otherwise, find columns with numbers in headers and apply priority
    let mut detected = Vec::new();
    let mut line_idx = 0;

    for c in &config.columns {
        // Check if this column has a header with numbers
        let has_number_in_header = c.header.as_ref()
            .is_some_and(|h| !h.trim().is_empty() && h.chars().any(|ch| ch.is_ascii_digit()));

        if has_number_in_header {
            // Priority: user input > auto detection
            let name = if let Some(user_name) = config.group_line_names.get(&line_idx) {
                if user_name.trim().is_empty() {
                    c.header.clone().expect("header should exist when has_number_in_header is true")
                } else {
                    user_name.clone()
                }
            } else {
                c.header.clone().expect("header should exist when has_number_in_header is true")
            };

            detected.push(name);
            line_idx += 1;
        }
    }

    if detected.is_empty() {
        // Fallback when no columns have numbers in headers
        // For single line: use explicit name > filename > "Line 1"
        vec![
            config.group_line_names.get(&0)
                .filter(|n| !n.trim().is_empty())
                .cloned()
                .or_else(|| config.filename.clone())
                .unwrap_or_else(|| "Line 1".to_string())
        ]
    } else {
        detected
    }
}

fn count_groups(config: &CsvImportConfig) -> usize {
    if config.pattern_repeat.is_some() {
        // Grouped mode: count groups
        config.columns.iter()
            .filter_map(|c| c.group_index)
            .max()
            .map_or(0, |max| max + 1)
    } else {
        // Non-grouped mode: count columns with numbers in headers
        let count = config.columns.iter()
            .filter(|c| {
                c.header.as_ref()
                    .is_some_and(|h| !h.trim().is_empty() && h.chars().any(|c| c.is_ascii_digit()))
            })
            .count();

        // At least 1 so the line name section shows
        count.max(1)
    }
}

#[component]
fn GroupingControls(
    local_config: ReadSignal<CsvImportConfig>,
    update_pattern_repeat: impl Fn(Option<usize>) + Copy + 'static,
    update_disable_infrastructure: impl Fn(bool) + Copy + 'static,
) -> impl IntoView {
    let data_column_count = move || {
        let cfg = local_config.get();
        let station_idx = cfg.columns.iter()
            .position(|c| c.column_type == ColumnType::StationName)
            .unwrap_or(0);
        cfg.columns.iter().filter(|c| {
            c.column_index > station_idx
            && c.column_type != ColumnType::Skip
            && c.column_type != ColumnType::TrackDistance
            && c.column_type != ColumnType::DistanceOffset
        }).count()
    };

    view! {
        <div class="mapper-section grouping-controls">
            <h3>"Column Grouping"</h3>
            <p class="help-text">
                "If your CSV has repeating patterns (e.g., Time1, Wait1, Track1, Time2, Wait2, Track2), specify the pattern length"
            </p>
            <div class="form-row">
                <label>"Pattern Repeat:"</label>
                <select
                    on:change=move |ev| {
                        let value = leptos::event_target_value(&ev);
                        let pattern = if value == "none" {
                            None
                        } else {
                            value.parse::<usize>().ok()
                        };
                        update_pattern_repeat(pattern);
                    }
                >
                    {move || {
                        let current_value = local_config.get().pattern_repeat
                            .map_or("none".to_string(), |p| p.to_string());

                        let mut options = vec![
                            view! {
                                <option value="none" selected=current_value == "none">
                                    "No grouping (each column is a separate line)"
                                </option>
                            }
                        ];

                        for n in 2..=data_column_count() {
                            if data_column_count() % n == 0 {
                                let num_groups = data_column_count() / n;
                                let value_str = n.to_string();
                                let is_selected = current_value == value_str;
                                options.push(view! {
                                    <option value=value_str.clone() selected=is_selected>
                                        {format!("Repeat every {n} columns ({num_groups} lines)")}
                                    </option>
                                });
                            }
                        }

                        options
                    }}
                </select>
            </div>
            <div class="form-row">
                <label>
                    <input
                        type="checkbox"
                        prop:checked=move || local_config.get().disable_infrastructure
                        on:change=move |ev| {
                            let checked = leptos::event_target_checked(&ev);
                            update_disable_infrastructure(checked);
                        }
                    />
                    " Don't create new infrastructure"
                </label>
                <p class="help-text" style="margin-left: 1.5rem;">
                    "Only use existing tracks and stations. Routes will be created by pathfinding between CSV stations."
                </p>
            </div>
        </div>
    }
}

#[component]
fn LineGroupingSection(
    local_config: ReadSignal<CsvImportConfig>,
    update_group_name: impl Fn(usize, String) + Copy + 'static,
) -> impl IntoView {
    let num_groups = move || count_groups(&local_config.get());
    let pattern_len = move || local_config.get().pattern_repeat.unwrap_or(0);

    view! {
        <div class="mapper-section line-grouping-section">
            <h3>"Line Names"</h3>
            <Show when=move || { pattern_len() > 1 }>
                <p class="help-text">
                    {move || {
                        let pattern = pattern_len();
                        let groups = num_groups();
                        format!("Detected repeating pattern: columns repeat every {pattern} columns for {groups} lines")
                    }}
                </p>
            </Show>

            <div class="group-names-form">
                <For
                    each=move || 0..num_groups()
                    key=|group_idx| *group_idx
                    let:group_idx
                >
                    {
                        // For single group, use filename as default; otherwise use "Line N"
                        let default_name = move || {
                            let cfg = local_config.get();
                            let groups = count_groups(&cfg);
                            if groups == 1 {
                                cfg.filename.clone().unwrap_or_else(|| format!("Line {}", group_idx + 1))
                            } else {
                                format!("Line {}", group_idx + 1)
                            }
                        };

                        view! {
                            <div class="form-row">
                                <label>{format!("Group {} Name:", group_idx + 1)}</label>
                                <input
                                    type="text"
                                    placeholder=default_name
                                    prop:value=move || {
                                        let cfg = local_config.get();
                                        let groups = count_groups(&cfg);
                                        // Auto-populate with filename for single group on initial load
                                        cfg.group_line_names
                                            .get(&group_idx)
                                            .cloned()
                                            .or_else(|| {
                                                if groups == 1 {
                                                    cfg.filename.clone()
                                                } else {
                                                    None
                                                }
                                            })
                                            .unwrap_or_default()
                                    }
                                    on:input=move |ev| {
                                        let value = leptos::event_target_value(&ev);
                                        update_group_name(group_idx, value);
                                    }
                                />
                            </div>
                        }
                    }
                </For>
            </div>
        </div>
    }
}

#[component]
fn ColumnMappingTable(
    local_config: ReadSignal<CsvImportConfig>,
    update_column_type: impl Fn(usize, ColumnType) + Copy + 'static,
) -> impl IntoView {
    view! {
        <div class="mapper-section">
            <h3>"Column Mapping"</h3>
            <p class="help-text">"Select the type of data in each column"</p>

            <table class="column-mapping-table">
                <thead>
                    <tr>
                        <th>"Col"</th>
                        <th>"Group"</th>
                        <th>"Header"</th>
                        <th>"Sample Values"</th>
                        <th>"Column Type"</th>
                    </tr>
                </thead>
                <tbody>
                    <For
                        each=move || {
                            let cfg = local_config.get();
                            let is_grouped = cfg.pattern_repeat.is_some();
                            cfg.columns.into_iter()
                                .filter(|col| {
                                    // Always show station column
                                    if col.column_type == ColumnType::StationName {
                                        return true;
                                    }
                                    // If grouped, only show group 0 columns
                                    if is_grouped {
                                        col.group_index == Some(0)
                                    } else {
                                        true
                                    }
                                })
                                .collect::<Vec<_>>()
                        }
                        key=|col| col.column_index
                        children=move |col: ColumnMapping| {
                            let col_idx = col.column_index;
                            let group_display = col.group_index.map_or("-".to_string(), |g| (g + 1).to_string());

                            view! {
                                <tr>
                                    <td>{col_idx}</td>
                                    <td class="group-indicator">{group_display}</td>
                                    <td>{col.header.clone().unwrap_or_else(|| "-".to_string())}</td>
                                    <td class="sample-values">
                                        {col.sample_values.iter().take(3).cloned().collect::<Vec<_>>().join(", ")}
                                    </td>
                                    <td>
                                        <select
                                            on:change=move |ev| {
                                                let value = leptos::event_target_value(&ev);
                                                if let Some(col_type) = parse_column_type(&value) {
                                                    update_column_type(col_idx, col_type);
                                                }
                                            }
                                        >
                                            {move || {
                                                let current_type = local_config.get().columns
                                                    .iter()
                                                    .find(|c| c.column_index == col_idx)
                                                    .map_or(ColumnType::Skip, |c| c.column_type);

                                                COLUMN_TYPE_OPTIONS.iter().map(|opt| {
                                                    let opt_str = column_type_to_string(*opt);
                                                    let is_selected = *opt == current_type;
                                                    view! {
                                                        <option value=opt_str.clone() selected=is_selected>{opt_str}</option>
                                                    }
                                                }).collect::<Vec<_>>()
                                            }}
                                        </select>
                                    </td>
                                </tr>
                            }
                        }
                    />
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn DefaultValuesSection(
    local_config: ReadSignal<CsvImportConfig>,
    update_line_wait_time: impl Fn(String, Duration) + Copy + 'static,
) -> impl IntoView {
    let detect_line_ids_signal = move || detect_line_ids(&local_config.get());

    view! {
        <Show when=move || !detect_line_ids_signal().is_empty()>
            <div class="mapper-section">
                <h3>"Per-Line Wait Times"</h3>
                <p class="help-text">"Default wait time at each station for each line"</p>

                <div class="default-values-form">
                    <For
                        each=detect_line_ids_signal
                        key=|line_id| line_id.clone()
                        children=move |line_id: String| {
                            let line_id_clone = line_id.clone();
                            view! {
                                <div class="form-row">
                                    <label>{line_id.clone()}</label>
                                    <DurationInput
                                        duration=Signal::derive(move || {
                                            local_config.get()
                                                .defaults
                                                .per_line_wait_times
                                                .get(&line_id.clone())
                                                .copied()
                                                .unwrap_or_else(|| local_config.get().defaults.default_wait_time)
                                        })
                                        on_change=move |duration| {
                                            update_line_wait_time(line_id_clone.clone(), duration);
                                        }
                                    />
                                </div>
                            }
                        }
                    />
                </div>
            </div>
        </Show>
    }
}
