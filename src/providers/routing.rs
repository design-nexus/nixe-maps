use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TravelMode {
    Drive,
    Walk,
    Bicycle,
}

impl TravelMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "walk" | "walking" | "pedestrian" => TravelMode::Walk,
            "bicycle" | "bike" | "cycling" => TravelMode::Bicycle,
            _ => TravelMode::Drive,
        }
    }

    pub fn as_geoapify_mode(&self) -> &'static str {
        match self {
            TravelMode::Drive => "drive",
            TravelMode::Walk => "walk",
            TravelMode::Bicycle => "bicycle",
        }
    }

    pub fn as_osrm_profile(&self) -> &'static str {
        match self {
            TravelMode::Drive => "driving",
            TravelMode::Walk => "walking",
            TravelMode::Bicycle => "cycling",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            TravelMode::Drive => "Drive",
            TravelMode::Walk => "Walk",
            TravelMode::Bicycle => "Cycle",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    pub instruction: String,
    pub distance_meters: f64,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    pub mode: TravelMode,
    pub distance_meters: f64,
    pub duration_seconds: f64,
    pub coordinates: Vec<(f64, f64)>,
    pub steps: Vec<RouteStep>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyRoutingResponse {
    features: Option<Vec<GeoapifyRouteFeature>>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyRouteFeature {
    properties: Option<GeoapifyRouteProperties>,
    geometry: Option<GeoapifyRouteGeometry>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyRouteProperties {
    distance: Option<f64>,
    time: Option<f64>,
    legs: Option<Vec<GeoapifyRouteLeg>>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyRouteLeg {
    steps: Option<Vec<GeoapifyStep>>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyStep {
    distance: Option<f64>,
    time: Option<f64>,
    instruction: Option<GeoapifyInstruction>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyInstruction {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeoapifyRouteGeometry {
    coordinates: Option<Vec<Vec<f64>>>,
}

#[derive(Debug, Deserialize)]
struct OsrmResponse {
    routes: Option<Vec<OsrmRoute>>,
}

#[derive(Debug, Deserialize)]
struct OsrmRoute {
    distance: f64,
    duration: f64,
    geometry: Option<OsrmGeometry>,
    legs: Option<Vec<OsrmLeg>>,
}

#[derive(Debug, Deserialize)]
struct OsrmGeometry {
    coordinates: Vec<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
struct OsrmLeg {
    steps: Option<Vec<OsrmStep>>,
}

#[derive(Debug, Deserialize)]
struct OsrmStep {
    distance: f64,
    duration: f64,
    maneuver: Option<OsrmManeuver>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OsrmManeuver {
    #[serde(rename = "type")]
    maneuver_type: Option<String>,
    modifier: Option<String>,
}

pub async fn calculate_route(
    start: (f64, f64),
    end: (f64, f64),
    mode: TravelMode,
    api_key: &str,
) -> Result<RouteResult> {
    let client = reqwest::Client::builder()
        .user_agent("nixe-maps/1.0 (https://github.com/design-nexus/nixe-maps)")
        .build()?;

    // 1. Try Geoapify Routing API if key is present
    if !api_key.trim().is_empty() {
        let url = format!(
            "https://api.geoapify.com/v1/routing?waypoints={:.6},{:.6}|{:.6},{:.6}&mode={}&format=json&apiKey={}",
            start.0, start.1, end.0, end.1,
            mode.as_geoapify_mode(),
            api_key.trim()
        );

        let resp = client.get(&url).send().await?;
        if resp.status().is_success() {
            let data: GeoapifyRoutingResponse = resp.json().await?;
            if let Some(feature) = data.features.and_then(|f| f.into_iter().next()) {
                let props = feature.properties.unwrap_or(GeoapifyRouteProperties {
                    distance: Some(0.0),
                    time: Some(0.0),
                    legs: None,
                });

                let distance_meters = props.distance.unwrap_or(0.0);
                let duration_seconds = props.time.unwrap_or(0.0);

                let mut coordinates = Vec::new();
                if let Some(geom) = feature.geometry {
                    if let Some(coords) = geom.coordinates {
                        for pt in coords {
                            if pt.len() >= 2 {
                                coordinates.push((pt[1], pt[0]));
                            }
                        }
                    }
                }

                let mut steps = Vec::new();
                if let Some(legs) = props.legs {
                    for leg in legs {
                        if let Some(stps) = leg.steps {
                            for s in stps {
                                if let Some(instr) = s.instruction.and_then(|i| i.text) {
                                    steps.push(RouteStep {
                                        instruction: instr,
                                        distance_meters: s.distance.unwrap_or(0.0),
                                        duration_seconds: s.time.unwrap_or(0.0),
                                    });
                                }
                            }
                        }
                    }
                }

                return Ok(RouteResult {
                    mode,
                    distance_meters,
                    duration_seconds,
                    coordinates,
                    steps,
                });
            }
        } else if resp.status().as_u16() == 429 {
            bail!("Geoapify daily routing quota exceeded.");
        }
    }

    // 2. Fallback to OpenStreetMap OSRM routing
    let url = format!(
        "https://router.project-osrm.org/route/v1/{}/{:.6},{:.6};{:.6},{:.6}?overview=full&geometries=geojson&steps=true",
        mode.as_osrm_profile(),
        start.1, start.0, end.1, end.0
    );

    let resp = client.get(&url).send().await?;
    if resp.status().is_success() {
        let data: OsrmResponse = resp.json().await?;
        if let Some(route) = data.routes.and_then(|r| r.into_iter().next()) {
            let mut coordinates = Vec::new();
            if let Some(geom) = route.geometry {
                for pt in geom.coordinates {
                    if pt.len() >= 2 {
                        coordinates.push((pt[1], pt[0]));
                    }
                }
            }

            let mut steps = Vec::new();
            if let Some(legs) = route.legs {
                for leg in legs {
                    if let Some(stps) = leg.steps {
                        for s in stps {
                            let street = s.name.unwrap_or_default();
                            let instruction = match s.maneuver {
                                Some(m) => {
                                    let t = m.maneuver_type.unwrap_or_else(|| "turn".to_string());
                                    let modif = m.modifier.unwrap_or_default();
                                    if !modif.is_empty() && !street.is_empty() {
                                        format!("{} {} onto {}", t, modif, street)
                                    } else if !street.is_empty() {
                                        format!("{} onto {}", t, street)
                                    } else {
                                        format!("{} {}", t, modif)
                                    }
                                }
                                None => {
                                    if !street.is_empty() {
                                        format!("Proceed onto {}", street)
                                    } else {
                                        "Proceed along route".to_string()
                                    }
                                }
                            };
                            steps.push(RouteStep {
                                instruction,
                                distance_meters: s.distance,
                                duration_seconds: s.duration,
                            });
                        }
                    }
                }
            }

            return Ok(RouteResult {
                mode,
                distance_meters: route.distance,
                duration_seconds: route.duration,
                coordinates,
                steps,
            });
        }
    }

    bail!("Failed to calculate route between points.")
}

pub fn format_distance(meters: f64, units: &str) -> String {
    if units.eq_ignore_ascii_case("metric") {
        if meters < 1000.0 {
            format!("{:.0} m", meters)
        } else {
            format!("{:.1} km", meters / 1000.0)
        }
    } else {
        let miles = meters * 0.000621371;
        if miles < 0.1 {
            let feet = (meters * 3.28084).round();
            format!("{:.0} ft", feet)
        } else {
            format!("{:.1} mi", miles)
        }
    }
}

pub fn format_duration(seconds: f64) -> String {
    let total_minutes = (seconds / 60.0).round() as u64;
    if total_minutes < 60 {
        format!("{} min", total_minutes.max(1))
    } else {
        let hours = total_minutes / 60;
        let mins = total_minutes % 60;
        if mins == 0 {
            format!("{} hr", hours)
        } else {
            format!("{} hr {} min", hours, mins)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_distance() {
        assert_eq!(format_distance(50.0, "imperial"), "164 ft");
        assert_eq!(format_distance(1609.34, "imperial"), "1.0 mi");
        assert_eq!(format_distance(500.0, "metric"), "500 m");
        assert_eq!(format_distance(2500.0, "metric"), "2.5 km");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(45.0), "1 min");
        assert_eq!(format_duration(1200.0), "20 min");
        assert_eq!(format_duration(3600.0), "1 hr");
        assert_eq!(format_duration(5400.0), "1 hr 30 min");
    }
}
