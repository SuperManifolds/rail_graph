//! Automatic crash reporting via Sentry.
//!
//! Captures three classes of failure and uploads them:
//! - **Native crashes** (segfaults, aborts) via `sentry-rust-minidump`, which
//!   re-execs this binary as a separate crash-reporter process so it outlives
//!   the crash.
//! - **Rust panics**, via Sentry's default panic integration.
//! - **Frontend/webview errors**, from the `@sentry/browser` bundle that
//!   `tauri-plugin-sentry` injects into every webview (see [`main`]).
//!
//! The DSN is baked at build time from `RAILGRAPH_SENTRY_DSN`. Without it —
//! every local dev build — reporting is a no-op. Users can opt out in Settings;
//! that flag is read straight from the on-disk settings store here, before
//! Tauri (and its store plugin) starts, so nothing is sent when disabled.

/// Sentry ingest endpoint, baked at build time. `None` (or empty) in builds
/// without `RAILGRAPH_SENTRY_DSN` set — all local dev builds — disabling
/// reporting.
const DSN: Option<&str> = option_env!("RAILGRAPH_SENTRY_DSN");

/// Settings key gating crash reporting. Absent or `true` means enabled: this is
/// opt-out, so a first-launch crash still reports before the user sees the
/// toggle.
const CRASH_REPORTING_KEY: &str = "crash_reporting";

/// Settings key holding the app version last seen on disk. Compared against the
/// running version at startup to tell a fresh install from an in-place update.
const LAST_VERSION_KEY: &str = "last_seen_version";

/// Settings key holding how many times the app has launched (with reporting on).
const RUN_COUNT_KEY: &str = "run_count";

/// The running app version (`tauri.conf.json` is kept in sync with this at
/// release time).
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bundle identifier from `tauri.conf.json`; the store plugin persists under
/// `config_dir()/<identifier>`.
const APP_IDENTIFIER: &str = "com.railgraph.app";

/// Name of the Tauri store file holding user settings.
pub const SETTINGS_STORE: &str = "settings.json";

/// Sentry release name. Kept as `railgraph@<version>` (not the crate name
/// `railgraph-backend`) to match the release the CI symbol-upload steps create.
fn release_name() -> String {
    format!("railgraph@{APP_VERSION}")
}

/// Read the settings store straight off disk. Used before Tauri (and its store
/// plugin) is up. `None` when the file is absent or unreadable.
pub(crate) fn read_store_from_disk() -> Option<serde_json::Value> {
    let path = dirs_next::config_dir()?
        .join(APP_IDENTIFIER)
        .join(SETTINGS_STORE);
    if !path.exists() {
        return None;
    }
    let parsed = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    if parsed.is_none() {
        tracing::warn!(
            "settings store at {} is unreadable; using defaults",
            path.display()
        );
    }
    parsed
}

/// Process start, stamped the first time [`attach_uptime_processor`] runs. Lets
/// every event carry an `uptime_s` tag, which separates startup crashes from
/// steady-state ones.
static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Tag events with the install/update transition, derived from the version last
/// seen on disk versus the running one. Read-only: it runs in both the app and
/// the re-exec'd reporter (before the fork), so native crashes carry it too; the
/// new version is written once, later, by [`record_launch`] in the app process.
///
/// `install_kind` is `fresh` (no prior version — first launch), `updated` (a
/// different prior version — the prime "did the release do it?" signal, paired
/// with `updated_from`), or `same`. `run_count` includes the current launch.
fn attach_release_scope(store: Option<&serde_json::Value>) {
    let last = store
        .and_then(|s| s.get(LAST_VERSION_KEY))
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty());
    let run_count = store
        .and_then(|s| s.get(RUN_COUNT_KEY))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let install_kind = match last {
        None => "fresh",
        Some(v) if v == APP_VERSION => "same",
        Some(_) => "updated",
    };

    sentry::configure_scope(|scope| {
        scope.set_tag("install_kind", install_kind);
        scope.set_tag("run_count", run_count + 1);
        if install_kind == "updated" {
            if let Some(prev) = last {
                scope.set_tag("updated_from", prev);
            }
        }
    });
}

/// Add a scope event processor that stamps every outgoing event with seconds
/// since process start. Covers Rust panics and `error!` events (native minidumps
/// are uploaded by the reporter process and carry their own timestamps instead).
fn attach_uptime_processor() {
    START.get_or_init(std::time::Instant::now);
    sentry::configure_scope(|scope| {
        scope.add_event_processor(|mut event| {
            if let Some(start) = START.get() {
                event.tags.insert(
                    "uptime_s".to_string(),
                    start.elapsed().as_secs().to_string(),
                );
            }
            Some(event)
        });
    });
}

