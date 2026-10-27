pub mod cli_api;

pub use cli_api::handle_cli_api;

#[derive(Debug, PartialEq)]
pub enum DeepLink {
    Location { lat: f64, lon: f64, label: String },
    Route { start: (f64, f64), end: (f64, f64), mode: String },
    Search { query: String },
}

pub fn parse_deep_link(uri_or_arg: &str) -> Option<DeepLink> {
    let s = uri_or_arg.trim();

    // 1. geo: URI scheme RFC 5870
    // Example: geo:36.1627,-86.7816 or geo:36.1627,-86.7816?q=Nashville
    if let Some(rest) = s.strip_prefix("geo:") {
        let (coords_part, query_part) = if let Some(idx) = rest.find('?') {
            (&rest[..idx], Some(&rest[idx + 1..]))
        } else {
            (rest, None)
        };

        if let Some(q) = query_part {
            if let Some(query_str) = q.strip_prefix("q=") {
                let decoded = query_str.replace('+', " ");
                return Some(DeepLink::Search { query: decoded });
            }
        }

        if let Some((lat, lon)) = crate::providers::parse_coordinates(coords_part) {
            return Some(DeepLink::Location {
                lat,
                lon,
                label: format!("{:.4}, {:.4}", lat, lon),
            });
        }
    }

    // 2. nyx-maps://show?lat=36.1627&lon=-86.7816&label=Nashville
    if let Some(rest) = s.strip_prefix("nyx-maps://show?") {
        let mut lat = None;
        let mut lon = None;
        let mut label = "Selected Location".to_string();

        for param in rest.split('&') {
            if let Some(v) = param.strip_prefix("lat=") {
                lat = v.parse::<f64>().ok();
            } else if let Some(v) = param.strip_prefix("lon=") {
                lon = v.parse::<f64>().ok();
            } else if let Some(v) = param.strip_prefix("label=") {
                label = v.replace('+', " ");
            }
        }

        if let (Some(la), Some(lo)) = (lat, lon) {
            return Some(DeepLink::Location {
                lat: la,
                lon: lo,
                label,
            });
        }
    }

    // 3. nyx-maps://route?start=36.1627,-86.7816&end=35.1495,-90.0490&mode=drive
    // Also supports start_lat/start_lon & end_lat/end_lon
    if let Some(rest) = s.strip_prefix("nyx-maps://route?") {
        let mut start = None;
        let mut end = None;
        let mut start_lat = None;
        let mut start_lon = None;
        let mut end_lat = None;
        let mut end_lon = None;
        let mut mode = "drive".to_string();

        for param in rest.split('&') {
            if let Some(v) = param.strip_prefix("start=") {
                start = crate::providers::parse_coordinates(v);
            } else if let Some(v) = param.strip_prefix("end=") {
                end = crate::providers::parse_coordinates(v);
            } else if let Some(v) = param.strip_prefix("start_lat=") {
                start_lat = v.parse::<f64>().ok();
            } else if let Some(v) = param.strip_prefix("start_lon=") {
                start_lon = v.parse::<f64>().ok();
            } else if let Some(v) = param.strip_prefix("end_lat=") {
                end_lat = v.parse::<f64>().ok();
            } else if let Some(v) = param.strip_prefix("end_lon=") {
                end_lon = v.parse::<f64>().ok();
            } else if let Some(v) = param.strip_prefix("mode=") {
                mode = v.to_string();
            }
        }

        if let (Some(sla), Some(slo)) = (start_lat, start_lon) {
            start = Some((sla, slo));
        }
        if let (Some(ela), Some(elo)) = (end_lat, end_lon) {
            end = Some((ela, elo));
        }

        if let (Some(st), Some(en)) = (start, end) {
            return Some(DeepLink::Route {
                start: st,
                end: en,
                mode,
            });
        }
    }

    // 4. Direct coordinates argument
    if let Some((lat, lon)) = crate::providers::parse_coordinates(s) {
        return Some(DeepLink::Location {
            lat,
            lon,
            label: format!("{:.4}, {:.4}", lat, lon),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_deep_link_geo() {
        assert_eq!(
            parse_deep_link("geo:36.1627,-86.7816"),
            Some(DeepLink::Location {
                lat: 36.1627,
                lon: -86.7816,
                label: "36.1627, -86.7816".to_string(),
            })
        );

        assert_eq!(
            parse_deep_link("geo:0,0?q=Nashville+Coffee"),
            Some(DeepLink::Search {
                query: "Nashville Coffee".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_deep_link_nyx_maps() {
        assert_eq!(
            parse_deep_link("nyx-maps://show?lat=35.1495&lon=-90.0490&label=Memphis"),
            Some(DeepLink::Location {
                lat: 35.1495,
                lon: -90.0490,
                label: "Memphis".to_string(),
            })
        );

        assert_eq!(
            parse_deep_link("nyx-maps://route?start=36.1627,-86.7816&end=35.1495,-90.0490&mode=drive"),
            Some(DeepLink::Route {
                start: (36.1627, -86.7816),
                end: (35.1495, -90.0490),
                mode: "drive".to_string(),
            })
        );
    }
}
