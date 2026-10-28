use crate::providers::{MapStyle, create_renderer};
use gtk::prelude::*;
use libshumate::prelude::*;
use libshumate::{Coordinate, Marker, MarkerLayer, PathLayer, SimpleMap};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct MapView {
    pub root: SimpleMap,
    marker_layer: MarkerLayer,
    route_layer: PathLayer,
    selected_marker: Rc<RefCell<Option<Marker>>>,
    user_marker: Rc<RefCell<Option<Marker>>>,
    route_start_marker: Rc<RefCell<Option<Marker>>>,
    route_end_marker: Rc<RefCell<Option<Marker>>>,
    current_style: Rc<RefCell<MapStyle>>,
    /// Shumate zoom minus the app's zoom (256 px tiles): 1 for half-size high-DPI tiles.
    zoom_offset: Rc<std::cell::Cell<f64>>,
}

impl MapView {
    pub fn new(initial_lat: f64, initial_lon: f64, initial_zoom: f64, style: MapStyle) -> Self {
        let root = SimpleMap::new();
        root.set_show_zoom_buttons(false);
        if let Some(lic) = root.license() {
            lic.set_visible(false);
        }
        root.set_hexpand(true);
        root.set_vexpand(true);

        let viewport = root.viewport().expect("SimpleMap has viewport");
        viewport.set_location(initial_lat, initial_lon);

        let marker_layer = MarkerLayer::new(&viewport);
        let route_layer = PathLayer::new(&viewport);
        route_layer.set_stroke(true);
        route_layer.set_closed(false);

        // The route in the app's accent.
        let route_color = gdk::RGBA::parse(nyx_theme::app_accent("maps").as_str())
            .unwrap_or_else(|_| gdk::RGBA::new(0.35, 0.60, 0.98, 1.0));
        route_layer.set_stroke_color(Some(&gdk::RGBA::new(route_color.red(), route_color.green(), route_color.blue(), 0.95)));
        route_layer.set_stroke_width(6.0);

        root.add_overlay_layer(&route_layer);
        root.add_overlay_layer(&marker_layer);

        let view = Self {
            root,
            marker_layer,
            route_layer,
            selected_marker: Rc::new(RefCell::new(None)),
            user_marker: Rc::new(RefCell::new(None)),
            route_start_marker: Rc::new(RefCell::new(None)),
            route_end_marker: Rc::new(RefCell::new(None)),
            current_style: Rc::new(RefCell::new(style)),
            zoom_offset: Rc::new(std::cell::Cell::new(0.0)),
        };

        view.set_style(style, "", "");
        if let Some(vp) = view.root.viewport() {
            vp.set_zoom_level(initial_zoom + view.zoom_offset.get());
        }
        view
    }

    /// Show `style`, keeping the place and scale on screen: sources with smaller tiles
    /// count zoom levels from a different start.
    pub fn set_style(&self, style: MapStyle, custom_sat: &str, custom_topo: &str) {
        *self.current_style.borrow_mut() = style;
        let renderer = create_renderer(style, custom_sat, custom_topo);
        let zoom = self.root.viewport().map(|vp| vp.zoom_level() - self.zoom_offset.get());
        let offset = (256.0 / renderer.tile_size().max(1) as f64).log2();
        self.zoom_offset.set(offset);
        self.root.set_map_source(Some(&renderer));
        if let (Some(vp), Some(z)) = (self.root.viewport(), zoom) {
            vp.set_zoom_level(z + offset);
        }
    }

    pub fn current_style(&self) -> MapStyle {
        *self.current_style.borrow()
    }

    pub fn center_on(&self, lat: f64, lon: f64, zoom: Option<f64>) {
        if let Some(viewport) = self.root.viewport() {
            viewport.set_location(lat, lon);
            if let Some(z) = zoom {
                viewport.set_zoom_level(z + self.zoom_offset.get());
            }
        }
    }

    pub fn set_selected_marker(&self, lat: f64, lon: f64, _title: &str) {
        self.clear_selected_marker();

        let marker = Marker::new();
        marker.set_location(lat, lon);

        // Custom pin icon
        let pin = gtk::Image::from_icon_name("mark-location-symbolic");
        pin.set_pixel_size(24);
        pin.add_css_class("map-pin-accent");
        marker.set_child(Some(&pin));

        self.marker_layer.add_marker(&marker);
        *self.selected_marker.borrow_mut() = Some(marker);
    }

    pub fn clear_selected_marker(&self) {
        if let Some(m) = self.selected_marker.borrow_mut().take() {
            self.marker_layer.remove_marker(&m);
        }
    }

