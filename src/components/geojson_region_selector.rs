use leptos::{component, create_effect, create_node_ref, create_signal, view, Callable, Callback, IntoView, ReadSignal, Signal, SignalGet, SignalSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys;

#[derive(Debug, Clone)]
pub struct StationData {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct SelectionBounds {
    pub min_lat: f64,
    pub min_lng: f64,
    pub max_lat: f64,
    pub max_lng: f64,
}

const MAX_STATIONS: usize = 1000;

fn request_animation_frame<F>(f: F)
where
    F: FnOnce() + 'static,
{
    let closure = Closure::once(f);
    if let Some(window) = web_sys::window() {
        let _ = window.request_animation_frame(closure.as_ref().unchecked_ref());
        closure.forget();
    }
}

fn invalidate_map_size(map: &JsValue) {
    if let Ok(invalidate_fn) = js_sys::Reflect::get(map, &JsValue::from_str("invalidateSize")) {
        if let Some(func) = invalidate_fn.dyn_ref::<js_sys::Function>() {
            // Call with options: {animate: false, pan: false}
            let options = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&options, &JsValue::from_str("animate"), &JsValue::from_bool(false));
            let _ = js_sys::Reflect::set(&options, &JsValue::from_str("pan"), &JsValue::from_bool(false));
            let _ = func.call1(map, &options);
        }
    }
}

fn invalidate_map_size_delayed(map: JsValue) {
    request_animation_frame(move || {
        let map_clone = map.clone();
        request_animation_frame(move || {
            invalidate_map_size(&map_clone);
        });
    });
}

fn setup_resize_observer(container: &web_sys::HtmlElement, map: JsValue) {
    let map_for_resize = map.clone();

    let callback = Closure::wrap(Box::new(move |_entries: JsValue| {
        invalidate_map_size(&map_for_resize);
    }) as Box<dyn FnMut(JsValue)>);

    let Some(window) = web_sys::window() else { return };
    let Ok(observer_constructor) = js_sys::Reflect::get(&window, &JsValue::from_str("ResizeObserver")) else { return };
    let Some(constructor) = observer_constructor.dyn_ref::<js_sys::Function>() else { return };

    let args = js_sys::Array::new();
    args.push(callback.as_ref().unchecked_ref());

    let Ok(observer) = js_sys::Reflect::construct(constructor, &args) else { return };
    let Ok(observe_fn) = js_sys::Reflect::get(&observer, &JsValue::from_str("observe")) else { return };
    let Some(observe) = observe_fn.dyn_ref::<js_sys::Function>() else { return };

    let _ = observe.call1(&observer, container);
    callback.forget();
}

#[component]
#[must_use]
pub fn GeoJsonRegionSelector(
    stations: ReadSignal<Vec<StationData>>,
    on_import: Callback<SelectionBounds>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let map_container_ref = create_node_ref::<leptos::html::Div>();
    let (selection_bounds, set_selection_bounds) = create_signal(None::<SelectionBounds>);
    let (selected_count, set_selected_count) = create_signal(0);
    let (map_instance, set_map_instance) = create_signal(None::<JsValue>);
    let (marker_layer, set_marker_layer) = create_signal(None::<JsValue>);

    // Default center (will be updated when stations load)
    let center_lat = 59.9;
    let center_lng = 10.75;

    // Initialize Leaflet map when container is mounted
    create_effect(move |_| {
        if let Some(container) = map_container_ref.get() {
            let container_element: &web_sys::HtmlElement = &container;

            // Call Leaflet to initialize map (without stations initially)
            match init_leaflet_map(
                container_element,
                center_lat,
                center_lng,
            ) {
                Ok((map, layer)) => {
                    // Store map instance and marker layer
                    set_map_instance.set(Some(map.clone()));
                    set_marker_layer.set(Some(layer));

                    // Initialize area selection immediately
                    leptos::logging::log!("Initializing area selection...");
                    match enable_area_selection(&map, set_selection_bounds) {
                        Ok(()) => leptos::logging::log!("Area selection initialized successfully"),
                        Err(e) => leptos::logging::error!("Failed to initialize area selection: {:?}", e),
                    }

                    // Invalidate size after initial mount with delay
                    invalidate_map_size_delayed(map.clone());

                    // Set up ResizeObserver to handle container resizing
                    setup_resize_observer(container_element, map);
                }
                Err(e) => {
                    leptos::logging::error!("Failed to initialize Leaflet map: {:?}", e);
                }
            }
        }
    });


    // Update markers when selection changes or stations load
    create_effect(move |_| {
        let current_stations = stations.get();
        let current_selection = selection_bounds.get();

        let Some(layer) = marker_layer.get() else { return };
        let Some(map) = map_instance.get() else { return };

        if !current_stations.is_empty() {
            // Count stations in selection
            if let Some(bounds) = &current_selection {
                let count = count_stations_in_bounds(&current_stations, bounds);
                set_selected_count.set(count);
                leptos::logging::log!("Selection updated: {} stations in bounds", count);
            } else {
                set_selected_count.set(0);
            }

            render_markers_with_selection(&layer, &map, &current_stations, current_selection.as_ref());
        }
    });

    let can_import = Signal::derive(move || {
        selection_bounds.get().is_some() && selected_count.get() > 0 && selected_count.get() <= MAX_STATIONS
    });

    let handle_import = move |_| {
        if let Some(bounds) = selection_bounds.get() {
            on_import.call(bounds);
        }
    };

    let handle_cancel = move |_| {
        on_cancel.call(());
    };

    view! {
        <div class="geojson-region-selector">
            <div class="instructions">
                {move || {
                    let station_list = stations.get();
                    if station_list.is_empty() {
                        view! {
                            <p>"Loading stations from GeoJSON file..."</p>
                        }.into_view()
                    } else {
                        view! {
                            <p>"Click the button in the top-right corner of the map to start. Click to add points. Click the first point again to complete the selection."</p>
                        }.into_view()
                    }
                }}
                <p class="station-count">
                    {move || {
                        let station_list = stations.get();
                        if station_list.is_empty() {
                            "Preparing map...".to_string()
                        } else {
                            let has_selection = selection_bounds.get().is_some();
                            let count = selected_count.get();

                            if !has_selection {
                                format!("Loaded {} stations - No region selected", station_list.len())
                            } else if count == 0 {
                                "⚠ Selected region contains 0 stations".to_string()
                            } else if count > MAX_STATIONS {
                                format!("⚠ Selected: {count} stations (exceeds limit of {MAX_STATIONS})")
                            } else {
                                format!("✓ Selected: {count} stations")
                            }
                        }
                    }}
                </p>
            </div>

            <div
                class="map-container"
                node_ref=map_container_ref
                style="width: 100%;"
            ></div>

            <div class="form-buttons">
                <button on:click=handle_cancel>
                    "Cancel"
                </button>
                <button
                    class="primary"
                    on:click=handle_import
                    prop:disabled=move || !can_import.get()
                >
                    "Import Selected Region"
                </button>
            </div>
        </div>
    }
}

fn init_leaflet_map(
    container: &web_sys::HtmlElement,
    center_lat: f64,
    center_lng: f64,
) -> Result<(JsValue, JsValue), JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    let l = js_sys::Reflect::get(&window, &JsValue::from_str("L"))?;

    // Create map: L.map(container).setView([lat, lng], zoom)
    let map_fn = js_sys::Reflect::get(&l, &JsValue::from_str("map"))?;
    let map_fn = map_fn.dyn_ref::<js_sys::Function>().ok_or("L.map not a function")?;
    let map = map_fn.call1(&l, container)?;

    // Set view: map.setView([lat, lng], zoom)
    let center_array = js_sys::Array::new();
    center_array.push(&JsValue::from_f64(center_lat));
    center_array.push(&JsValue::from_f64(center_lng));

    let set_view_fn = js_sys::Reflect::get(&map, &JsValue::from_str("setView"))?;
    let set_view_fn = set_view_fn.dyn_ref::<js_sys::Function>().ok_or("setView not a function")?;
    set_view_fn.call2(&map, &center_array, &JsValue::from_f64(10.0))?;

    // Add OSM tile layer: L.tileLayer(url, options).addTo(map)
    let tile_layer_fn = js_sys::Reflect::get(&l, &JsValue::from_str("tileLayer"))?;
    let tile_layer_fn = tile_layer_fn.dyn_ref::<js_sys::Function>().ok_or("tileLayer not a function")?;

    let tile_url = "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png";
    let tile_options = js_sys::Object::new();
    js_sys::Reflect::set(&tile_options, &JsValue::from_str("attribution"), &JsValue::from_str("© OpenStreetMap contributors"))?;
    js_sys::Reflect::set(&tile_options, &JsValue::from_str("maxZoom"), &JsValue::from_f64(19.0))?;

    let tile_layer = tile_layer_fn.call2(&l, &JsValue::from_str(tile_url), &tile_options)?;
    let add_to_fn = js_sys::Reflect::get(&tile_layer, &JsValue::from_str("addTo"))?;
    let add_to_fn = add_to_fn.dyn_ref::<js_sys::Function>().ok_or("addTo not a function")?;
    add_to_fn.call1(&tile_layer, &map)?;

    // Create layer group for markers
    let layer_group_fn = js_sys::Reflect::get(&l, &JsValue::from_str("layerGroup"))?;
    let layer_group_fn = layer_group_fn.dyn_ref::<js_sys::Function>().ok_or("layerGroup not a function")?;
    let marker_layer = layer_group_fn.call0(&l)?;

    let add_to_fn = js_sys::Reflect::get(&marker_layer, &JsValue::from_str("addTo"))?;
    let add_to_fn = add_to_fn.dyn_ref::<js_sys::Function>().ok_or("addTo not a function")?;
    add_to_fn.call1(&marker_layer, &map)?;

    Ok((map, marker_layer))
}

