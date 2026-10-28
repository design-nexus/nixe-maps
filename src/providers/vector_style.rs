//! The Standard map: a MapLibre style for OpenFreeMap's vector tiles (the OpenMapTiles
//! schema), drawn by libshumate's vector renderer. Its colours come from the theme:
//! light or dark from the palette, with roads and labels mixed from its background and
//! text so the map sits on the window's surface. Only what libshumate supports is used:
//! background, fill, line and text symbol layers, `get`/`match`/`interpolate` expressions.

use serde_json::{Value, json};

/// OpenFreeMap's TileJSON; its `tiles` URL names the current build and changes weekly.
pub const TILEJSON_URL: &str = "https://tiles.openfreemap.org/planet";

pub const ATTRIBUTION: &str = "© OpenFreeMap, © OpenMapTiles, data © OpenStreetMap contributors";

struct Colours {
    land: String,
    residential: String,
    park: String,
    wood: String,
    water: String,
    building: String,
    boundary: String,
    road_minor: String,
    road_major: String,
    road_motorway: String,
    casing: String,
    rail: String,
    label: String,
    label_dim: String,
    halo: String,
}

fn rgb(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64;
    (p(0), p(2), p(4))
}

/// `a` moved toward `b` by `t` (0–1), as `#rrggbb`.
fn mix(a: &str, b: &str, t: f64) -> String {
    let (a, b) = (rgb(a), rgb(b));
    let m = |x: f64, y: f64| (x + (y - x) * t).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

fn colours(p: &nyx_theme::Palette) -> Colours {
    let (bg, text) = (p.bg.as_str(), p.text.as_str());
    if p.light {
        let land = mix(bg, "#f2efe9", 0.6);
        Colours {
            residential: mix(&land, "#e6e1d8", 0.7),
            park: mix(&land, "#cfe3c2", 0.85),
            wood: mix(&land, "#bcd9ae", 0.85),
            water: mix(&land, "#a8cbe6", 0.9),
            building: mix(&land, "#d9d3c9", 0.8),
            boundary: mix(&land, "#9a8fa8", 0.6),
            road_minor: "#ffffff".into(),
            road_major: mix("#ffffff", "#fbe6b4", 0.6),
            road_motorway: mix("#ffffff", "#f4c98a", 0.75),
            casing: mix(&land, "#b9b1a4", 0.75),
            rail: mix(&land, "#9e978d", 0.7),
            label: mix(text, "#2a2a2a", 0.4),
            label_dim: mix(text, &land, 0.35),
            halo: land.clone(),
            land,
        }
    } else {
        let land = mix(bg, "#ffffff", 0.06);
        Colours {
            residential: mix(&land, "#ffffff", 0.035),
            park: mix(&land, "#3f6b4a", 0.28),
            wood: mix(&land, "#35603f", 0.34),
            water: mix(&land, "#2f4a6b", 0.55),
            building: mix(&land, "#ffffff", 0.08),
            boundary: mix(&land, "#9a8fb8", 0.45),
            road_minor: mix(&land, text, 0.16),
            road_major: mix(&land, text, 0.28),
            road_motorway: mix(&land, "#e2b46a", 0.45),
            casing: mix(&land, "#000000", 0.35),
            rail: mix(&land, text, 0.22),
            label: mix(text, &land, 0.1),
            label_dim: mix(text, &land, 0.4),
            halo: mix(&land, "#000000", 0.3),
            land,
        }
    }
}

/// Width by zoom for a road class: (zoom, width) stops.
fn widths(stops: &[(f64, f64)]) -> Value {
    let mut e = vec![json!("interpolate"), json!(["exponential", 1.5]), json!(["zoom"])];
    for (z, w) in stops {
        e.push(json!(z));
        e.push(json!(w));
    }
    Value::Array(e)
}

fn road(id: &str, classes: &[&str], colour: &str, stops: &[(f64, f64)], minzoom: f64) -> Value {
    json!({
        "id": id, "type": "line", "source": "openmaptiles", "source-layer": "transportation",
        "minzoom": minzoom,
        "filter": ["match", ["get", "class"], classes, true, false],
        "layout": { "line-cap": "round", "line-join": "round" },
        "paint": { "line-color": colour, "line-width": widths(stops) }
    })
}

fn casing(id: &str, classes: &[&str], colour: &str, stops: &[(f64, f64)], minzoom: f64) -> Value {
    let wider: Vec<(f64, f64)> = stops.iter().map(|(z, w)| (*z, w + 1.6)).collect();
    road(id, classes, colour, &wider, minzoom)
}

fn name() -> Value {
    json!(["coalesce", ["get", "name:en"], ["get", "name"]])
}

/// The style JSON for `tiles` (the `{z}/{x}/{y}.pbf` URL from the TileJSON).
pub fn style_json(tiles: &str, palette: &nyx_theme::Palette) -> String {
    let c = colours(palette);
    let motorway: &[(f64, f64)] = &[(5.0, 0.6), (10.0, 2.0), (14.0, 5.0), (18.0, 22.0)];
    let major: &[(f64, f64)] = &[(7.0, 0.4), (10.0, 1.2), (14.0, 4.0), (18.0, 18.0)];
    let minor: &[(f64, f64)] = &[(12.0, 0.5), (14.0, 2.0), (18.0, 12.0)];
    let service: &[(f64, f64)] = &[(14.0, 0.6), (18.0, 6.0)];
    let majors = ["trunk", "primary", "secondary", "tertiary"];
    let minors = ["minor", "residential", "unclassified", "living_street"];
    let style = json!({
        "version": 8,
        "name": "Nyx",
        "sources": {
            "openmaptiles": { "type": "vector", "tiles": [tiles], "minzoom": 0, "maxzoom": 14 }
        },
        "layers": [
            { "id": "background", "type": "background", "paint": { "background-color": c.land } },
            { "id": "landuse-residential", "type": "fill", "source": "openmaptiles", "source-layer": "landuse",
              "filter": ["match", ["get", "class"], ["residential", "suburb", "neighbourhood"], true, false],
              "paint": { "fill-color": c.residential } },
            { "id": "landcover-wood", "type": "fill", "source": "openmaptiles", "source-layer": "landcover",
              "filter": ["match", ["get", "class"], ["wood", "forest"], true, false],
              "paint": { "fill-color": c.wood } },
            { "id": "landcover-grass", "type": "fill", "source": "openmaptiles", "source-layer": "landcover",
              "filter": ["match", ["get", "class"], ["grass", "farmland", "wetland"], true, false],
              "paint": { "fill-color": c.park, "fill-opacity": 0.6 } },
            { "id": "park", "type": "fill", "source": "openmaptiles", "source-layer": "park",
              "paint": { "fill-color": c.park } },
            { "id": "water", "type": "fill", "source": "openmaptiles", "source-layer": "water",
              "paint": { "fill-color": c.water } },
            { "id": "waterway", "type": "line", "source": "openmaptiles", "source-layer": "waterway",
              "minzoom": 8,
              "paint": { "line-color": c.water, "line-width": widths(&[(8.0, 0.5), (14.0, 2.0), (18.0, 6.0)]) } },
            { "id": "building", "type": "fill", "source": "openmaptiles", "source-layer": "building",
              "minzoom": 14,
              "paint": { "fill-color": c.building } },
            { "id": "boundary-state", "type": "line", "source": "openmaptiles", "source-layer": "boundary",
              "filter": ["==", ["get", "admin_level"], 4],
              "minzoom": 3,
              "paint": { "line-color": c.boundary, "line-width": 0.8, "line-dasharray": [3, 2] } },
            { "id": "boundary-country", "type": "line", "source": "openmaptiles", "source-layer": "boundary",
              "filter": ["==", ["get", "admin_level"], 2],
              "paint": { "line-color": c.boundary, "line-width": widths(&[(2.0, 0.8), (10.0, 2.0)]) } },
            casing("road-service-casing", &["service", "track"], &c.casing, service, 14.0),
            casing("road-minor-casing", &minors, &c.casing, minor, 12.0),
            casing("road-major-casing", &majors, &c.casing, major, 9.0),
            casing("road-motorway-casing", &["motorway"], &c.casing, motorway, 8.0),
            { "id": "path", "type": "line", "source": "openmaptiles", "source-layer": "transportation",
              "minzoom": 14,
              "filter": ["match", ["get", "class"], ["path", "pedestrian"], true, false],
              "paint": { "line-color": c.road_minor, "line-width": 1.0, "line-dasharray": [2, 2] } },
            { "id": "rail", "type": "line", "source": "openmaptiles", "source-layer": "transportation",
              "minzoom": 10,
              "filter": ["==", ["get", "class"], "rail"],
              "paint": { "line-color": c.rail, "line-width": widths(&[(10.0, 0.6), (16.0, 2.0)]) } },
            road("road-service", &["service", "track"], &c.road_minor, service, 14.0),
            road("road-minor", &minors, &c.road_minor, minor, 12.0),
            road("road-major", &majors, &c.road_major, major, 7.0),
            road("road-motorway", &["motorway"], &c.road_motorway, motorway, 5.0),
            { "id": "water-name", "type": "symbol", "source": "openmaptiles", "source-layer": "water_name",
              "layout": { "text-field": name(), "text-size": 12 },
              "paint": { "text-color": c.label_dim, "text-halo-color": c.halo, "text-halo-width": 1.2 } },
            { "id": "road-name", "type": "symbol", "source": "openmaptiles", "source-layer": "transportation_name",
              "minzoom": 13,
              "layout": { "text-field": name(), "text-size": 11, "symbol-placement": "line" },
              "paint": { "text-color": c.label_dim, "text-halo-color": c.halo, "text-halo-width": 1.2 } },
            { "id": "place-minor", "type": "symbol", "source": "openmaptiles", "source-layer": "place",
              "minzoom": 11,
              "filter": ["match", ["get", "class"], ["village", "suburb", "neighbourhood", "hamlet"], true, false],
              "layout": { "text-field": name(), "text-size": 12 },
              "paint": { "text-color": c.label_dim, "text-halo-color": c.halo, "text-halo-width": 1.4 } },
            { "id": "place-town", "type": "symbol", "source": "openmaptiles", "source-layer": "place",
              "minzoom": 8,
              "filter": ["==", ["get", "class"], "town"],
              "layout": { "text-field": name(), "text-size": 13 },
              "paint": { "text-color": c.label, "text-halo-color": c.halo, "text-halo-width": 1.4 } },
            { "id": "place-city", "type": "symbol", "source": "openmaptiles", "source-layer": "place",
              "minzoom": 4,
              "filter": ["==", ["get", "class"], "city"],
              "layout": { "text-field": name(), "text-size": widths(&[(4.0, 12.0), (12.0, 18.0)]) },
              "paint": { "text-color": c.label, "text-halo-color": c.halo, "text-halo-width": 1.6 } },
            { "id": "place-state", "type": "symbol", "source": "openmaptiles", "source-layer": "place",
              "minzoom": 4, "maxzoom": 8,
              "filter": ["==", ["get", "class"], "state"],
              "layout": { "text-field": name(), "text-size": 12 },
              "paint": { "text-color": c.label_dim, "text-halo-color": c.halo, "text-halo-width": 1.4 } },
            { "id": "place-country", "type": "symbol", "source": "openmaptiles", "source-layer": "place",
              "maxzoom": 7,
              "filter": ["==", ["get", "class"], "country"],
              "layout": { "text-field": name(), "text-size": widths(&[(2.0, 12.0), (6.0, 16.0)]) },
              "paint": { "text-color": c.label, "text-halo-color": c.halo, "text-halo-width": 1.6 } }
        ]
    });
    style.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_halfway() {
        assert_eq!(mix("#000000", "#ffffff", 0.5), "#808080");
    }

    #[test]
    fn style_is_json_with_the_tile_url() {
        let c = |s: &str| s.to_string();
        let p = nyx_theme::Palette {
            bg: c("#101012"),
            surface: c("#18181b"),
            text: c("#e8e8ea"),
            dim_text: c("#9a9aa0"),
            accent: c("#5b9cff"),
            border: c("#2a2a2e"),
            muted: c("#202024"),
            highlight: c("#26262b"),
            danger: c("#ff5f5f"),
            glow: c("#5b9cff"),
            shadow: c("#000000"),
            light: false,
        };
        let s: Value = serde_json::from_str(&style_json("https://t/{z}/{x}/{y}.pbf", &p)).unwrap();
        assert_eq!(s["sources"]["openmaptiles"]["tiles"][0], "https://t/{z}/{x}/{y}.pbf");
        assert!(s["layers"].as_array().unwrap().len() > 20);
    }
}
