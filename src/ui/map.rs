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
    current_style: Rc<RefCell<MapStyle>>,
}

impl MapView {
    pub fn new(initial_lat: f64, initial_lon: f64, initial_zoom: f64, style: MapStyle) -> Self {
        let root = SimpleMap::new();
        root.set_hexpand(true);
        root.set_vexpand(true);

        let viewport = root.viewport().expect("SimpleMap has viewport");
        viewport.set_zoom_level(initial_zoom);
        viewport.set_location(initial_lat, initial_lon);

        let marker_layer = MarkerLayer::new(&viewport);
        let route_layer = PathLayer::new(&viewport);

        // Styling for route line
        let route_color = gdk::RGBA::new(0.537, 0.706, 0.980, 0.9); // Catppuccin Blue / Accent
        route_layer.set_stroke_color(Some(&route_color));
        route_layer.set_stroke_width(5.0);

        root.add_overlay_layer(&route_layer);
        root.add_overlay_layer(&marker_layer);

        let view = Self {
            root,
            marker_layer,
            route_layer,
            selected_marker: Rc::new(RefCell::new(None)),
            user_marker: Rc::new(RefCell::new(None)),
            current_style: Rc::new(RefCell::new(style)),
        };

        view.set_style(style, "", "");
        view
    }

    pub fn set_style(&self, style: MapStyle, custom_sat: &str, custom_topo: &str) {
        *self.current_style.borrow_mut() = style;
        let renderer = create_renderer(style, custom_sat, custom_topo);
        self.root.set_map_source(Some(&renderer));
    }

    #[allow(dead_code)]
    pub fn current_style(&self) -> MapStyle {
        *self.current_style.borrow()
    }

    pub fn center_on(&self, lat: f64, lon: f64, zoom: Option<f64>) {
        if let Some(viewport) = self.root.viewport() {
            viewport.set_location(lat, lon);
            if let Some(z) = zoom {
                viewport.set_zoom_level(z);
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

    pub fn show_route(&self, coordinates: &[(f64, f64)]) {
        self.clear_route();

        for (lat, lon) in coordinates {
            let coord = Coordinate::new();
            coord.set_location(*lat, *lon);
            self.route_layer.add_node(&coord);
        }
    }

    pub fn clear_route(&self) {
        self.route_layer.remove_all();
    }

    pub fn zoom_in(&self) {
        if let Some(vp) = self.root.viewport() {
            let current = vp.zoom_level();
            let max = vp.max_zoom_level() as f64;
            vp.set_zoom_level((current + 1.0).min(max));
        }
    }

    pub fn zoom_out(&self) {
        if let Some(vp) = self.root.viewport() {
            let current = vp.zoom_level();
            let min = vp.min_zoom_level() as f64;
            vp.set_zoom_level((current - 1.0).max(min));
        }
    }

    pub fn get_viewport_state(&self) -> (f64, f64, f64) {
        if let Some(vp) = self.root.viewport() {
            (vp.latitude(), vp.longitude(), vp.zoom_level())
        } else {
            (0.0, 0.0, 0.0)
        }
    }
}