fn count_stations_in_bounds(stations: &[StationData], bounds: &SelectionBounds) -> usize {
    stations.iter().filter(|s| {
        s.lat >= bounds.min_lat && s.lat <= bounds.max_lat &&
        s.lng >= bounds.min_lng && s.lng <= bounds.max_lng
    }).count()
}

fn fit_bounds_to_stations(map: &JsValue, stations: &[StationData]) -> Result<(), JsValue> {
    if stations.is_empty() {
        return Ok(());
    }

    // Calculate bounds from all stations
    let mut min_lat = stations[0].lat;
    let mut max_lat = stations[0].lat;
    let mut min_lng = stations[0].lng;
    let mut max_lng = stations[0].lng;

    for station in stations {
        min_lat = min_lat.min(station.lat);
        max_lat = max_lat.max(station.lat);
        min_lng = min_lng.min(station.lng);
        max_lng = max_lng.max(station.lng);
    }

    // Get window.L
    let window = web_sys::window().ok_or("No window")?;
    let l = js_sys::Reflect::get(&window, &JsValue::from_str("L"))?;

    // Create bounds: L.latLngBounds([[min_lat, min_lng], [max_lat, max_lng]])
    let lat_lng_bounds_fn = js_sys::Reflect::get(&l, &JsValue::from_str("latLngBounds"))?;
    let lat_lng_bounds_fn = lat_lng_bounds_fn.dyn_ref::<js_sys::Function>()
        .ok_or("latLngBounds not a function")?;

    // Create southwest corner [min_lat, min_lng]
    let sw_corner = js_sys::Array::new();
    sw_corner.push(&JsValue::from_f64(min_lat));
    sw_corner.push(&JsValue::from_f64(min_lng));

    // Create northeast corner [max_lat, max_lng]
    let ne_corner = js_sys::Array::new();
    ne_corner.push(&JsValue::from_f64(max_lat));
    ne_corner.push(&JsValue::from_f64(max_lng));

    // Create bounds
    let bounds = lat_lng_bounds_fn.call2(&l, &sw_corner, &ne_corner)?;

    // Create options with padding
    let options = js_sys::Object::new();
    let padding = js_sys::Array::new();
    padding.push(&JsValue::from_f64(50.0));
    padding.push(&JsValue::from_f64(50.0));
    js_sys::Reflect::set(&options, &JsValue::from_str("padding"), &padding)?;

    // Call map.fitBounds(bounds, options)
    let fit_bounds_fn = js_sys::Reflect::get(map, &JsValue::from_str("fitBounds"))?;
    let fit_bounds_fn = fit_bounds_fn.dyn_ref::<js_sys::Function>()
        .ok_or("fitBounds not a function")?;
    fit_bounds_fn.call2(map, &bounds, &options)?;

    Ok(())
}

