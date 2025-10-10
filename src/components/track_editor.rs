use crate::models::{Track, TrackDirection};
use leptos::{component, IntoView, ReadSignal, SignalGet, view};

#[component]
pub fn TrackEditor(
    tracks: ReadSignal<Vec<Track>>,
    from_station_name: ReadSignal<String>,
    to_station_name: ReadSignal<String>,
    on_add_track: impl Fn() + 'static,
    on_remove_track: impl Fn(usize) + 'static + Copy,
    on_change_direction: impl Fn(usize, TrackDirection) + 'static + Copy,
) -> impl IntoView {
    view! {
        <div class="tracks-visual">
            <div class="station-label station-top">{move || from_station_name.get()}</div>
            <div class="tracks-horizontal">
                {move || {
                    tracks.get().iter().enumerate().map(|(i, track)| {
                        let direction = track.direction;
                        view! {
                            <div class="track-column">
                                <div class="track-number">{i + 1}</div>
                                <button
                                    class="direction-button"
                                    on:click=move |_| {
                                        let new_dir = match direction {
                                            TrackDirection::Bidirectional => TrackDirection::Forward,
                                            TrackDirection::Forward => TrackDirection::Backward,
                                            TrackDirection::Backward => TrackDirection::Bidirectional,
                                        };
                                        on_change_direction(i, new_dir);
                                    }
                                    title=move || match direction {
                                        TrackDirection::Bidirectional => "Bidirectional".to_string(),
                                        TrackDirection::Forward => format!("{} → {}", from_station_name.get(), to_station_name.get()),
                                        TrackDirection::Backward => format!("{} → {}", to_station_name.get(), from_station_name.get()),
                                    }
                                >
                                    <i class=move || match direction {
                                        TrackDirection::Bidirectional => "fa-solid fa-arrows-up-down",
                                        TrackDirection::Forward => "fa-solid fa-arrow-down",
                                        TrackDirection::Backward => "fa-solid fa-arrow-up",
                                    }></i>
                                </button>
                                {if tracks.get().len() > 1 {
                                    view! {
                                        <button
                                            class="remove-track-button-small"
                                            on:click=move |_| on_remove_track(i)
                                            title="Remove track"
                                        >
                                            <i class="fa-solid fa-xmark"></i>
                                        </button>
                                    }.into_view()
                                } else {
                                    view! { <div class="track-spacer"></div> }.into_view()
                                }}
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}
                <button class="add-track-button-inline" on:click=move |_| on_add_track() title="Add Track">
                    <i class="fa-solid fa-plus"></i>
                </button>
            </div>
            <div class="station-label station-bottom">{move || to_station_name.get()}</div>
        </div>
    }
}
