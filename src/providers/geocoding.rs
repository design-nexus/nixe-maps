use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub formatted_address: String,
    pub lat: f64,
    pub lon: f64,
    pub category: Option<String>,
}

pub fn parse_coordinates(input: &str) -> Option<(f64, f64)> {
    let trimmed = input.trim();
    let parts: Vec<&str> = if trimmed.contains(',') {
        trimmed.split(',').collect()
    } else if trimmed.contains(' ') {
        trimmed.split_whitespace().collect()
    } else {
        return None;
    };

    if parts.len() != 2 {
        return None;
    }

    let lat = parts[0].trim().parse::<f64>().ok()?;
    let lon = parts[1].trim().parse::<f64>().ok()?;

    if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
        Some((lat, lon))
    } else {
        None
    }
}

// Geoapify Geocoding API response format
#[derive(Debug, Deserialize)]
struct GeoapifyResponse {
    results: Option<Vec<GeoapifyPlace>>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyPlace {
    formatted: Option<String>,
    name: Option<String>,
    address_line1: Option<String>,
    address_line2: Option<String>,
    lat: f64,
    lon: f64,
    category: Option<String>,
    result_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyPlacesFeatureCollection {
    features: Option<Vec<GeoapifyPlacesFeature>>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyPlacesFeature {
    properties: Option<GeoapifyPlaceProperties>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyPlaceProperties {
    name: Option<String>,
    formatted: Option<String>,
    address_line1: Option<String>,
    address_line2: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
    category: Option<String>,
}

// Nominatim Geocoding API response format (fallback)
#[derive(Debug, Deserialize)]
struct NominatimPlace {
    display_name: String,
    name: Option<String>,
    lat: String,
    lon: String,
    #[serde(rename = "type")]
    place_type: Option<String>,
}

pub async fn search_places(query: &str, api_key: &str) -> Result<Vec<SearchResult>> {
    search_places_near(query, api_key, None).await
}

/// Great-circle distance in metres.
pub fn distance_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    6_371_000.0 * 2.0 * h.sqrt().asin()
}

#[derive(Debug, Deserialize)]
struct PhotonCollection {
    features: Vec<PhotonFeature>,
}

#[derive(Debug, Deserialize)]
struct PhotonFeature {
    geometry: PhotonGeometry,
    properties: PhotonProperties,
}

#[derive(Debug, Deserialize)]
struct PhotonGeometry {
    coordinates: (f64, f64),
}

#[derive(Debug, Deserialize)]
struct PhotonProperties {
    name: Option<String>,
    housenumber: Option<String>,
    street: Option<String>,
    city: Option<String>,
    state: Option<String>,
    postcode: Option<String>,
    country: Option<String>,
    osm_key: Option<String>,
    osm_value: Option<String>,
}

/// Photon (komoot's OpenStreetMap geocoder): made for search as you type, and it
/// favours places near `near`. Stops and parking lots named after a place are dropped,
/// so "walmart" finds the store, not its bus stop.
async fn photon(client: &reqwest::Client, query: &str, near: Option<(f64, f64)>) -> Result<Vec<SearchResult>> {
    let mut url = format!("https://photon.komoot.io/api/?q={}&limit=12&lang=en", urlencoding::encode(query));
    if let Some((lat, lon)) = near {
        url.push_str(&format!("&lat={lat:.5}&lon={lon:.5}"));
    }
    let data: PhotonCollection = client.get(&url).send().await?.error_for_status()?.json().await?;
    let mut results: Vec<SearchResult> = data
        .features
        .into_iter()
        .filter(|f| {
            let key = f.properties.osm_key.as_deref().unwrap_or("");
            let value = f.properties.osm_value.as_deref().unwrap_or("");
            !(key == "highway" || key == "public_transport" || (key == "amenity" && value == "parking"))
        })
        .map(|f| {
            let p = f.properties;
            let street = match (&p.housenumber, &p.street) {
                (Some(n), Some(s)) => Some(format!("{n} {s}")),
                (None, Some(s)) => Some(s.clone()),
                _ => None,
            };
            let name = p.name.clone().or_else(|| street.clone()).unwrap_or_else(|| "Place".into());
            let place = [street, p.city, p.state, p.postcode, p.country]
                .into_iter()
                .flatten()
                .filter(|part| *part != name)
                .collect::<Vec<_>>()
                .join(", ");
            SearchResult {
                name,
                formatted_address: place,
                lat: f.geometry.coordinates.1,
                lon: f.geometry.coordinates.0,
                category: p.osm_value,
            }
        })
        .collect();
    if let Some(here) = near {
        results.sort_by(|a, b| distance_m(here, (a.lat, a.lon)).total_cmp(&distance_m(here, (b.lat, b.lon))));
    }
    results.truncate(8);
    Ok(results)
}

/// Places matching `query`, nearest to `near` first when it's given.
pub async fn search_places_near(query: &str, api_key: &str, near: Option<(f64, f64)>) -> Result<Vec<SearchResult>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    // 1. Direct coordinate match
    if let Some((lat, lon)) = parse_coordinates(query) {
        return Ok(vec![SearchResult {
            name: format!("{:.5}, {:.5}", lat, lon),
            formatted_address: format!("Latitude: {:.5}, Longitude: {:.5}", lat, lon),
            lat,
            lon,
            category: Some("Coordinate".to_string()),
        }]);
    }

    let client = reqwest::Client::builder()
        .user_agent("nyx-maps/1.0 (https://github.com/design-nexus/nyx-maps)")
        .build()?;

    // 2. Geoapify API if key configured
    if !api_key.trim().is_empty() {
        let mut url = format!(
            "https://api.geoapify.com/v1/geocode/search?text={}&format=json&apiKey={}&limit=8",
            urlencoding::encode(query),
            api_key.trim()
        );
        if let Some((lat, lon)) = near {
            url.push_str(&format!("&bias=proximity:{lon:.5},{lat:.5}"));
        }

        let resp = client.get(&url).send().await?;
        if resp.status().is_success() {
            let data: GeoapifyResponse = resp.json().await?;
            if let Some(places) = data.results {
                let results = places
                    .into_iter()
                    .map(|p| {
                        let name = p.name
                            .or(p.address_line1)
                            .unwrap_or_else(|| "Place".to_string());
                        let formatted = p.formatted
                            .or(p.address_line2)
                            .unwrap_or_else(|| name.clone());
                        SearchResult {
                            name,
                            formatted_address: formatted,
                            lat: p.lat,
                            lon: p.lon,
                            category: p.category.or(p.result_type),
                        }
                    })
                    .collect();
                return Ok(results);
            }
            return Ok(Vec::new());
        } else if resp.status().as_u16() == 429 {
            bail!("Geoapify daily quota or rate limit exceeded.");
        } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
            bail!("Invalid or expired Geoapify API key.");
        }
    }

    // 3. Photon: no key needed, near-first.
    if let Ok(results) = photon(&client, query, near).await
        && !results.is_empty()
    {
        return Ok(results);
    }

    // 4. Nominatim, if Photon is unreachable.
    let url = format!(
        "https://nominatim.openstreetmap.org/search?format=json&q={}&limit=5",
        urlencoding::encode(query)
    );

    let resp = client.get(&url).send().await?;
    if resp.status().is_success() {
        let places: Vec<NominatimPlace> = resp.json().await?;
        let results = places
            .into_iter()
            .filter_map(|p| {
                let lat = p.lat.parse::<f64>().ok()?;
                let lon = p.lon.parse::<f64>().ok()?;
                let name = p.name
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| {
                        p.display_name.split(',').next().unwrap_or("Place").trim().to_string()
                    });
                Some(SearchResult {
                    name,
                    formatted_address: p.display_name,
                    lat,
                    lon,
                    category: p.place_type,
                })
            })
            .collect();
        return Ok(results);
    }