fn render_markers_with_selection(
    layer: &JsValue,
    map: &JsValue,
    stations: &[StationData],
    selection: Option<&SelectionBounds>,
) {
    // Clear existing markers
    if let Ok(clear_layers) = js_sys::Reflect::get(layer, &JsValue::from_str("clearLayers")) {
        if let Some(clear_fn) = clear_layers.dyn_ref::<js_sys::Function>() {
            let _ = clear_fn.call0(layer);
        }
    }

    let Some(window) = web_sys::window() else { return };
    let Ok(l) = js_sys::Reflect::get(&window, &JsValue::from_str("L")) else { return };

    for station in stations {
        let is_selected = selection.is_some_and(|bounds| {
            station.lat >= bounds.min_lat && station.lat <= bounds.max_lat &&
            station.lng >= bounds.min_lng && station.lng <= bounds.max_lng
        });

        if let Err(e) = add_station_marker(&l, layer, station, is_selected) {
            leptos::logging::error!("Failed to add station marker: {:?}", e);
        }
    }

    // Fit map view to show all stations (only if no selection yet)
    if selection.is_none() {
        let _ = fit_bounds_to_stations(map, stations);
    }
}

fn add_station_marker(
    l: &JsValue,
    layer: &JsValue,
    station: &StationData,
    is_selected: bool,
) -> Result<(), JsValue> {
    // L.circleMarker([lat, lng], options)
    let circle_marker_fn = js_sys::Reflect::get(l, &JsValue::from_str("circleMarker"))?;
    let circle_marker_fn = circle_marker_fn.dyn_ref::<js_sys::Function>().ok_or("circleMarker not a function")?;

    let coords = js_sys::Array::new();
    coords.push(&JsValue::from_f64(station.lat));
    coords.push(&JsValue::from_f64(station.lng));

    // Use different colors for selected vs unselected stations
    let (color, fill_color) = if is_selected {
        ("#00ff00", "#00ff00")  // Green for selected
    } else {
        ("#ff0000", "#ff0000")  // Red for unselected
    };

    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &JsValue::from_str("radius"), &JsValue::from_f64(3.0))?;
    js_sys::Reflect::set(&options, &JsValue::from_str("color"), &JsValue::from_str(color))?;
    js_sys::Reflect::set(&options, &JsValue::from_str("fillColor"), &JsValue::from_str(fill_color))?;
    js_sys::Reflect::set(&options, &JsValue::from_str("fillOpacity"), &JsValue::from_f64(0.6))?;

    let marker = circle_marker_fn.call2(l, &coords, &options)?;

    // Bind popup with station name
    let bind_popup_fn = js_sys::Reflect::get(&marker, &JsValue::from_str("bindPopup"))?;
    let bind_popup_fn = bind_popup_fn.dyn_ref::<js_sys::Function>().ok_or("bindPopup not a function")?;
    bind_popup_fn.call1(&marker, &JsValue::from_str(&station.name))?;

    // Add to layer
    let add_to_fn = js_sys::Reflect::get(&marker, &JsValue::from_str("addTo"))?;
    let add_to_fn = add_to_fn.dyn_ref::<js_sys::Function>().ok_or("addTo not a function")?;
    add_to_fn.call1(&marker, layer)?;

    Ok(())
}

