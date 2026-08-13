// Forwards wasm panics and IPC/command failures to Sentry when the
// tauri-plugin-sentry global (window.Sentry) is present. Every function is a
// no-op outside the packaged app (plain browser / dev), where window.Sentry is
// absent, and never throws.

// How long to keep polling for the injected Sentry client before giving up
// (the plugin injects it at webview init, usually before this runs).
var WASM_ATTACH_MAX_TRIES = 50;
var WASM_ATTACH_INTERVAL_MS = 100;

// Wire the vendored @sentry/wasm integration so wasm stack frames symbolicate to
// Rust function + file:line (server-side, against the uploaded debug file, keyed
// by the build_id baked into the wasm). Patch WebAssembly synchronously NOW —
// this script runs before the deferred wasm module below it — so the module is
// registered as a debug image the moment it instantiates. Then attach the
// integration to the plugin-injected client (for the processEvent that rewrites
// frames + adds debug_meta) as soon as that client exists.
(function() {
    var sw = window.__SentryWasm;
    if (!sw) {
        return;
    }
    try {
        sw.patchWebAssembly();
    } catch (e) {}
    var tries = 0;
    function attach() {
        try {
            var client = window.Sentry && window.Sentry.getClient && window.Sentry.getClient();
            if (client) {
                client.addIntegration(sw.wasmIntegration());
                return;
            }
        } catch (e) {
            return;
        }
        if (tries++ < WASM_ATTACH_MAX_TRIES) {
            setTimeout(attach, WASM_ATTACH_INTERVAL_MS);
        }
    }
    attach();
})();

window.__railgraph_report_panic = function(msg) {
    try {
        if (window.Sentry && window.Sentry.captureException) {
            window.Sentry.captureException(new Error(msg));
        }
    } catch (e) {}
};

window.__railgraph_report_error = function(context, detail) {
    try {
        if (window.Sentry && window.Sentry.captureMessage) {
            window.Sentry.captureMessage(context + ": " + detail, "error");
        }
    } catch (e) {}
};

// Record a breadcrumb — a step in the trail Sentry attaches to the next error,
// so a report reads as "what the user did" rather than a bare stack. Level is
// "info" for actions and "warning" for non-fatal failures.
window.__railgraph_breadcrumb = function(category, message, level) {
    try {
        if (window.Sentry && window.Sentry.addBreadcrumb) {
            window.Sentry.addBreadcrumb({
                category: category,
                message: message,
                level: level || "info",
            });
        }
    } catch (e) {}
};
