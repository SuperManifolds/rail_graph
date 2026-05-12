use wasm_bindgen::JsValue;

/// Format an RFC3339 timestamp into a human-readable local date/time string
/// using the browser's Intl.DateTimeFormat API
#[must_use]
pub fn format_rfc3339_local(rfc3339: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(rfc3339) else {
        return rfc3339.to_string();
    };

    let timestamp_millis = dt.timestamp_millis();
    #[allow(clippy::cast_precision_loss)]
    let js_date = js_sys::Date::new(&JsValue::from_f64(timestamp_millis as f64));

    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"dateStyle".into(), &"medium".into());
    let _ = js_sys::Reflect::set(&options, &"timeStyle".into(), &"short".into());

    let formatter = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &options);

    // Call the format function with the date
    let format_fn = formatter.format();
    let result = format_fn.call1(&JsValue::NULL, &js_date);

    result
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| rfc3339.to_string())
}