/// Persist the running version and bump the launch counter, so the next startup
/// can detect an update and report the run count. Uses the Tauri store plugin,
/// so it must run in the app process **after** that plugin is initialised (the
/// setup hook), not in [`init`] which predates Tauri. Best-effort: a store
/// failure just means the next launch sees a slightly stale fingerprint.
pub fn record_launch(app: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;
    let Ok(store) = app.store(SETTINGS_STORE) else {
        return;
    };
    let prev = store
        .get(RUN_COUNT_KEY)
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    store.set(RUN_COUNT_KEY, serde_json::json!(prev + 1));
    store.set(LAST_VERSION_KEY, serde_json::json!(APP_VERSION));
    let _ = store.save();
}

/// Whether the store permits crash reporting. Absent key, absent store, a
/// non-bool value, or `true` all mean enabled (opt-out, fail-open) so a
/// first-launch or corrupt-store crash still reports.
fn reporting_enabled(store: Option<&serde_json::Value>) -> bool {
    store
        .and_then(|s| s.get(CRASH_REPORTING_KEY))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

/// Initialise the Sentry client, or return `None` when reporting is disabled
/// (no DSN baked, an unparseable DSN, or the user opted out).
///
/// The returned guard must be kept alive for the whole program: dropping it
/// flushes and shuts down the transport. Runs in **both** the app and the
/// re-exec'd crash-reporter process (up to `minidump::init`), so both agree on
/// the opt-out and tags. `sentry::init` does spawn a background transport
/// thread.
pub fn init() -> Option<sentry::ClientInitGuard> {
    let dsn = DSN.filter(|dsn| !dsn.trim().is_empty())?;
    let store = read_store_from_disk();
    if !reporting_enabled(store.as_ref()) {
        return None;
    }
    // Parse the DSN ourselves — `sentry::init` panics on a malformed one, which
    // would be a worse startup crash than the ones we're trying to capture.
    let dsn: sentry::types::Dsn = match dsn.trim().parse() {
        Ok(dsn) => dsn,
        Err(err) => {
            tracing::warn!("invalid Sentry DSN; crash reporting disabled: {err}");
            return None;
        }
    };
    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some(release_name().into()),
            // Set by the release workflow ("production") so future prerelease
            // builds don't pollute shipped-version stats. Unset → None.
            environment: option_env!("RAILGRAPH_SENTRY_ENVIRONMENT")
                .map(std::borrow::Cow::Borrowed),
            // Release health: one session per app run, so the dashboard reports
            // crash-free-session/user rates and per-release adoption.
            auto_session_tracking: true,
            session_mode: sentry::SessionMode::Application,
            // Forward `tracing` events as structured logs (see main's
            // subscriber).
            enable_logs: true,
            // Attach a stack trace to messages/logs, not just captured errors.
            attach_stacktrace: true,
            // Crash diagnostics only — no IP addresses or request headers.
            send_default_pii: false,
            ..Default::default()
        },
    ));
    // Set before `minidump::init` forks (both processes run this), so
    // native-crash events carry the tags too. Device/OS come free via
    // `contexts`.
    attach_release_scope(store.as_ref());
    attach_uptime_processor();
    Some(guard)
}

/// The flag `minidumper-child` appends to the re-exec'd crash-reporter process
/// (its default `server_arg`). Its presence is how that crate — and we — tell
/// the reporter process apart from the main app process.
const CRASH_REPORTER_SERVER_ARG: &str = "--crash-reporter-server";

/// How many times the **main** process tries to start the native reporter
/// before giving up. Each attempt spawns a fresh reporter with a new socket
/// name, so a transient bind collision (`EADDRINUSE`) with a leftover socket
/// file resolves on the next try.
const MINIDUMP_INIT_ATTEMPTS: u32 = 3;

/// Delay between reporter start attempts.
const MINIDUMP_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// Only leftover reporter socket files older than this are swept, so a live
/// reporter's freshly created socket is never removed.
const REPORTER_SOCKET_MAX_AGE: std::time::Duration = std::time::Duration::from_hours(1);

/// Whether this process is the re-exec'd crash-reporter server (it carries
/// minidumper-child's server flag) rather than the main app process. Mirrors
/// `minidumper_child`'s own argv check.
fn is_crash_reporter_process() -> bool {
    std::env::args().any(|arg| arg.starts_with(CRASH_REPORTER_SERVER_ARG))
}

