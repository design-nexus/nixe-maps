use libshumate::prelude::*;
use libshumate::{MapProjection, MapSource, RasterRenderer, VectorRenderer};

use super::vector_style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapStyle {
    Standard,
    Topographic,
    Satellite,
}

impl MapStyle {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "topo" | "topographic" => MapStyle::Topographic,
            "sat" | "satellite" => MapStyle::Satellite,
            _ => MapStyle::Standard,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MapStyle::Standard => "standard",
            MapStyle::Topographic => "topographic",
            MapStyle::Satellite => "satellite",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            MapStyle::Standard => "Standard",
            MapStyle::Topographic => "Topographic",
            MapStyle::Satellite => "Satellite",
        }
    }

    pub fn attribution(&self) -> &'static str {
        match self {
            MapStyle::Standard if vector_tiles().is_some() => vector_style::ATTRIBUTION,
            MapStyle::Standard => "© OpenStreetMap contributors",
            MapStyle::Topographic => "Map: © OpenTopoMap (CC-BY-SA), Data: © OpenStreetMap, SRTM",
            MapStyle::Satellite => "Tiles © Esri — Source: Esri, Maxar, Earthstar Geographics",
        }
    }
}

/// The size imagery tiles are drawn at. On a screen scaled 1.5× or more, tiles from one
/// zoom level deeper are drawn at half size, so each has about twice the detail rather
/// than being stretched. Not for tiles with baked-in labels: those would be too small.
fn imagery_tile_size() -> u32 {
    let scale = gtk::gdk::Display::default()
        .map(|d| {
            let monitors = d.monitors();
            (0..monitors.n_items())
                .filter_map(|i| monitors.item(i).and_downcast::<gtk::gdk::Monitor>())
                .map(|m| m.scale())
                .fold(1.0_f64, f64::max)
        })
        .unwrap_or(1.0);
    if scale >= 1.5 { 128 } else { 256 }
}

fn raster(id: &str, name: &str, license: &str, uri: &str, max_zoom: u32, tile_size: u32, url: &str) -> MapSource {
    RasterRenderer::new_full_from_url(id, name, license, uri, 0, max_zoom, tile_size, MapProjection::Mercator, url)
        .upcast::<MapSource>()
}

/// The vector Standard map, when libshumate can draw vector tiles and the tile URL is
/// known (see `refresh_vector_tiles`).
fn vector(tiles: &str) -> Option<MapSource> {
    if !VectorRenderer::is_supported() {
        return None;
    }
    let style = vector_style::style_json(tiles, &nyx_theme::palette());
    match VectorRenderer::new("nyx-standard", &style) {
        Ok(r) => {
            r.set_license(vector_style::ATTRIBUTION);
            r.set_license_uri("https://openfreemap.org");
            r.set_max_zoom_level(20);
            Some(r.upcast::<MapSource>())
        }
        Err(e) => {
            eprintln!("nyx-maps: vector style: {e}");
            None
        }
    }
}

pub fn create_renderer(style: MapStyle, custom_satellite_url: &str, custom_topo_url: &str) -> MapSource {
    match style {
        MapStyle::Standard => vector_tiles().and_then(|t| vector(&t)).unwrap_or_else(|| {
            raster(
                "osm-standard",
                "OpenStreetMap",
                "© OpenStreetMap contributors",
                "https://www.openstreetmap.org/copyright",
                19,
                256,
                "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            )
        }),
        MapStyle::Topographic => {
            let url = custom_topo_url.trim();
            let url = if url.is_empty() { "https://tile.opentopomap.org/{z}/{x}/{y}.png" } else { url };
            raster(
                "opentopomap",
                "OpenTopoMap",
                "Map: © OpenTopoMap (CC-BY-SA), Data: © OpenStreetMap",
                "https://opentopomap.org/about",
                17,
                256,
                url,
            )
        }
        MapStyle::Satellite => {
            let url = custom_satellite_url.trim();
            let url = if url.is_empty() {
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
            } else {
                url
            };
            raster(
                "satellite-imagery",
                "Satellite Imagery",
                "Tiles © Esri — Source: Esri, Maxar, Earthstar Geographics",
                "https://www.esri.com",
                19,
                imagery_tile_size(),
                url,
            )
        }
    }
}

fn tiles_cache() -> std::path::PathBuf {
    nyx_core::paths::cache_home().join("nyx-maps/openfreemap-tiles")
}

/// The vector tile URL last read from OpenFreeMap's TileJSON, if any.
pub fn vector_tiles() -> Option<String> {
    let url = std::fs::read_to_string(tiles_cache()).ok()?;
    let url = url.trim();
    (url.starts_with("https://") && url.contains("{z}")).then(|| url.to_string())
}

/// Reads the current tile URL from OpenFreeMap's TileJSON and caches it. Returns it
/// when it differs from the cached one (the map should be built again).
pub async fn refresh_vector_tiles() -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("nyx-maps/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let json: serde_json::Value = client.get(vector_style::TILEJSON_URL).send().await.ok()?.json().await.ok()?;
    let url = json["tiles"].get(0)?.as_str()?.to_string();
    if !url.starts_with("https://") || !url.contains("{z}") {
        return None;
    }
    if vector_tiles().as_deref() == Some(url.as_str()) {
        return None;
    }
    let path = tiles_cache();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&path, &url).ok()?;
    Some(url)
}
