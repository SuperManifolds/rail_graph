mod commands;
mod crash_reporting;
mod menu;
mod updates;

use railgraph_core::models::{Project, UndoManager};
use tauri::{Emitter, Manager};

pub struct AppState {
    pub project: std::sync::Mutex<Project>,
    pub undo_manager: std::sync::Mutex<UndoManager>,
    pub last_snapshot_time: std::sync::Mutex<std::time::Instant>,
    pub last_snapshot_field: std::sync::Mutex<String>,
}

fn load_initial_project(app: &tauri::AppHandle) -> Project {
    let project_id = commands::get_current_project_id_internal(app);
    if let Some(id) = project_id {
        let Ok(dir) = commands::projects_dir_internal(app) else {
            return Project::empty();
        };
        let path = dir.join(format!("{id}.rgproject"));
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(project) = Project::from_bytes(&bytes) {
                log::info!("Loaded project: {}", project.metadata.name);
                return project;
            }
        }
    }
    log::info!("No saved project found, creating empty project");
    Project::empty()
}

/// Dependency log targets that emit transient or benign operational noise —
/// update-check network blips — rather than actionable bugs. Their events
/// become Sentry breadcrumbs (context on a real crash) instead of an issue per
/// line, which would bury genuine panics and burn the quota.
const NOISY_LOG_TARGETS: &[&str] = &["tauri_plugin_updater"];

fn is_noisy_target(target: &str) -> bool {
    NOISY_LOG_TARGETS.iter().any(|prefix| target.starts_with(prefix))
}

/// Pulls the real target out of a `log`-crate record. These reach us via
/// `tracing-log`, which gives the bridged event a generic `metadata().target()`
/// of `"log"` and stashes the true target (e.g. `tauri_plugin_updater`) in a
/// `log.target` field — so a check on the metadata target alone never matches
/// them. This app's own backend code also logs via the `log` crate.
struct LogTargetVisitor(Option<String>);

impl tracing::field::Visit for LogTargetVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "log.target" {
            self.0 = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {}
}

/// Maps `tracing` events to Sentry. Mirrors the default (`error!` → issue,
/// `warn!`/`info!` → breadcrumb, lower → ignore), except events from
/// [`NOISY_LOG_TARGETS`] always become breadcrumbs. A mapper (not an
/// `event_filter`) is required because the filter only sees `Metadata`, whose
/// target is `"log"` for `log`-bridged records.
fn sentry_event_mapper<S>(
    event: &tracing::Event<'_>,
    _ctx: tracing_subscriber::layer::Context<'_, S>,
) -> sentry::integrations::tracing::EventMapping
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    use sentry::integrations::tracing::{breadcrumb_from_event, event_from_event, EventMapping};
    use tracing_subscriber::layer::Context;

    let mut visitor = LogTargetVisitor(None);
    event.record(&mut visitor);
    let target = visitor.0.as_deref().unwrap_or_else(|| event.metadata().target());

    let no_ctx = None::<&Context<'_, S>>;
    if is_noisy_target(target) {
        return EventMapping::Breadcrumb(breadcrumb_from_event(event, no_ctx));
    }
    match *event.metadata().level() {
        tracing::Level::ERROR => EventMapping::Event(event_from_event(event, no_ctx)),
        tracing::Level::WARN | tracing::Level::INFO => {
            EventMapping::Breadcrumb(breadcrumb_from_event(event, no_ctx))
        }
        tracing::Level::DEBUG | tracing::Level::TRACE => EventMapping::Ignore,
    }
}

fn main() {
    // Leveled logging to stderr; RUST_LOG overrides the default (info and
    // above). The Sentry layer turns these events into breadcrumbs (the trail
    // before a crash) and forwards `error!` as issues + structured logs. It
    // no-ops until a Sentry client is bound below, and stays inert entirely
    // when reporting is off. `log`-crate records are bridged in via
    // tracing-log, so the existing `log::` macros keep working.
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(sentry::integrations::tracing::layer().event_mapper(sentry_event_mapper))
        .init();

    // Crash reporting must init before the Tauri builder: the minidump handler
    // re-execs this binary as a separate crash-reporter process, and everything
    // up to `minidump::init` runs in both the app and reporter processes. Both
    // guards must live until the app exits — dropping them stops the reporter
    // and flushes Sentry. `None` when reporting is disabled.
    let sentry_guard = crash_reporting::init();
    let _minidump_guard = crash_reporting::arm_minidump_reporter(sentry_guard.as_ref());

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Second launch: focus an existing main window instead of starting
            // a new process (which would fight over the project state on disk).
            let windows = app.webview_windows();
            let window = windows
                .values()
                .find(|w| w.label().starts_with("main"))
                .or_else(|| windows.values().next());
            if let Some(window) = window {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    if let Some(client) = &sentry_guard {
        // Enriches webview errors with Rust/OS context and merges breadcrumbs
        // across the Rust and browser SDKs.
        builder = builder.plugin(tauri_plugin_sentry::init(client));
    }
    // The webview renders in a separate content process; when it dies the
    // window goes blank with no Rust panic and no minidump (the minidump
    // handler covers only the main process, and `@sentry/browser` only sees JS
    // errors, not a renderer crash). Capture it as an error so it surfaces in
    // Sentry. Tauri only exposes this on macOS/iOS — a WebView2 renderer crash
    // on Windows has no equivalent hook, so that blind spot remains.
    #[cfg(target_os = "macos")]
    {
        builder = builder.on_web_content_process_terminate(|webview| {
            tracing::error!("webview content process terminated: {}", webview.label());
        });
    }
    builder
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let project = load_initial_project(app.handle());
            app.manage(AppState {
                project: std::sync::Mutex::new(project),
                undo_manager: std::sync::Mutex::new(UndoManager::default()),
                last_snapshot_time: std::sync::Mutex::new(std::time::Instant::now()),
                last_snapshot_field: std::sync::Mutex::new(String::new()),
            });
            menu::setup(app.handle())?;
            // Persist this launch's version + run count for the next startup's
            // install/update fingerprint (see crash_reporting::attach_release_scope).
            crash_reporting::record_launch(app.handle());
            tauri::async_runtime::spawn(updates::check_on_startup(app.handle().clone()));
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == menu::CHECK_UPDATES_ID {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    updates::check_interactive(&app).await;
                });
            }
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let label = window.label();
                let _ = window
                    .app_handle()
                    .emit(&format!("window-closed:{label}"), ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::save_project,
            commands::load_project,
            commands::delete_project,
            commands::list_projects,
            commands::get_current_project_id,
            commands::set_current_project_id,
            commands::detect_conflicts,
            commands::compute_auto_layout,
            commands::update_field,
            commands::load_project_state,
            commands::replace_project,
            commands::undo,
            commands::redo,
            commands::save_window_metadata,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
