use anyhow::{Result, bail};
use std::time::Duration;
use zbus::Connection;
use zbus::zvariant::OwnedObjectPath;

#[derive(Debug, Clone)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: f64,
}

pub async fn request_current_location() -> Result<GeoLocation> {
    let connection = Connection::system().await?;

    // 1. Get/Create Client from GeoClue2 Manager
    let reply = connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            "/org/freedesktop/GeoClue2/Manager",
            Some("org.freedesktop.GeoClue2.Manager"),
            "CreateClient",
            &(),
        )
        .await?;

    let client_path: OwnedObjectPath = reply.body().deserialize()?;

    // 2. Set DesktopId to identify our application
    let _ = connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            client_path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Set",
            &(
                "org.freedesktop.GeoClue2.Client",
                "DesktopId",
                zbus::zvariant::Value::from("io.github.design_nexus.nixe.Maps"),
            ),
        )
        .await;

    // Set requested accuracy level to exact (8) or city (4)
    let _ = connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            client_path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Set",
            &(
                "org.freedesktop.GeoClue2.Client",
                "DistanceThreshold",
                zbus::zvariant::Value::from(0u32),
            ),
        )
        .await;

    // 3. Start client
    connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            client_path.as_str(),
            Some("org.freedesktop.GeoClue2.Client"),
            "Start",
            &(),
        )
        .await?;

    // 4. Poll for Location property for up to 4 seconds
    let mut location_path: Option<OwnedObjectPath> = None;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(200)).await;

        let prop_reply = connection
            .call_method(
                Some("org.freedesktop.GeoClue2"),
                client_path.as_str(),
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &("org.freedesktop.GeoClue2.Client", "Location"),
            )
            .await;

        if let Ok(reply) = prop_reply {
            if let Ok(variant) = reply.body().deserialize::<zbus::zvariant::OwnedValue>() {
                if let Ok(path) = OwnedObjectPath::try_from(variant) {
                    if path.as_str() != "/" {
                        location_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    // Stop client to save power
    let _ = connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            client_path.as_str(),
            Some("org.freedesktop.GeoClue2.Client"),
            "Stop",
            &(),
        )
        .await;

    let Some(loc_path) = location_path else {
        bail!("Location unavailable or timed out");
    };

    // 5. Read coordinates from Location object
    let lat_val = connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            loc_path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.GeoClue2.Location", "Latitude"),
        )
        .await?;
    let lat_v: zbus::zvariant::OwnedValue = lat_val.body().deserialize()?;
    let latitude: f64 = lat_v.try_into()?;

    let lon_val = connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            loc_path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.GeoClue2.Location", "Longitude"),
        )
        .await?;
    let lon_v: zbus::zvariant::OwnedValue = lon_val.body().deserialize()?;
    let longitude: f64 = lon_v.try_into()?;

    let acc_val = connection
        .call_method(
            Some("org.freedesktop.GeoClue2"),
            loc_path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.GeoClue2.Location", "Accuracy"),
        )
        .await?;
    let acc_v: zbus::zvariant::OwnedValue = acc_val.body().deserialize()?;
    let accuracy: f64 = acc_v.try_into().unwrap_or(50.0);

    Ok(GeoLocation {
        latitude,
        longitude,
        accuracy,
    })
}
