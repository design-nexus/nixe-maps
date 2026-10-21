use libshumate::prelude::*;
use libshumate::{MapProjection, MapSource, RasterRenderer};

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
            MapStyle::Standard => "© OpenStreetMap contributors",
            MapStyle::Topographic => "Map: © OpenTopoMap (CC-BY-SA), Data: © OpenStreetMap, SRTM",
            MapStyle::Satellite => "Tiles © Esri — Source: Esri, Maxar, Earthstar Geographics",
        }
    }
}

pub fn create_renderer(
    style: MapStyle,
    custom_satellite_url: &str,
    custom_topo_url: &str,
) -> MapSource {
    match style {
        MapStyle::Standard => {
            let renderer = RasterRenderer::new_full_from_url(
                "osm-standard",
                "OpenStreetMap",
                "© OpenStreetMap contributors",
                "https://www.openstreetmap.org/copyright",
                0,
                19,
                256,
                MapProjection::Mercator,
                "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            );
            renderer.upcast::<MapSource>()
        }
        MapStyle::Topographic => {
            let url = if !custom_topo_url.trim().is_empty() {
                custom_topo_url.trim()
            } else {
                "https://tile.opentopomap.org/{z}/{x}/{y}.png"
            };
            let renderer = RasterRenderer::new_full_from_url(
                "opentopomap",
                "OpenTopoMap",
                "Map: © OpenTopoMap (CC-BY-SA), Data: © OpenStreetMap",
                "https://opentopomap.org/about",
                0,
                17,
                256,
                MapProjection::Mercator,
                url,
            );
            renderer.upcast::<MapSource>()
        }
        MapStyle::Satellite => {
            let url = if !custom_satellite_url.trim().is_empty() {
                custom_satellite_url.trim()
            } else {
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"
            };
            let renderer = RasterRenderer::new_full_from_url(
                "satellite-imagery",
                "Satellite Imagery",
                "Tiles © Esri — Source: Esri, Maxar, Earthstar Geographics",
                "https://www.esri.com",
                0,
                19,
                256,
                MapProjection::Mercator,
                url,
            );
            renderer.upcast::<MapSource>()
        }
    }
}
