use leptos::{component, view, IntoView, create_signal, SignalGet, SignalSet, create_resource, create_effect, Signal, use_context, WriteSignal, SignalUpdate};
use crate::components::modal_overlay::ModalOverlay;
use crate::components::window::Window;
use crate::storage::{Storage, IndexedDbStorage};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use pulldown_cmark::{Parser, Options, html};

const LAST_VIEWED_CHANGELOG_KEY: &str = "rail_graph_last_viewed_changelog";
const CHANGELOG_API: &str = "/api/changelog";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangelogRelease {
    tag_name: String,
    name: String,
    body: String,
    published_at: String,
}

#[component]
#[must_use]
pub fn ChangelogPopup(
    #[prop(optional)] manual_open: Option<Signal<bool>>,
    #[prop(optional)] set_manual_open: Option<impl Fn(bool) + 'static + Copy>,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);
    let (releases_to_show, set_releases_to_show) = create_signal(Vec::<ChangelogRelease>::new());

    // Check if we should show the changelog
    let should_show = create_resource(
        || (),
        move |()| async move {
            // Check if user has project data
            let storage = IndexedDbStorage;
            let Ok(projects) = storage.list_projects().await else {
                return false;
            };

            if projects.is_empty() {
                return false;
            }

            // Fetch all releases
            let Ok(all_releases) = fetch_all_releases().await else {
                return false;
            };

            if all_releases.is_empty() {
                return false;
            }

            // Get the last viewed version
            let last_viewed = get_last_viewed_version();

            // Filter releases to show only those since last viewed
            let releases = if let Some(last_viewed_version) = last_viewed {
                // Find the index of the last viewed version
                let last_viewed_idx = all_releases
                    .iter()
                    .position(|r| r.tag_name == last_viewed_version);

                match last_viewed_idx {
                    Some(idx) => all_releases[..idx].to_vec(), // Get all releases before the last viewed one
                    None => all_releases, // If we can't find it, show all
                }
            } else {
                // First time viewing, only show the latest release
                all_releases.into_iter().take(1).collect()
            };

            if releases.is_empty() {
                return false;
            }

            set_releases_to_show.set(releases);
            true
        },
    );

    let on_close = move || {
        let releases = releases_to_show.get();
        if let Some(latest) = releases.first() {
            mark_version_viewed(&latest.tag_name);
        }
        set_is_open.set(false);
        if let Some(setter) = set_manual_open {
            setter(false);
        }
    };

    // Auto-show changelog for new versions
    create_effect(move |_| {
        if let Some(should_show_val) = should_show.get() {
            if should_show_val {
                set_is_open.set(true);
            }
        }
    });

    // Manual open from About button
    create_effect(move |_| {
        let Some(manual_signal) = manual_open else { return };
        if !manual_signal.get() {
            return;
        }

        // Reset the signal immediately so it can be triggered again
        if let Some(setter) = set_manual_open {
            setter(false);
        }

        // Fetch all releases to show
        leptos::spawn_local(async move {
            let Ok(all_releases) = fetch_all_releases().await else { return };
            set_releases_to_show.set(all_releases);
            set_is_open.set(true);
        });
    });

    view! {
        <ModalOverlay is_open=Signal::derive(move || is_open.get())>
            <Window
                is_open=Signal::derive(|| true)
                title=Signal::derive(|| "What's New".to_string())
                on_close=move || on_close()
            >
                <ChangelogContent releases_to_show=releases_to_show on_close=on_close />
            </Window>
        </ModalOverlay>
    }
}

#[component]
fn ChangelogContent(
    releases_to_show: leptos::ReadSignal<Vec<ChangelogRelease>>,
    on_close: impl Fn() + 'static + Copy,
) -> impl IntoView {
    // Get resize trigger from Window context
    let resize_trigger = use_context::<WriteSignal<u32>>();

    // Trigger resize when content loads
    create_effect(move |_| {
        if !releases_to_show.get().is_empty() {
            if let Some(trigger) = resize_trigger {
                trigger.update(|v| *v += 1);
            }
        }
    });

    view! {
        <div class="changelog-content">
            {move || {
                let releases = releases_to_show.get();
                if releases.is_empty() {
                    return view! {}.into_view();
                }

                view! {
                    <>
                        {releases.into_iter().map(|release| view! {
                            <div class="changelog-release">
                                <div class="changelog-header">
                                    <div class="changelog-version">{&release.tag_name}</div>
                                    <div class="changelog-title">{&release.name}</div>
                                    <div class="changelog-date">{format_date(&release.published_at)}</div>
                                </div>
                                <div class="changelog-body" inner_html=markdown_to_html(&release.body)></div>
                            </div>
                        }).collect::<Vec<_>>()}
                        <div class="changelog-buttons">
                            <button class="primary" on:click=move |_| on_close()>
                                "Got it!"
                            </button>
                        </div>
                    </>
                }.into_view()
            }}
        </div>
    }
}

async fn fetch_all_releases() -> Result<Vec<ChangelogRelease>, String> {
    let Some(window) = web_sys::window() else {
        return Err("No window".to_string());
    };

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");

    let request = web_sys::Request::new_with_str_and_init(CHANGELOG_API, &opts)
        .map_err(|_| "Failed to create request".to_string())?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "Fetch failed".to_string())?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Invalid response".to_string())?;

    if !resp.ok() {
        return Err(format!("HTTP error: {}", resp.status()));
    }

    let text = JsFuture::from(resp.text().map_err(|_| "Failed to get text".to_string())?)
        .await
        .map_err(|_| "Failed to parse text".to_string())?;

    let text_str = text.as_string().ok_or("Response is not a string")?;

    serde_json::from_str(&text_str)
        .map_err(|e| format!("Failed to deserialize: {e:?}"))
}

fn get_last_viewed_version() -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(LAST_VIEWED_CHANGELOG_KEY).ok()?
}

fn mark_version_viewed(version: &str) {
    let Some(window) = web_sys::window() else { return };
    let Ok(Some(storage)) = window.local_storage() else { return };

    let _ = storage.set_item(LAST_VIEWED_CHANGELOG_KEY, version);
}

fn format_date(iso_date: &str) -> String {
    // Simple date formatting - just take the date part
    iso_date
        .split('T')
        .next()
        .unwrap_or(iso_date)
        .to_string()
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
