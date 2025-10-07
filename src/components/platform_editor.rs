use crate::models::Platform;
use leptos::*;

#[component]
pub fn PlatformEditor(
    platforms: ReadSignal<Vec<Platform>>,
    set_platforms: WriteSignal<Vec<Platform>>,
    is_passing_loop: ReadSignal<bool>,
) -> impl IntoView {
    let handle_add_platform = move |_| {
        set_platforms.update(|p| {
            let next_num = p.len() + 1;
            p.push(Platform {
                name: next_num.to_string(),
            });
        });
    };

    let handle_remove_platform = move |index: usize| {
        set_platforms.update(|p| {
            if p.len() > 1 {
                p.remove(index);
                // Renumber remaining platforms
                for (i, platform) in p.iter_mut().enumerate() {
                    platform.name = (i + 1).to_string();
                }
            }
        });
    };

    view! {
        <div class="form-field">
            <label>{move || if is_passing_loop.get() { "Tracks" } else { "Platforms" }}</label>
            <div class="tracks-visual">
                <div class="tracks-horizontal">
                    {move || {
                        platforms.get().iter().enumerate().map(|(i, platform)| {
                            let platform_name = platform.name.clone();
                            view! {
                                <div class="track-column">
                                    <input
                                        type="text"
                                        class="track-number-input"
                                        value=platform_name
                                        on:change=move |ev| {
                                            let new_name = event_target_value(&ev);
                                            set_platforms.update(|p| {
                                                if let Some(platform) = p.get_mut(i) {
                                                    platform.name = new_name;
                                                }
                                            });
                                        }
                                    />
                                    {if platforms.get().len() > 1 {
                                        view! {
                                            <button
                                                class="remove-track-button-small"
                                                on:click=move |_| handle_remove_platform(i)
                                                title=move || if is_passing_loop.get() { "Remove track" } else { "Remove platform" }
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
                    <button
                        class="add-track-button-inline"
                        on:click=handle_add_platform
                        title=move || if is_passing_loop.get() { "Add Track" } else { "Add Platform" }
                    >
                        <i class="fa-solid fa-plus"></i>
                    </button>
                </div>
            </div>
        </div>
    }
}
