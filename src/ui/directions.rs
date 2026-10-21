use crate::providers::{RouteResult, TravelMode, calculate_route, format_distance, format_duration};
use crate::settings;
use crate::storage::Database;
use gtk::glib;
use gtk::prelude::*;
use nixe_ui::widgets;
use std::cell::RefCell;
use std::rc::Rc;

pub struct DirectionsPanel {
    pub root: gtk::Box,
    pub origin_entry: gtk::Entry,
    pub dest_entry: gtk::Entry,
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
    pub fn new(
        db: Database,
        on_route_calculated: impl Fn(RouteResult) + 'static,
        on_close: impl Fn() + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.add_css_class("panel-body");

        // Header
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::new(Some("DIRECTIONS"));
        title.add_css_class("panel-title");
        title.set_hexpand(true);
        title.set_xalign(0.0);
        header.append(&title);

        let close_btn = widgets::bar_button("window-close-symbolic", "Close directions");
        close_btn.connect_clicked(move |_| on_close());
        header.append(&close_btn);
        root.append(&header);

        // Travel mode switcher
        let mode_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let mode_drive = gtk::Button::with_label("Drive");
        mode_drive.add_css_class("travel-mode-btn");
        mode_drive.add_css_class("active");

        let mode_walk = gtk::Button::with_label("Walk");
        mode_walk.add_css_class("travel-mode-btn");

        let mode_cycle = gtk::Button::with_label("Cycle");
        mode_cycle.add_css_class("travel-mode-btn");

        mode_box.append(&mode_drive);
        mode_box.append(&mode_walk);
        mode_box.append(&mode_cycle);
        root.append(&mode_box);

        let current_mode = Rc::new(RefCell::new(TravelMode::Drive));

        // Origin and Destination inputs
        let inputs_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inputs_box.set_margin_top(4);

        let fields_vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
        fields_vbox.set_hexpand(true);

        let origin_entry = gtk::Entry::new();
        origin_entry.set_placeholder_text(Some("Starting point (or 'My Location')"));
        origin_entry.add_css_class("waypoint-entry");
        fields_vbox.append(&origin_entry);

        let dest_entry = gtk::Entry::new();
        dest_entry.set_placeholder_text(Some("Destination"));
        dest_entry.add_css_class("waypoint-entry");
        fields_vbox.append(&dest_entry);

        inputs_box.append(&fields_vbox);

        // Reverse button
        let reverse_btn = widgets::bar_button("view-refresh-symbolic", "Reverse start and destination");
        reverse_btn.set_valign(gtk::Align::Center);
        inputs_box.append(&reverse_btn);
        root.append(&inputs_box);

        // Route button
        let get_route_btn = gtk::Button::with_label("Get Directions");
        get_route_btn.add_css_class("suggested-action");
        get_route_btn.set_margin_top(4);
        root.append(&get_route_btn);

        // Status / Error message
        let status_label = gtk::Label::new(None);
        status_label.add_css_class("dim");
        status_label.set_visible(false);
        root.append(&status_label);

        // Summary Card
        let summary_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        summary_box.add_css_class("route-summary-card");
        summary_box.set_visible(false);

        let stats_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let time_label = gtk::Label::new(None);
        time_label.add_css_class("route-stat-value");
        stats_row.append(&time_label);

        let dist_label = gtk::Label::new(None);
        dist_label.add_css_class("route-stat-label");
        stats_row.append(&dist_label);
        summary_box.append(&stats_row);

        root.append(&summary_box);

        // Step-by-step maneuvers list
        let steps_scroller = gtk::ScrolledWindow::new();
        steps_scroller.set_vexpand(true);
        steps_scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let steps_list = gtk::ListBox::new();
        steps_list.add_css_class("group-list");
        steps_scroller.set_child(Some(&steps_list));
        root.append(&steps_scroller);

        let current_origin = Rc::new(RefCell::new(None));
        let current_dest = Rc::new(RefCell::new(None));

        // Connect reverse button
        let o_ent = origin_entry.clone();
        let d_ent = dest_entry.clone();
        let co_clone = current_origin.clone();
        let cd_clone = current_dest.clone();
        reverse_btn.connect_clicked(move |_| {
            let o_text = o_ent.text().to_string();
            let d_text = d_ent.text().to_string();
            o_ent.set_text(&d_text);
            d_ent.set_text(&o_text);

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

        get_route_btn.connect_clicked(move |_| {
            let o_str = o_e.text().trim().to_string();
            let d_str = d_e.text().trim().to_string();

            if o_str.is_empty() || d_str.is_empty() {
                st_lbl.set_text("Please enter both origin and destination.");
                st_lbl.set_visible(true);
                return;
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
                            st_lbl_c.set_visible(false);
                            s_box_c.set_visible(true);

                            let units = settings::get().units;
                            d_lbl_c.set_text(&format_distance(route.distance_meters, &units));
                            t_lbl_c.set_text(&format_duration(route.duration_seconds));

                            // Populate steps
                            while let Some(child) = st_list_c.first_child() {
                                st_list_c.remove(&child);
                            }

                            for step in &route.steps {
                                let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
                                row.add_css_class("step-row");

                                let instr = gtk::Label::new(Some(&step.instruction));
                                instr.add_css_class("step-text");
                                instr.set_xalign(0.0);
                                instr.set_wrap(true);
                                row.append(&instr);

                                let dist = gtk::Label::new(Some(&format_distance(step.distance_meters, &units)));
                                dist.add_css_class("step-distance");
                                dist.set_xalign(0.0);
                                row.append(&dist);

                                st_list_c.append(&row);
                            }

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
            origin_entry,
            dest_entry,
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
        *self.current_origin.borrow_mut() = Some((lat, lon));
        self.origin_entry.set_text(text);
    }

    pub fn set_destination(&self, lat: f64, lon: f64, text: &str) {
        *self.current_dest.borrow_mut() = Some((lat, lon));
        self.dest_entry.set_text(text);
    }
}

async fn resolve_point(text: &str, api_key: &str, db: &Database) -> Option<(f64, f64)> {
    if text.eq_ignore_ascii_case("home") {
        if let Ok(Some(p)) = db.get_favorite_by_label("home") {
            return Some((p.lat, p.lon));
        }
    } else if text.eq_ignore_ascii_case("work") {
        if let Ok(Some(p)) = db.get_favorite_by_label("work") {
            return Some((p.lat, p.lon));
        }
    }

    if let Ok(results) = crate::providers::search_places(text, api_key).await {
        if let Some(first) = results.into_iter().next() {
            return Some((first.lat, first.lon));
        }
    }
    None
}