/// Name of a leftover reporter IPC socket file (minidumper's
/// `temp-socket-<uuid>`).
fn is_reporter_socket_name(name: &str) -> bool {
    name.starts_with("temp-socket-")
}

/// Remove leftover crash-reporter IPC socket files from the temp dir. On
/// Windows/macOS minidumper binds an `AF_UNIX` socket at
/// `<temp>/temp-socket-<uuid>` and never deletes it, so they accumulate; a
/// leftover file also makes a reporter's `bind` fail with `EADDRINUSE`. Only
/// files older than [`REPORTER_SOCKET_MAX_AGE`] are removed, so a live
/// reporter's socket is left alone. Best-effort — errors are ignored. (Linux
/// uses abstract sockets, so its temp dir has none of these and this is a no-op
/// there.)
fn sweep_stale_reporter_sockets() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_str()
            .is_some_and(is_reporter_socket_name)
        {
            continue;
        }
        let too_old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > REPORTER_SOCKET_MAX_AGE);
        if too_old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Arms the native minidump handler: a re-exec'd reporter process that outlives
/// a crash of the main process and uploads a minidump. Returns the handle,
/// which **must be kept alive for the whole program** — dropping it stops the
/// reporter.
///
/// `None` when reporting is off (no Sentry client) *or* the reporter failed to
/// start after [`MINIDUMP_INIT_ATTEMPTS`]. The reporter's IPC socket bind can
/// hit `EADDRINUSE` (a leftover socket file); the main process sweeps stale
/// sockets and retries with a fresh reporter to recover. A final failure is
/// logged at `error!` so it becomes a Sentry event in its own right: without
/// this we can't tell a machine where native capture is silently unavailable
/// from one that simply never crashed. Only covers the **main** process — a
/// `WebView2` (Windows) or `WKWebView` renderer crash is a separate process and
/// is not caught here (see the macOS-only `on_web_content_process_terminate`
/// hook in `main`).
pub fn arm_minidump_reporter(
    client: Option<&sentry::ClientInitGuard>,
) -> Option<tauri_plugin_sentry::minidump::Handle> {
    let client = client?;
    let reporter_process = is_crash_reporter_process();

    // The reporter process gets a single attempt — its socket name is fixed by
    // the parent's flag, so a retry would only re-bind the same failing name.
    // The main process sweeps stale sockets once, then retries with fresh
    // reporters.
    let attempts = if reporter_process {
        1
    } else {
        sweep_stale_reporter_sockets();
        MINIDUMP_INIT_ATTEMPTS
    };

    for attempt in 1..=attempts {
        match tauri_plugin_sentry::minidump::init(client) {
            Ok(handle) => {
                tracing::info!("native crash reporter armed");
                return Some(handle);
            }
            Err(err) if attempt < attempts => {
                tracing::warn!(
                    "native crash reporter start attempt {attempt}/{attempts} failed: {err}; retrying"
                );
                std::thread::sleep(MINIDUMP_RETRY_DELAY);
            }
            Err(err) => {
                tracing::error!(
                    "native crash reporter failed to start; native crashes will not be captured: {err}"
                );
                // In the reporter process a failed start means minidumper-child
                // returned before its own `exit(0)`, so `main` would go on to
                // run a second, half-initialised copy of the app. Exit instead.
                if reporter_process {
                    std::process::exit(1);
                }
                return None;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{is_reporter_socket_name, reporting_enabled};
    use serde_json::json;

    #[test]
    fn reporter_socket_names_are_recognized() {
        assert!(is_reporter_socket_name(
            "temp-socket-2f1a9c0b4d5e4f6a8b7c1d2e3f405162"
        ));
        // Don't sweep unrelated temp files.
        assert!(!is_reporter_socket_name("settings.json"));
        assert!(!is_reporter_socket_name(""));
    }

    #[test]
    fn opt_out_is_honored() {
        let store = json!({ "crash_reporting": false });
        assert!(!reporting_enabled(Some(&store)));
    }

    #[test]
    fn enabled_by_default() {
        // Absent store (fresh install), absent key, and explicit true all
        // enable.
        assert!(reporting_enabled(None));
        assert!(reporting_enabled(Some(&json!({}))));
        assert!(reporting_enabled(Some(&json!({ "crash_reporting": true }))));
    }

    #[test]
    fn malformed_store_fails_open() {
        // A non-bool value or a non-object store must not silently disable
        // reporting (fail-open), matching the disk-read fallback.
        assert!(reporting_enabled(Some(
            &json!({ "crash_reporting": "false" })
        )));
        assert!(reporting_enabled(Some(&json!([1, 2, 3]))));
    }
}
