use crate::providers::{RouteResult, TravelMode, calculate_route, format_distance, format_duration};
use crate::settings;
use crate::storage::Database;
use crate::ui::search::{Near, SearchBar};
use gtk::glib;
use gtk::prelude::*;
use nyx_ui::widgets;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct DirectionsPanel {
    pub root: gtk::Box,
    origin: Rc<SearchBar>,
    dest: Rc<SearchBar>,
    pub get_route_btn: gtk::Button,
    #[allow(dead_code)]
    mode_drive: gtk::Button,
    #[allow(dead_code)]
    mode_walk: gtk::Button,
    #[allow(dead_code)]
    mode_cycle: gtk::Button,
    #[allow(dead_code)]
    current_mode: Rc<RefCell<TravelMode>>,
    #[allow(dead_code)]
    summary_box: gtk::Box,
    #[allow(dead_code)]
    dist_label: gtk::Label,
    #[allow(dead_code)]
    time_label: gtk::Label,
    #[allow(dead_code)]
    steps_list: gtk::ListBox,
    current_origin: Rc<RefCell<Option<(f64, f64)>>>,
    current_dest: Rc<RefCell<Option<(f64, f64)>>>,
    #[allow(dead_code)]
    status_label: gtk::Label,
}

impl DirectionsPanel {
    /// `near` says where searches should look first (you, or the map's centre).
    pub fn new(
        db: Database,
        near: Near,
        on_route_calculated: impl Fn(RouteResult) + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(false);
        root.set_vexpand(true);
        root.set_size_request(320, -1);

        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::External)
            .propagate_natural_width(false)
            .hexpand(false)
            .vexpand(true)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.add_css_class("panel-body");
        content.set_hexpand(false);
        content.set_size_request(320, -1);

        // Travel mode switcher
        let mode_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let make_mode_btn = |icon: &str, label: &str| -> gtk::Button {
            let btn = gtk::Button::new();
            btn.add_css_class("travel-mode-btn");
            btn.set_hexpand(true);
            let h = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            h.set_halign(gtk::Align::Center);
            let img = gtk::Image::from_icon_name(icon);
            img.set_pixel_size(14);
            h.append(&img);
            let lbl = gtk::Label::new(Some(label));
            h.append(&lbl);
            btn.set_child(Some(&h));
            btn
        };

        let mode_drive = make_mode_btn("drive-symbolic", "Drive");
        let initial_mode = TravelMode::from_str(&crate::settings::get().travel_mode);
        let mode_walk = make_mode_btn("walk-symbolic", "Walk");
        let mode_cycle = make_mode_btn("bicycle-symbolic", "Cycle");

        mode_box.append(&mode_drive);
        mode_box.append(&mode_walk);
        mode_box.append(&mode_cycle);
        content.append(&mode_box);

        match initial_mode {
            TravelMode::Drive => mode_drive.add_css_class("active"),
            TravelMode::Walk => mode_walk.add_css_class("active"),
            TravelMode::Bicycle => mode_cycle.add_css_class("active"),
        }
        let current_mode = Rc::new(RefCell::new(initial_mode));

        // Origin and Destination inputs
        let inputs_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inputs_box.set_margin_top(4);

        let fields_vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
        fields_vbox.set_hexpand(true);