fn enable_area_selection(
    map: &JsValue,
    set_selection_bounds: leptos::WriteSignal<Option<SelectionBounds>>,
) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("No window")?;

    // Access window.leafletAreaSelection.DrawAreaSelection
    let las_ns = js_sys::Reflect::get(&window, &JsValue::from_str("leafletAreaSelection"))
        .map_err(|_| "leafletAreaSelection not found on window")?;
    let draw_class = js_sys::Reflect::get(&las_ns, &JsValue::from_str("DrawAreaSelection"))
        .map_err(|_| "DrawAreaSelection not found on leafletAreaSelection")?;

    // Create callback - must take (polygon, control) as per library source
    let on_polygon_ready = Closure::wrap(Box::new(move |polygon: JsValue, _control: JsValue| {
        leptos::logging::log!("onPolygonReady fired!");
        extract_and_set_bounds(polygon, set_selection_bounds);
    }) as Box<dyn FnMut(JsValue, JsValue)>);

    // Create config object
    let config = js_sys::Object::new();

    // Set the callback - MUST use the function reference directly
    let callback_fn = on_polygon_ready.as_ref().unchecked_ref::<js_sys::Function>();
    js_sys::Reflect::set(&config, &JsValue::from_str("onPolygonReady"), callback_fn)?;

    leptos::logging::log!("Config created with callback");

    // Create DrawAreaSelection control with config
    let args = js_sys::Array::new();
    args.push(&config);

    let draw_constructor = draw_class.dyn_ref::<js_sys::Function>()
        .ok_or("DrawAreaSelection is not a constructor")?;
    let control = js_sys::Reflect::construct(draw_constructor, &args)?;

    leptos::logging::log!("Control constructed");

    // Add control to map
    let add_control = js_sys::Reflect::get(map, &JsValue::from_str("addControl"))?;
    let add_control_fn = add_control.dyn_ref::<js_sys::Function>()
        .ok_or("addControl not a function")?;
    add_control_fn.call1(map, &control)?;

    leptos::logging::log!("Control added to map");

    on_polygon_ready.forget();

    Ok(())
}

