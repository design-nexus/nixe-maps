use crate::providers::{TravelMode, calculate_route};
use crate::settings;
use std::sync::mpsc::Sender;
use zbus::interface;

#[derive(Debug, Clone)]
pub enum MapCommand {
    ShowLocation { lat: f64, lon: f64, label: String },
    Search { query: String },
    ShowRoute { start: (f64, f64), end: (f64, f64), mode: String },
}

pub struct MapsDbusService {
    sender: Sender<MapCommand>,
}

impl MapsDbusService {
    pub fn new(sender: Sender<MapCommand>) -> Self {
        Self { sender }
    }
}

#[interface(name = "io.github.design_nexus.nixe.Maps")]
impl MapsDbusService {
    async fn show_location(&self, lat: f64, lon: f64, label: String) {
        let _ = self.sender.send(MapCommand::ShowLocation { lat, lon, label });
    }

    async fn search(&self, query: String) {
        let _ = self.sender.send(MapCommand::Search { query });
    }

    async fn show_route(&self, start_lat: f64, start_lon: f64, end_lat: f64, end_lon: f64, mode: String) {
        let _ = self.sender.send(MapCommand::ShowRoute {
            start: (start_lat, start_lon),
            end: (end_lat, end_lon),
            mode,
        });
    }

    async fn get_distance(
        &self,
        start_lat: f64,
        start_lon: f64,
        end_lat: f64,
        end_lon: f64,
        mode: String,
    ) -> (f64, f64) {
        let t_mode = TravelMode::from_str(&mode);
        let api_key = settings::get().geoapify_api_key;
        if let Ok(route) = calculate_route((start_lat, start_lon), (end_lat, end_lon), t_mode, &api_key).await {
            (route.distance_meters, route.duration_seconds)
        } else {
            (0.0, 0.0)
        }
    }
}

pub async fn start_dbus_server(sender: Sender<MapCommand>) -> anyhow::Result<()> {
    let service = MapsDbusService::new(sender);
    let _conn = zbus::connection::Builder::session()?
        .name("io.github.design_nexus.nixe.Maps")?
        .serve_at("/io/github/design_nexus/nixe/Maps", service)?
        .build()
        .await?;
    
    // Keep connection alive in task
    std::future::pending::<()>().await;
    Ok(())
}
