use crate::providers::{calculate_route, parse_coordinates, search_places, TravelMode};
use crate::settings;
use crate::ui::MainWindow;
use gtk::glib;
use gtk::prelude::*;
use nixe_core::control::Reply;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

pub fn tools() -> Value {
    json!([
        {
            "name": "maps.search",
            "description": "Search for places, addresses, or businesses and return coordinates.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "maps.show",
            "description": "Show a specific location or place on the map.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "lat": { "type": "number" },
                    "lon": { "type": "number" },
                    "label": { "type": "string" }
                },
                "required": ["lat", "lon"]
            }
        },
        {
            "name": "maps.distance",
            "description": "Calculate distance and travel duration between two locations.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start": { "type": "string", "description": "start lat,lon" },
                    "end": { "type": "string", "description": "end lat,lon" },
                    "mode": { "type": "string", "enum": ["drive", "walk", "bicycle"] }
                },
                "required": ["start", "end"]
            }
        },
        {
            "name": "maps.route",
            "description": "Calculate and display a route on the map.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start": { "type": "string" },
                    "end": { "type": "string" },
                    "mode": { "type": "string" }
                },
                "required": ["start", "end"]
            }
        },
        {
            "name": "maps.nearby",
            "description": "Find places of a specific category (food, groceries, fuel, entertainment, parks) near coordinates or map center.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": { "type": "string", "enum": ["food", "groceries", "fuel", "entertainment", "parks"] },
                    "lat": { "type": "number" },
                    "lon": { "type": "number" },
                    "radius_km": { "type": "number" }
                },
                "required": ["category"]
            }
        }
    ])
}

fn respond_async<F>(reply: Reply, f: F)
where
    F: std::future::Future<Output = Result<Value, String>> + Send + 'static,
{
    let (tx, rx) = async_channel::bounded::<Result<Value, String>>(1);
    crate::runtime::spawn(async move {
        let res = f.await;
        let _ = tx.send(res).await;
    });
    glib::spawn_future_local(async move {
        if let Ok(res) = rx.recv().await {
            reply(res);
        }
    });
}

fn respond_async_gui<F, G>(reply: Reply, f: F, on_gui: G)
where
    F: std::future::Future<Output = Result<Value, String>> + Send + 'static,
    G: FnOnce(&Value) + 'static,
{
    let (tx, rx) = async_channel::bounded::<Result<Value, String>>(1);
    crate::runtime::spawn(async move {
        let res = f.await;
        let _ = tx.send(res).await;
    });
    glib::spawn_future_local(async move {
        if let Ok(res) = rx.recv().await {
            if let Ok(ref val) = res {
                on_gui(val);
            }
            reply(res);
        }
    });
}

pub fn handle(name: &str, args: Value, win_ref: Rc<RefCell<Option<Rc<MainWindow>>>>, reply: Reply) {
    let api_key = settings::get().geoapify_api_key;

    match name {
        "maps.search" => {
            let query = match args["query"].as_str() {
                Some(q) => q.to_string(),
                None => return reply(Err("query is required".into())),
            };
            respond_async(reply, async move {
                match search_places(&query, &api_key).await {
                    Ok(places) => Ok(json!(places)),
                    Err(e) => Err(e.to_string()),
                }
            });
        }
        "maps.show" => {
            let lat = match args["lat"].as_f64() {
                Some(l) => l,
                None => return reply(Err("lat is required".into())),
            };
            let lon = match args["lon"].as_f64() {
                Some(l) => l,
                None => return reply(Err("lon is required".into())),
            };
            let label = args["label"].as_str().unwrap_or("Selected Location");

            if let Some(ref win) = *win_ref.borrow() {
                win.open_location(lat, lon, label);
                win.window.present();
                reply(Ok(json!({ "shown": true, "lat": lat, "lon": lon })));
            } else {
                reply(Err("Maps window not open".into()));
            }
        }
        "maps.distance" => {
            let start = match args["start"].as_str().and_then(parse_coordinates) {
                Some(c) => c,
                None => return reply(Err("start must be 'lat,lon'".into())),
            };
            let end = match args["end"].as_str().and_then(parse_coordinates) {
                Some(c) => c,
                None => return reply(Err("end must be 'lat,lon'".into())),
            };
            let mode = TravelMode::from_str(args["mode"].as_str().unwrap_or("drive"));

            respond_async(reply, async move {
                match calculate_route(start, end, mode, &api_key).await {
                    Ok(route) => Ok(json!({
                        "mode": route.mode.display_name(),
                        "distance_meters": route.distance_meters,
                        "duration_seconds": route.duration_seconds,
                        "formatted_distance": crate::providers::format_distance(route.distance_meters, &settings::get().units),
                        "formatted_duration": crate::providers::format_duration(route.duration_seconds),
                    })),
                    Err(e) => Err(e.to_string()),
                }
            });
        }
        "maps.route" => {
            let start = match args["start"].as_str().and_then(parse_coordinates) {
                Some(c) => c,
                None => return reply(Err("start must be 'lat,lon'".into())),
            };
            let end = match args["end"].as_str().and_then(parse_coordinates) {
                Some(c) => c,
                None => return reply(Err("end must be 'lat,lon'".into())),
            };
            let mode = args["mode"].as_str().unwrap_or("drive").to_string();

            if let Some(ref win) = *win_ref.borrow() {
                win.open_route(start, end, &mode);
                win.window.present();
                reply(Ok(json!({ "routed": true })));
            } else {
                reply(Err("Maps window not open".into()));
            }
        }
        "maps.nearby" => {
            let category = match args["category"].as_str() {
                Some(c) => c.to_string(),
                None => return reply(Err("category is required".into())),
            };
            let (lat, lon) = if let (Some(la), Some(lo)) = (args["lat"].as_f64(), args["lon"].as_f64()) {
                (la, lo)
            } else if let Some(ref win) = *win_ref.borrow() {
                let (la, lo, _) = win.map_view.get_viewport_state();
                if la == 0.0 && lo == 0.0 { (33.1207, -86.7601) } else { (la, lo) }
            } else {
                (33.1207, -86.7601)
            };
            let radius_km = args["radius_km"].as_f64().unwrap_or(8.0);
            let cat_query = category.clone();

            let win_opt = win_ref.clone();
            respond_async_gui(
                reply,
                async move {
                    match crate::providers::search_nearby(&cat_query, lat, lon, radius_km, &api_key).await {
                        Ok(places) => Ok(json!(places)),
                        Err(e) => Err(e.to_string()),
                    }
                },
                move |val| {
                    if let Ok(places) = serde_json::from_value::<Vec<crate::providers::SearchResult>>(val.clone()) {
                        let cat_label = match category.as_str() {
                            "food" => "Food nearby",
                            "groceries" => "Groceries nearby",
                            "fuel" => "Fuel nearby",
                            "entertainment" => "Entertainment nearby",
                            "parks" => "Parks nearby",
                            _ => "Places nearby",
                        };
                        if let Some(ref win) = *win_opt.borrow() {
                            win.search_bar.show_results(cat_label, places);
                            win.window.present();
                        }
                    }
                },
            );
        }
        _ => reply(Err(format!("unknown tool {name}"))),
    }
}