    Ok(Vec::new())
}

pub async fn search_nearby(
    category: &str,
    lat: f64,
    lon: f64,
    radius_km: f64,
    api_key: &str,
) -> Result<Vec<SearchResult>> {
    let client = reqwest::Client::builder()
        .user_agent("nyx-maps/1.0 (https://github.com/design-nexus/nyx-maps)")
        .build()?;

    // 1. Geoapify Places API v2 if API key is provided
    if !api_key.trim().is_empty() {
        let geoapify_cat = match category {
            "food" => "catering.restaurant,catering.fast_food",
            "groceries" => "commercial.supermarket,commercial.food_and_drink",
            "fuel" => "service.vehicle.fuel",
            "entertainment" => "entertainment.culture,entertainment.cinema",
            "parks" => "leisure.park",
            other => other,
        };

        let radius_meters = (radius_km * 1000.0) as u32;
        let url = format!(
            "https://api.geoapify.com/v2/places?categories={}&filter=circle:{},{},{}&bias=proximity:{},{}&limit=10&apiKey={}",
            geoapify_cat,
            lon,
            lat,
            radius_meters,
            lon,
            lat,
            api_key.trim()
        );

        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                if let Ok(data) = resp.json::<GeoapifyPlacesFeatureCollection>().await {
                    if let Some(features) = data.features {
                        let results: Vec<SearchResult> = features
                            .into_iter()
                            .filter_map(|f| {
                                let p = f.properties?;
                                let p_lat = p.lat?;
                                let p_lon = p.lon?;
                                let name = p.name
                                    .or(p.address_line1)
                                    .unwrap_or_else(|| "Place".to_string());
                                let formatted = p.formatted
                                    .or(p.address_line2)
                                    .unwrap_or_else(|| name.clone());
                                Some(SearchResult {
                                    name,
                                    formatted_address: formatted,
                                    lat: p_lat,
                                    lon: p_lon,
                                    category: p.category.or_else(|| Some(category.to_string())),
                                })
                            })
                            .collect();
                        if !results.is_empty() {
                            return Ok(results);
                        }
                    }
                }
            }
        }
    }

    // 2. Nominatim Bounded Search Fallback
    let (query, display_cat) = match category {
        "food" => ("restaurant", "Restaurant"),
        "groceries" => ("supermarket", "Groceries"),
        "fuel" => ("fuel", "Gas Station"),
        "entertainment" => ("cinema", "Entertainment"),
        "parks" => ("park", "Park"),
        other => (other, other),
    };

    let lat_delta = radius_km / 111.0;
    let cos_lat = lat.to_radians().cos().abs().max(0.01);
    let lon_delta = radius_km / (111.0 * cos_lat);

    let min_lat = (lat - lat_delta).max(-90.0);
    let max_lat = (lat + lat_delta).min(90.0);
    let min_lon = (lon - lon_delta).max(-180.0);
    let max_lon = (lon + lon_delta).min(180.0);

    let url = format!(
        "https://nominatim.openstreetmap.org/search?format=json&q={}&viewbox={:.6},{:.6},{:.6},{:.6}&bounded=1&limit=10",
        urlencoding::encode(query),
        min_lon,
        max_lat,
        max_lon,
        min_lat
    );

    let resp = client.get(&url).send().await?;
    if resp.status().is_success() {
        let places: Vec<NominatimPlace> = resp.json().await?;
        let results = places
            .into_iter()
            .filter_map(|p| {
                let p_lat = p.lat.parse::<f64>().ok()?;
                let p_lon = p.lon.parse::<f64>().ok()?;
                let name = p.name
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| {
                        p.display_name.split(',').next().unwrap_or("Place").trim().to_string()
                    });
                Some(SearchResult {
                    name,
                    formatted_address: p.display_name,
                    lat: p_lat,
                    lon: p_lon,
                    category: p.place_type.or_else(|| Some(display_cat.to_string())),
                })
            })
            .collect();
        return Ok(results);
    }

    Ok(Vec::new())
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut encoded = String::new();
        for b in s.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(b as char);
                }
                _ => {
                    encoded.push_str(&format!("%{:02X}", b));
                }
            }
        }
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coordinates() {
        assert_eq!(parse_coordinates("36.1627, -86.7816"), Some((36.1627, -86.7816)));
        assert_eq!(parse_coordinates("35.1495,-90.0490"), Some((35.1495, -90.0490)));
        assert_eq!(parse_coordinates("   40.7128   -74.0060  "), Some((40.7128, -74.0060)));
        assert_eq!(parse_coordinates("100.0, 50.0"), None); // Invalid lat
        assert_eq!(parse_coordinates("Nashville"), None);
    }

    #[test]
    fn test_search_nearby_bounding_box() {
        let lat: f64 = 33.1207;
        let lon: f64 = -86.7601;
        let radius_km: f64 = 8.0;

        let lat_delta: f64 = radius_km / 111.0;
        let cos_lat: f64 = lat.to_radians().cos().abs().max(0.01);
        let lon_delta: f64 = radius_km / (111.0 * cos_lat);

        let min_lat: f64 = (lat - lat_delta).max(-90.0);
        let max_lat: f64 = (lat + lat_delta).min(90.0);
        let min_lon: f64 = (lon - lon_delta).max(-180.0);
        let max_lon: f64 = (lon + lon_delta).min(180.0);

        assert!(min_lat < lat && lat < max_lat);
        assert!(min_lon < lon && lon < max_lon);
        assert!((max_lat - min_lat) > 0.1);
        assert!((max_lon - min_lon) > 0.1);
    }
}
