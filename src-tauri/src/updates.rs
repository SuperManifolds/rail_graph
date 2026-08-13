use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_store::StoreExt;
use tauri_plugin_updater::UpdaterExt;

const CHECK_FOR_UPDATES_KEY: &str = "check_for_updates";

/// Whether the user has enabled automatic update checks. Defaults to true when
/// the setting is absent so fresh installs receive updates.
fn auto_check_enabled(app: &tauri::AppHandle) -> bool {
    let Ok(store) = app.store(crate::crash_reporting::SETTINGS_STORE) else {
        return true;
    };
    store
        .get(CHECK_FOR_UPDATES_KEY)
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// Silent startup check: any failure (offline, no release published yet) is
/// ignored so it never blocks or bothers the user.
pub async fn check_on_startup(app: tauri::AppHandle) {
    if !auto_check_enabled(&app) {
        return;
    }
    let Ok(updater) = app.updater() else { return };
    let Ok(Some(update)) = updater.check().await else {
        return;
    };
    prompt_install(&app, update).await;
}

/// Manual check from the "Check for Updates…" menu item: every outcome gets a
/// dialog so the user always sees a response to their action.
pub async fn check_interactive(app: &tauri::AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            app.dialog()
                .message(format!("Updater unavailable: {e}"))
                .title("Update Error")
                .blocking_show();
            return;
        }
    };
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            app.dialog()
                .message("You're running the latest version.")
                .title("No Updates Available")
                .blocking_show();
            return;
        }
        Err(e) => {
            app.dialog()
                .message(format!("Failed to check for updates: {e}"))
                .title("Update Error")
                .blocking_show();
            return;
        }
    };
    prompt_install(app, update).await;
}

async fn prompt_install(app: &tauri::AppHandle, update: tauri_plugin_updater::Update) {
    let version = update.version.clone();
    let confirmed = app
        .dialog()
        .message(format!("Version {version} is available. Download and install?"))
        .title("Update Available")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Install & Restart".into(),
            "Later".into(),
        ))
        .blocking_show();

    if !confirmed {
        return;
    }

    log::info!("installing update {version}");
    if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
        // error! so failed updates are visible in logs/crash reporting, not
        // just a dialog the user dismisses.
        log::error!("update to {version} failed: {e}");
        app.dialog()
            .message(format!("Update failed: {e}"))
            .title("Update Error")
            .blocking_show();
        return;
    }
    app.restart();
}