        let current_origin: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));
        let current_dest: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));

        // Start and destination: type a name or an address and pick from the places
        // found, nearest first.
        let co = current_origin.clone();
        let origin = Rc::new(SearchBar::field(db.clone(), "Start (My Location)", near.clone(), move |p| {
            *co.borrow_mut() = Some((p.lat, p.lon));
        }));
        fields_vbox.append(&origin.root);
        let cd = current_dest.clone();
        let dest = Rc::new(SearchBar::field(db.clone(), "Search for a destination", near, move |p| {
            *cd.borrow_mut() = Some((p.lat, p.lon));
        }));
        fields_vbox.append(&dest.root);
        let origin_entry = origin.entry.clone();
        let dest_entry = dest.entry.clone();

        inputs_box.append(&fields_vbox);

        // Reverse button
        let reverse_btn = widgets::bar_button("view-refresh-symbolic", "Reverse start and destination");
        reverse_btn.set_valign(gtk::Align::Center);
        inputs_box.append(&reverse_btn);
        content.append(&inputs_box);

        // Route button
        let get_route_btn = gtk::Button::with_label("Get Directions");
        get_route_btn.add_css_class("suggested-action");
        get_route_btn.set_margin_top(4);
        content.append(&get_route_btn);

        // Status / Error message
        let status_label = gtk::Label::new(None);
        status_label.add_css_class("dim");
        status_label.set_visible(false);
        status_label.set_wrap(true);
        status_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        status_label.set_natural_wrap_mode(gtk::NaturalWrapMode::None);
        status_label.set_max_width_chars(28);
        content.append(&status_label);

        // Summary Card
        let summary_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        summary_box.add_css_class("route-summary-card");
        summary_box.set_visible(false);

        let stats_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let time_label = gtk::Label::new(None);
        time_label.add_css_class("route-stat-value");
        time_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        stats_row.append(&time_label);

        let dist_label = gtk::Label::new(None);
        dist_label.add_css_class("route-stat-label");
        dist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        stats_row.append(&dist_label);
        summary_box.append(&stats_row);

        content.append(&summary_box);

        // Step-by-step maneuvers list
        let steps_list = gtk::ListBox::new();
        steps_list.set_hexpand(false);
        steps_list.add_css_class("group-list");
        content.append(&steps_list);

        scroller.set_child(Some(&content));
        root.append(&scroller);

        // Connect reverse button
        let (o_bar, d_bar) = (origin.clone(), dest.clone());
        let co_clone = current_origin.clone();
        let cd_clone = current_dest.clone();
        reverse_btn.connect_clicked(move |_| {
            let (o_text, d_text) = (o_bar.text(), d_bar.text());
            let (o_pick, d_pick) = (o_bar.has_pick(), d_bar.has_pick());
            o_bar.set_text(&d_text);
            d_bar.set_text(&o_text);
            o_bar.set_picked(d_pick);
            d_bar.set_picked(o_pick);

            let old_o = *co_clone.borrow();
            let old_d = *cd_clone.borrow();
            *co_clone.borrow_mut() = old_d;
            *cd_clone.borrow_mut() = old_o;
        });

        // Mode toggles
        let cm1 = current_mode.clone();
        let md1 = mode_drive.clone();
        let mw1 = mode_walk.clone();
        let mc1 = mode_cycle.clone();
        mode_drive.connect_clicked(move |_| {
            *cm1.borrow_mut() = TravelMode::Drive;
            md1.add_css_class("active");
            mw1.remove_css_class("active");
            mc1.remove_css_class("active");
        });

        let cm2 = current_mode.clone();
        let md2 = mode_drive.clone();
        let mw2 = mode_walk.clone();
        let mc2 = mode_cycle.clone();
        mode_walk.connect_clicked(move |_| {
            *cm2.borrow_mut() = TravelMode::Walk;
            md2.remove_css_class("active");
            mw2.add_css_class("active");
            mc2.remove_css_class("active");
        });

        let cm3 = current_mode.clone();
        let md3 = mode_drive.clone();
        let mw3 = mode_walk.clone();
        let mc3 = mode_cycle.clone();
        mode_cycle.connect_clicked(move |_| {
            *cm3.borrow_mut() = TravelMode::Bicycle;
            md3.remove_css_class("active");
            mw3.remove_css_class("active");
            mc3.add_css_class("active");
        });

        // Trigger calculation
        let on_calc = Rc::new(on_route_calculated);
        let o_e = origin_entry.clone();
        let d_e = dest_entry.clone();
        let co_c = current_origin.clone();
        let cd_c = current_dest.clone();
        let c_mode = current_mode.clone();
        let s_box = summary_box.clone();
        let d_lbl = dist_label.clone();
        let t_lbl = time_label.clone();
        let st_list = steps_list.clone();
        let st_lbl = status_label.clone();
        let btn_clone = get_route_btn.clone();
        let db_c = db.clone();

        // Editing a field forgets the place it held.
        let (co_ch, o_bar) = (current_origin.clone(), origin.clone());
        origin_entry.connect_changed(move |_| {
            if !o_bar.has_pick() {
                *co_ch.borrow_mut() = None;
            }
        });
        let (cd_ch, d_bar) = (current_dest.clone(), dest.clone());
        dest_entry.connect_changed(move |_| {
            if !d_bar.has_pick() {
                *cd_ch.borrow_mut() = None;
            }
        });

        let (o_pick, d_pick) = (origin.clone(), dest.clone());
        get_route_btn.connect_clicked(move |_| {
            let mut o_str = o_e.text().trim().to_string();
            let d_str = d_e.text().trim().to_string();

            // Default to My Location if origin is blank
            if o_str.is_empty() {
                let s = settings::get();
                if let (Some(lat), Some(lon)) = (s.last_current_lat, s.last_current_lon) {
                    *co_c.borrow_mut() = Some((lat, lon));
                    o_e.set_text("My Location");
                    o_str = "My Location".to_string();
                }
            }

            if o_str.is_empty() || d_str.is_empty() {
                st_lbl.set_text("Choose a start and a destination.");
                st_lbl.set_visible(true);
                return;
            }

            // A name that isn't a picked place: show the places it could be, rather than
            // guessing the first one.
            for (text, coords, bar, what) in [
                (&o_str, *co_c.borrow(), &o_pick, "start"),
                (&d_str, *cd_c.borrow(), &d_pick, "destination"),
            ] {
                if coords.is_none() && !is_known_point(text) {
                    bar.search_now();
                    st_lbl.set_text(&format!("Pick the {what} from the list."));
                    st_lbl.set_visible(true);
                    return;
                }
            }

            st_lbl.set_text("Calculating route...");
            st_lbl.set_visible(true);
            btn_clone.set_sensitive(false);

            let (sender, receiver) = async_channel::bounded(1);
            let mode = *c_mode.borrow();
            let api_key = settings::get().geoapify_api_key;
            let o_coords = *co_c.borrow();
            let d_coords = *cd_c.borrow();
            let db_inner = db_c.clone();

            crate::runtime::spawn(async move {
                // Resolve origin coordinates if needed
                let origin = match o_coords {
                    Some(c) => Some(c),
                    None => resolve_point(&o_str, &api_key, &db_inner).await,
                };

                // Resolve dest coordinates if needed
                let dest = match d_coords {
                    Some(c) => Some(c),
                    None => resolve_point(&d_str, &api_key, &db_inner).await,
                };

                let res = if let (Some(start), Some(end)) = (origin, dest) {
                    calculate_route(start, end, mode, &api_key).await
                } else {
                    Err(anyhow::anyhow!("Could not find locations for routing."))
                };

                let _ = sender.send(res).await;
            });

            let on_calc_inner = on_calc.clone();
            let s_box_c = s_box.clone();
            let d_lbl_c = d_lbl.clone();
            let t_lbl_c = t_lbl.clone();
            let st_list_c = st_list.clone();
            let st_lbl_c = st_lbl.clone();
            let btn_c = btn_clone.clone();

            glib::spawn_future_local(async move {
                if let Ok(result) = receiver.recv().await {
                    btn_c.set_sensitive(true);
                    match result {
                        Ok(route) => {
                            populate_route(
                                &s_box_c,
                                &d_lbl_c,
                                &t_lbl_c,
                                &st_list_c,
                                &st_lbl_c,
                                &route,
                            );
                            on_calc_inner(route);
                        }
                        Err(e) => {
                            st_lbl_c.set_text(&format!("Error: {}", e));
                            st_lbl_c.set_visible(true);
                            s_box_c.set_visible(false);
                        }
                    }
                }
            });
        });

        Self {
            root,
            origin,
            dest,
            get_route_btn,
            mode_drive,
            mode_walk,
            mode_cycle,
            current_mode,
            summary_box,
            dist_label,
            time_label,
            steps_list,
            current_origin,
            current_dest,
            status_label,
        }
    }

    pub fn set_origin(&self, lat: f64, lon: f64, text: &str) {
        self.origin.set_text(text);
        self.origin.set_picked(true);
        *self.current_origin.borrow_mut() = Some((lat, lon));
    }

    pub fn set_destination(&self, lat: f64, lon: f64, text: &str) {
        self.dest.set_text(text);
        self.dest.set_picked(true);
        *self.current_dest.borrow_mut() = Some((lat, lon));
    }

    pub fn calculate_route(&self) {
        self.get_route_btn.emit_clicked();
    }

    pub fn get_origin_coords(&self) -> Option<(f64, f64)> {
        *self.current_origin.borrow()
    }

    #[allow(dead_code)]
    pub fn get_dest_coords(&self) -> Option<(f64, f64)> {
        *self.current_dest.borrow()
    }

    pub fn display_route(&self, route: &RouteResult) {
        populate_route(
            &self.summary_box,
            &self.dist_label,
            &self.time_label,
            &self.steps_list,
            &self.status_label,
            route,
        );
    }
}