    pub fn set_user_location(&self, lat: f64, lon: f64) {
        if let Some(m) = self.user_marker.borrow_mut().take() {
            self.marker_layer.remove_marker(&m);
        }

        let marker = Marker::new();
        marker.set_location(lat, lon);

        let icon = gtk::Image::from_icon_name("find-location-symbolic");
        icon.set_pixel_size(22);
        icon.add_css_class("map-pin-user");
        marker.set_child(Some(&icon));

        self.marker_layer.add_marker(&marker);
        *self.user_marker.borrow_mut() = Some(marker);
    }

    pub fn fit_coordinates(&self, coordinates: &[(f64, f64)]) {
        if coordinates.is_empty() {
            return;
        }
        let mut min_lat = f64::MAX;
        let mut max_lat = f64::MIN;
        let mut min_lon = f64::MAX;
        let mut max_lon = f64::MIN;

        for &(lat, lon) in coordinates {
            if lat < min_lat { min_lat = lat; }
            if lat > max_lat { max_lat = lat; }
            if lon < min_lon { min_lon = lon; }
            if lon > max_lon { max_lon = lon; }
        }

        let center_lat = (min_lat + max_lat) / 2.0;
        let center_lon = (min_lon + max_lon) / 2.0;

        let lat_diff = (max_lat - min_lat).abs();
        let lon_diff = (max_lon - min_lon).abs();
        let max_diff = lat_diff.max(lon_diff);

        let zoom = if max_diff < 0.005 {
            16.0
        } else if max_diff < 0.015 {
            15.0
        } else if max_diff < 0.035 {
            14.0
        } else if max_diff < 0.08 {
            13.0
        } else if max_diff < 0.18 {
            12.0
        } else if max_diff < 0.4 {
            11.0
        } else if max_diff < 0.9 {
            10.0
        } else if max_diff < 2.0 {
            9.0
        } else if max_diff < 4.0 {
            8.0
        } else if max_diff < 8.0 {
            7.0
        } else {
            6.0
        };

        self.center_on(center_lat, center_lon, Some(zoom));
    }

    pub fn show_route(&self, coordinates: &[(f64, f64)]) {
        self.clear_route();

        if coordinates.is_empty() {
            return;
        }

        for (lat, lon) in coordinates {
            let coord = Coordinate::new_full(*lat, *lon);
            self.route_layer.add_node(&coord);
        }

        // Start marker (origin)
        if let Some(first) = coordinates.first() {
            let marker = Marker::new();
            marker.set_location(first.0, first.1);
            let icon = gtk::Image::from_icon_name("find-location-symbolic");
            icon.set_pixel_size(22);
            icon.add_css_class("map-pin-start");
            marker.set_child(Some(&icon));
            self.marker_layer.add_marker(&marker);
            *self.route_start_marker.borrow_mut() = Some(marker);
        }

        // End marker (destination)
        if let Some(last) = coordinates.last() {
            let marker = Marker::new();
            marker.set_location(last.0, last.1);
            let icon = gtk::Image::from_icon_name("mark-location-symbolic");
            icon.set_pixel_size(24);
            icon.add_css_class("map-pin-accent");
            marker.set_child(Some(&icon));
            self.marker_layer.add_marker(&marker);
            *self.route_end_marker.borrow_mut() = Some(marker);
        }

        self.fit_coordinates(coordinates);
    }

    pub fn clear_route(&self) {
        self.route_layer.remove_all();
        if let Some(m) = self.route_start_marker.borrow_mut().take() {
            self.marker_layer.remove_marker(&m);
        }
        if let Some(m) = self.route_end_marker.borrow_mut().take() {
            self.marker_layer.remove_marker(&m);
        }
    }

    pub fn zoom_in(&self) {
        if let Some(map) = self.root.map() {
            map.zoom_in();
        } else if let Some(vp) = self.root.viewport() {
            let current = vp.zoom_level();
            let max = vp.max_zoom_level() as f64;
            vp.set_zoom_level((current + 1.0).min(max));
        }
    }

    pub fn zoom_out(&self) {
        if let Some(map) = self.root.map() {
            map.zoom_out();
        } else if let Some(vp) = self.root.viewport() {
            let current = vp.zoom_level();
            let min = vp.min_zoom_level() as f64;
            vp.set_zoom_level((current - 1.0).max(min));
        }
    }

    pub fn get_viewport_state(&self) -> (f64, f64, f64) {
        if let Some(vp) = self.root.viewport() {
            (vp.latitude(), vp.longitude(), vp.zoom_level() - self.zoom_offset.get())
        } else {
            (0.0, 0.0, 0.0)
        }
    }
}
