use leptos::*;
use chrono::Duration;
use crate::models::Line;
use crate::components::frequency_input::FrequencyInput;
use crate::components::time_input::TimeInput;

// Default values for line controls
const DEFAULT_COLOR: &str = "#000000";
const DEFAULT_FIRST_DEPARTURE: &str = "00:00";
const DEFAULT_RETURN_DEPARTURE: &str = "06:00";
const DEFAULT_FREQUENCY_MINUTES: i64 = 30;

#[component]
pub fn LineControls(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
) -> impl IntoView {
    view! {
        <div class="controls">
            <h3>"Line Configuration:"</h3>
            <div class="line-controls">
                {move || {
                    lines.get().into_iter().map(|line| {
                        view! {
                            <LineControl
                                line_id=line.id.clone()
                                lines=lines
                                set_lines=set_lines
                            />
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </div>
    }
}

#[component]
pub fn LineControl(
    line_id: String,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
) -> impl IntoView {
    view! {
        <div
            class="line-control"
            style={
                let id = line_id.clone();
                move || {
                    let current_lines = lines.get();
                    if let Some(current_line) = current_lines.iter().find(|l| l.id == id) {
                        format!("border-left: 4px solid {}", current_line.color)
                    } else {
                        "border-left: 4px solid #000".to_string()
                    }
                }
            }
        >
            <div class="line-header">
                <strong>{line_id.clone()}</strong>
                <input
                    type="color"
                    class="color-picker"
                    prop:value={
                        let id = line_id.clone();
                        move || {
                            let current_lines = lines.get();
                            if let Some(current_line) = current_lines.iter().find(|l| l.id == id) {
                                current_line.color.clone()
                            } else {
                                DEFAULT_COLOR.to_string()
                            }
                        }
                    }
                    on:change={
                        let id = line_id.clone();
                        move |ev| {
                            let new_color = event_target_value(&ev);
                            set_lines.update(|lines_vec| {
                                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                    line.color = new_color;
                                }
                            });
                        }
                    }
                />
            </div>
            <div class="control-row">
                <TimeInput
                    label="First departure: "
                    value={
                        let id = line_id.clone();
                        Signal::derive(move || {
                            let current_lines = lines.get();
                            if let Some(current_line) = current_lines.iter().find(|l| l.id == id) {
                                current_line.first_departure
                            } else {
                                chrono::NaiveTime::parse_from_str(&format!("{}:00", DEFAULT_FIRST_DEPARTURE), "%H:%M:%S")
                                    .map(|t| crate::constants::BASE_DATE.and_time(t))
                                    .unwrap()
                            }
                        })
                    }
                    default_time=DEFAULT_FIRST_DEPARTURE
                    on_change={
                        let id = line_id.clone();
                        Box::new(move |new_datetime| {
                            set_lines.update(|lines_vec| {
                                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                    line.first_departure = new_datetime;
                                }
                            });
                        })
                    }
                />
                <FrequencyInput
                    frequency={
                        let id = line_id.clone();
                        Signal::derive(move || {
                            let current_lines = lines.get();
                            if let Some(current_line) = current_lines.iter().find(|l| l.id == id) {
                                current_line.frequency
                            } else {
                                Duration::minutes(DEFAULT_FREQUENCY_MINUTES)
                            }
                        })
                    }
                    on_change={
                        let id = line_id.clone();
                        move |new_frequency| {
                            set_lines.update(|lines_vec| {
                                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                    line.frequency = new_frequency;
                                }
                            });
                        }
                    }
                />
                <TimeInput
                    label="Return departure: "
                    value={
                        let id = line_id.clone();
                        Signal::derive(move || {
                            let current_lines = lines.get();
                            if let Some(current_line) = current_lines.iter().find(|l| l.id == id) {
                                current_line.return_first_departure
                            } else {
                                chrono::NaiveTime::parse_from_str(&format!("{}:00", DEFAULT_RETURN_DEPARTURE), "%H:%M:%S")
                                    .map(|t| crate::constants::BASE_DATE.and_time(t))
                                    .unwrap()
                            }
                        })
                    }
                    default_time=DEFAULT_RETURN_DEPARTURE
                    on_change={
                        let id = line_id.clone();
                        Box::new(move |new_datetime| {
                            set_lines.update(|lines_vec| {
                                if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                    line.return_first_departure = new_datetime;
                                }
                            });
                        })
                    }
                />
            </div>
        </div>
    }
}