fn extract_and_set_bounds(polygon: JsValue, set_selection_bounds: leptos::WriteSignal<Option<SelectionBounds>>) {
    // Get bounds from the polygon
    let Ok(get_bounds) = js_sys::Reflect::get(&polygon, &JsValue::from_str("getBounds")) else {
        leptos::logging::error!("Failed to get getBounds");
        return;
    };
    let Some(bounds_fn) = get_bounds.dyn_ref::<js_sys::Function>() else {
        leptos::logging::error!("getBounds not a function");
        return;
    };
    let Ok(bounds) = bounds_fn.call0(&polygon) else {
        leptos::logging::error!("Failed to call getBounds");
        return;
    };

    // Extract southwest and northeast corners
    let Ok(sw_val) = js_sys::Reflect::get(&bounds, &JsValue::from_str("getSouthWest")) else { return };
    let Ok(ne_val) = js_sys::Reflect::get(&bounds, &JsValue::from_str("getNorthEast")) else { return };
    let Some(sw_fn) = sw_val.dyn_ref::<js_sys::Function>() else { return };
    let Some(ne_fn) = ne_val.dyn_ref::<js_sys::Function>() else { return };
    let Ok(sw) = sw_fn.call0(&bounds) else { return };
    let Ok(ne) = ne_fn.call0(&bounds) else { return };

    // Extract lat/lng values
    let Ok(sw_lat) = js_sys::Reflect::get(&sw, &JsValue::from_str("lat")) else { return };
    let Ok(sw_lng) = js_sys::Reflect::get(&sw, &JsValue::from_str("lng")) else { return };
    let Ok(ne_lat) = js_sys::Reflect::get(&ne, &JsValue::from_str("lat")) else { return };
    let Ok(ne_lng) = js_sys::Reflect::get(&ne, &JsValue::from_str("lng")) else { return };

    let Some(min_lat) = sw_lat.as_f64() else { return };
    let Some(min_lng) = sw_lng.as_f64() else { return };
    let Some(max_lat) = ne_lat.as_f64() else { return };
    let Some(max_lng) = ne_lng.as_f64() else { return };

    let selection = SelectionBounds {
        min_lat,
        min_lng,
        max_lat,
        max_lng,
    };

    leptos::logging::log!("Bounds: lat {}-{}, lng {}-{}", min_lat, max_lat, min_lng, max_lng);
    set_selection_bounds.set(Some(selection));
}

