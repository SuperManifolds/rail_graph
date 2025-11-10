use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response};

const CACHE_VERSION: &str = "railgraph-v1";
const FONT_AWESOME_URL: &str =
    "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css";

#[wasm_bindgen(start)]
#[allow(clippy::main_recursion)]
fn main() {
    console_error_panic_hook::set_once();

    let global = js_sys::global().unchecked_into::<web_sys::ServiceWorkerGlobalScope>();

    // Install event handler
    let install_closure = Closure::wrap(Box::new(move |event: web_sys::ExtendableEvent| {
        web_sys::console::log_1(&"[SW] Installing service worker...".into());

        let promise = wasm_bindgen_futures::future_to_promise(async move {
            install_handler().await?;
            Ok(JsValue::UNDEFINED)
        });

        let _ = event.wait_until(&promise);
    }) as Box<dyn FnMut(_)>);

    global.set_oninstall(Some(install_closure.as_ref().unchecked_ref()));
    install_closure.forget();

    // Activate event handler
    let activate_closure = Closure::wrap(Box::new(move |event: web_sys::ExtendableEvent| {
        web_sys::console::log_1(&"[SW] Activating service worker...".into());

        let promise = wasm_bindgen_futures::future_to_promise(async move {
            activate_handler().await?;
            Ok(JsValue::UNDEFINED)
        });

        let _ = event.wait_until(&promise);
    }) as Box<dyn FnMut(_)>);

    global.set_onactivate(Some(activate_closure.as_ref().unchecked_ref()));
    activate_closure.forget();

    // Fetch event handler
    let fetch_closure = Closure::wrap(Box::new(move |event: web_sys::FetchEvent| {
        let request = event.request();

        // Skip non-GET requests
        if request.method() != "GET" {
            return;
        }

        let url = request.url();

        // Network-first for API calls
        if url.contains("/api/") {
            let promise = wasm_bindgen_futures::future_to_promise(async move {
                let global = js_sys::global().unchecked_into::<web_sys::ServiceWorkerGlobalScope>();
                let response = JsFuture::from(global.fetch_with_request(&request))
                    .await
                    .inspect_err(|e| {
                        web_sys::console::error_1(e);
                    })?;
                Ok(response)
            });

            let _ = event.respond_with(&promise);
            return;
        }

        // Cache-first for everything else
        let cache_name = format!("{CACHE_VERSION}-app");
        let promise = wasm_bindgen_futures::future_to_promise(async move {
            cache_first_handler(&request, &cache_name).await
        });

        let _ = event.respond_with(&promise);
    }) as Box<dyn FnMut(_)>);

    global.set_onfetch(Some(fetch_closure.as_ref().unchecked_ref()));
    fetch_closure.forget();
}

async fn install_handler() -> Result<JsValue, JsValue> {
    // Fetch asset manifest
    let manifest_response = JsFuture::from(
        js_sys::global()
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .fetch_with_str("/asset-manifest.json"),
    )
    .await?;

    let manifest_response: Response = manifest_response.dyn_into()?;
    let manifest_text = JsFuture::from(manifest_response.text()?)
        .await?
        .as_string()
        .ok_or_else(|| JsValue::from_str("Manifest is not a string"))?;

    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse manifest: {e}")))?;

    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    web_sys::console::log_1(&format!("[SW] Loading asset manifest, version: {version}").into());

    let assets = manifest
        .get("assets")
        .and_then(|a| a.as_array())
        .ok_or_else(|| JsValue::from_str("No assets in manifest"))?;

    web_sys::console::log_1(&format!("[SW] Assets to cache: {}", assets.len()).into());

    // Open cache
    let cache_name = format!("{CACHE_VERSION}-app");
    let cache_storage = js_sys::global()
        .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
        .caches()?;

    let cache = JsFuture::from(cache_storage.open(&cache_name))
        .await?
        .dyn_into::<web_sys::Cache>()?;

    // Cache Font Awesome
    let _ = JsFuture::from(cache.add_with_str(FONT_AWESOME_URL))
        .await
        .map_err(|e| {
            web_sys::console::warn_1(&format!("[SW] Failed to cache Font Awesome: {e:?}").into());
        });

    // Cache all assets from manifest
    for asset in assets {
        if let Some(url) = asset.as_str() {
            let _ = JsFuture::from(cache.add_with_str(url))
                .await
                .map_err(|e| {
                    web_sys::console::warn_1(&format!("[SW] Failed to cache asset: {url} {e:?}").into());
                });
        }
    }

    web_sys::console::log_1(&"[SW] Install complete".into());

    // Skip waiting
    let _ = js_sys::global()
        .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
        .skip_waiting();

    Ok(JsValue::UNDEFINED)
}

async fn activate_handler() -> Result<JsValue, JsValue> {
    let cache_storage = js_sys::global()
        .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
        .caches()?;

    // Get all cache names
    let cache_names = JsFuture::from(cache_storage.keys())
        .await?
        .dyn_into::<js_sys::Array>()?;

    let cache_name = format!("{CACHE_VERSION}-app");

    // Delete old caches
    for i in 0..cache_names.length() {
        if let Some(name) = cache_names.get(i).as_string() {
            if name.starts_with("railgraph-") && name != cache_name {
                web_sys::console::log_1(&format!("[SW] Deleting old cache: {name}").into());
                let _ = JsFuture::from(cache_storage.delete(&name)).await;
            }
        }
    }

    web_sys::console::log_1(&"[SW] Activation complete".into());

    // Claim clients
    JsFuture::from(
        js_sys::global()
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .clients()
            .claim(),
    )
    .await?;

    Ok(JsValue::UNDEFINED)
}

async fn cache_first_handler(request: &Request, cache_name: &str) -> Result<JsValue, JsValue> {
    let cache_storage = js_sys::global()
        .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
        .caches()?;

    // Try to get from cache
    let cached = JsFuture::from(cache_storage.match_with_request(request)).await?;

    if !cached.is_undefined() {
        return Ok(cached);
    }

    // Not in cache, fetch from network
    let global = js_sys::global().unchecked_into::<web_sys::ServiceWorkerGlobalScope>();
    let response_promise = global.fetch_with_request(request);
    let response = JsFuture::from(response_promise).await.map_err(|e| {
        web_sys::console::error_1(&format!("[SW] Fetch failed: {e:?}").into());
        e
    })?;

    let response: Response = response.dyn_into()?;

    // Don't cache if not a successful response
    if response.status() != 200 || response.type_() == web_sys::ResponseType::Error {
        return Ok(response.into());
    }

    // Clone the response for caching
    let response_clone = response.clone()?;

    // Cache the fetched resource (don't await, run in background)
    let cache_name = cache_name.to_string();
    let request_clone = request.clone()?;
    wasm_bindgen_futures::spawn_local(async move {
        if let Ok(cache_storage) = js_sys::global()
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .caches()
        {
            if let Ok(cache) = JsFuture::from(cache_storage.open(&cache_name))
                .await
                .and_then(wasm_bindgen::JsCast::dyn_into::<web_sys::Cache>)
            {
                let _ = JsFuture::from(cache.put_with_request(&request_clone, &response_clone)).await;
            }
        }
    });

    Ok(response.into())
}