fn populate_route(
    summary_box: &gtk::Box,
    dist_label: &gtk::Label,
    time_label: &gtk::Label,
    steps_list: &gtk::ListBox,
    status_label: &gtk::Label,
    route: &RouteResult,
) {
    status_label.set_visible(false);
    summary_box.set_visible(true);

    let units = settings::get().units;
    dist_label.set_text(&format_distance(route.distance_meters, &units));
    time_label.set_text(&format_duration(route.duration_seconds));

    while let Some(child) = steps_list.first_child() {
        steps_list.remove(&child);
    }

    for step in &route.steps {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row.add_css_class("step-row");

        let instr = gtk::Label::new(Some(&step.instruction));
        instr.add_css_class("step-text");
        instr.set_xalign(0.0);
        instr.set_wrap(true);
        instr.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        instr.set_natural_wrap_mode(gtk::NaturalWrapMode::None);
        instr.set_max_width_chars(28);
        row.append(&instr);

        let dist = gtk::Label::new(Some(&format_distance(step.distance_meters, &units)));
        dist.add_css_class("step-distance");
        dist.set_xalign(0.0);
        dist.set_wrap(true);
        dist.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        dist.set_natural_wrap_mode(gtk::NaturalWrapMode::None);
        row.append(&dist);

        steps_list.append(&row);
    }
}

