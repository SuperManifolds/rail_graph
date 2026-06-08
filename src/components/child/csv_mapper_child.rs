use crate::components::csv_column_mapper::CsvColumnMapper;
use crate::import::csv::CsvImportConfig;
use crate::window_protocol::{ImporterCsvInit, ImporterCsvResult};
use leptos::{component, create_signal, view, Callback, IntoView, Signal, SignalGet};

#[component]
#[must_use]
pub fn CsvMapperChild(init: ImporterCsvInit, session: String) -> impl IntoView {
    let (config, _) = create_signal(Some(init.config));
    let (import_error, _) = create_signal(None::<String>);

    let session_import = session.clone();
    let handle_import = move |cfg: CsvImportConfig| {
        let result = ImporterCsvResult::Import(cfg);
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_import}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    };

    let handle_cancel = move |()| {
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::close_native_window("child-importer-csv").await;
        });
    };

    view! {
        <div class="child-window-content">
            <CsvColumnMapper
                config=Signal::derive(move || config.get().unwrap_or_else(|| {
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
                on_cancel=Callback::new(handle_cancel)
                on_import=Callback::new(handle_import)
                import_error=import_error
            />
        </div>
    }
}
