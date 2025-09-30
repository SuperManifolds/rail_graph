use crate::data::parse_csv_string;
use crate::models::{Line, Station};
use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn Importer(
    set_lines: WriteSignal<Vec<Line>>,
    set_stations: WriteSignal<Vec<Station>>,
) -> impl IntoView {
    let file_input_ref = create_node_ref::<leptos::html::Input>();

    let handle_file_change = move |_| {
        let Some(input_elem) = file_input_ref.get() else { return };
        let input: &web_sys::HtmlInputElement = &input_elem;
        let Some(files) = input.files() else { return };
        let Some(file) = files.get(0) else { return };

        spawn_local(async move {
            let reader = web_sys::FileReader::new().unwrap();
            let reader_clone = reader.clone();

            let onload = Closure::wrap(Box::new(move |_: web_sys::Event| {
                if let Ok(result) = reader_clone.result() {
                    if let Some(text) = result.as_string() {
                        let (new_lines, new_stations) = parse_csv_string(&text);
                        set_lines.set(new_lines);
                        set_stations.set(new_stations);
                    }
                }
            }) as Box<dyn FnMut(_)>);

            reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();

            let _ = reader.read_as_text(&file);
        });
    };

    view! {
        <input
            type="file"
            accept=".csv"
            node_ref=file_input_ref
            on:change=handle_file_change
            style="display: none;"
        />
        <button
            class="import-button"
            on:click=move |_| {
                if let Some(input) = file_input_ref.get() {
                    input.click();
                }
            }
            title="Import CSV"
        >
            <i class="fa-solid fa-file-import"></i>
        </button>
    }
}
