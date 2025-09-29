use leptos::*;
use chrono::{Duration, NaiveTime};
use crate::models::Line;

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
                                "#000000".to_string()
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
                <label>
                    "First departure: "
                    <input
                        type="time"
                        prop:value={
                            let id = line_id.clone();
                            move || {
                                let current_lines = lines.get();
                                if let Some(current_line) = current_lines.iter().find(|l| l.id == id) {
                                    current_line.first_departure.format("%H:%M").to_string()
                                } else {
                                    "00:00".to_string()
                                }
                            }
                        }
                        on:input={
                            let id = line_id.clone();
                            move |ev| {
                                let time_str = event_target_value(&ev);
                                if let Ok(new_time) = NaiveTime::parse_from_str(&format!("{}:00", time_str), "%H:%M:%S") {
                                    set_lines.update(|lines_vec| {
                                        if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                            line.first_departure = new_time;
                                        }
                                    });
                                }
                            }
                        }
                    />
                </label>
                <label>
                    "Frequency (min): "
                    <input
                        type="number"
                        min="1"
                        max="180"
                        prop:value={
                            let id = line_id.clone();
                            move || {
                                let current_lines = lines.get();
                                if let Some(current_line) = current_lines.iter().find(|l| l.id == id) {
                                    current_line.frequency.num_minutes().to_string()
                                } else {
                                    "30".to_string()
                                }
                            }
                        }
                        on:input={
                            let id = line_id.clone();
                            move |ev| {
                                let freq_str = event_target_value(&ev);
                                if let Ok(minutes) = freq_str.parse::<i64>() {
                                    if minutes > 0 {
                                        set_lines.update(|lines_vec| {
                                            if let Some(line) = lines_vec.iter_mut().find(|l| l.id == id) {
                                                line.frequency = Duration::minutes(minutes);
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    />
                </label>
            </div>
        </div>
    }
}