/// Words and forms the route can find without a list: you, coordinates, home and work.
fn is_known_point(text: &str) -> bool {
    let t = text.trim();
    ["my location", "current location", "here", "me", "home", "work"].iter().any(|w| t.eq_ignore_ascii_case(w))
        || crate::providers::parse_coordinates(t).is_some()
}

async fn resolve_point(text: &str, api_key: &str, db: &Database) -> Option<(f64, f64)> {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("my location")
        || trimmed.eq_ignore_ascii_case("current location")
        || trimmed.eq_ignore_ascii_case("here")
        || trimmed.eq_ignore_ascii_case("me")
    {
        let s = settings::get();
        if let (Some(lat), Some(lon)) = (s.last_current_lat, s.last_current_lon) {
            return Some((lat, lon));
        }
        if let Ok(loc) = crate::location::request_current_location().await {
            settings::update(|s| {
                s.last_current_lat = Some(loc.latitude);
                s.last_current_lon = Some(loc.longitude);
                s.last_current_label = loc.label.clone();
            });
            return Some((loc.latitude, loc.longitude));
        }
    }

    if let Some(coords) = crate::providers::parse_coordinates(trimmed) {
        return Some(coords);
    }

    if trimmed.eq_ignore_ascii_case("home") {
        if let Ok(Some(p)) = db.get_favorite_by_label("home") {
            return Some((p.lat, p.lon));
        }
    } else if trimmed.eq_ignore_ascii_case("work") {
        if let Ok(Some(p)) = db.get_favorite_by_label("work") {
            return Some((p.lat, p.lon));
        }
    }

    if let Ok(results) = crate::providers::search_places(trimmed, api_key).await {
        if let Some(first) = results.into_iter().next() {
            return Some((first.lat, first.lon));
        }
    }
    None
}
