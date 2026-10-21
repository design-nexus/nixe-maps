use crate::location::request_current_location;
use crate::providers::{MapStyle, SearchResult};
use crate::settings;
use crate::settings_dialog;
use crate::storage::{Database, db};
use crate::ui::directions::DirectionsPanel;
use crate::ui::map::MapView;
use crate::ui::places::{PlaceCard, PlacesHubView};
use crate::ui::search::SearchBar;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use libshumate::prelude::*;
use nixe_ui::chrome;
use nixe_ui::widgets;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct MainWindow {
    pub window: gtk::ApplicationWindow,
    pub overlay: gtk::Overlay,
    map_view: MapView,
    side_panel: gtk::Box,
    panel_stack: gtk::Stack,
    search_bar: Rc<SearchBar>,
    directions_panel: Rc<DirectionsPanel>,
    place_card: Rc<PlaceCard>,
    places_hub_view: Rc<PlacesHubView>,
    btn_seg_places: gtk::Button,
    btn_seg_route: gtk::Button,
    style_btn: gtk::Button,
    attribution_lbl: gtk::Label,
    coords_lbl: gtk::Label,
    toast_lbl: gtk::Label,
    toast_timer: Rc<RefCell<Option<glib::SourceId>>>,
    db: Database,
}

impl MainWindow {
    pub fn build(app: &gtk::Application) -> Rc<Self> {
        let database = db().expect("Failed to initialize maps database");
        let initial_settings = settings::get();

        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Maps")
            .default_width(1120)
            .default_height(800)
            .build();
        window.add_css_class("maps-window");
        window.set_titlebar(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
        chrome::set_title(&window, "Maps", "");

        let overlay = gtk::Overlay::new();
        let main_vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

        // --- TOP BAR ---
        let topbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        topbar.add_css_class("maps-topbar");

        let sidebar_toggle = widgets::bar_button("sidebar-show-symbolic", "Toggle side panel (Ctrl+B)");
        topbar.append(&sidebar_toggle);

        let app_title = gtk::Label::new(Some("Maps"));
        app_title.add_css_class("maps-title-accent");
        topbar.append(&app_title);

        let cur_style = MapStyle::from_str(&initial_settings.map_style);
        let style_btn = gtk::Button::with_label(&format!("{} ▾", cur_style.display_name()));
        style_btn.add_css_class("style-selector");
        style_btn.set_tooltip_text(Some("Switch map style"));

        let style_popover = gtk::Popover::new();
        let style_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        style_box.set_margin_start(4);
        style_box.set_margin_end(4);
        style_box.set_margin_top(4);
        style_box.set_margin_bottom(4);

        let btn_std = gtk::Button::with_label("Standard");
        btn_std.add_css_class("flat");
        let btn_topo = gtk::Button::with_label("Topographic");
        btn_topo.add_css_class("flat");
        let btn_sat = gtk::Button::with_label("Satellite");
        btn_sat.add_css_class("flat");

        style_box.append(&btn_std);
        style_box.append(&btn_topo);
        style_box.append(&btn_sat);
        style_popover.set_child(Some(&style_box));
        style_popover.set_parent(&style_btn);

        let sp_clone = style_popover.clone();
        style_btn.connect_clicked(move |_| sp_clone.popup());
        topbar.append(&style_btn);

        // Search Bar Holder
        let search_holder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        search_holder.set_hexpand(true);
        topbar.append(&search_holder);

        // Directions Button
        let directions_toggle = widgets::bar_button("directions-symbolic", "Directions (Ctrl+R)");
        topbar.append(&directions_toggle);

        // Locate Me Button
        let locate_btn = widgets::bar_button("find-location-symbolic", "Find my location");
        topbar.append(&locate_btn);

        main_vbox.append(&topbar);

        // --- BODY (SIDE PANEL + MAP) ---
        let body_hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body_hbox.set_hexpand(true);
        body_hbox.set_vexpand(true);

        // Contextual Side Panel (320px)
        let side_panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        side_panel.add_css_class("context-panel");
        side_panel.set_hexpand(false);
        side_panel.set_size_request(320, -1);
        side_panel.set_visible(false);

        // Sidebar Header: Segmented Switcher [ Places | Directions ] + Close
        let panel_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        panel_header.add_css_class("panel-header");

        let seg_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        seg_box.add_css_class("panel-segmented");
        seg_box.set_hexpand(true);

        let btn_seg_places = gtk::Button::with_label("Places");
        btn_seg_places.add_css_class("segment-btn");
        btn_seg_places.add_css_class("active");
        btn_seg_places.set_hexpand(true);

        let btn_seg_route = gtk::Button::with_label("Directions");
        btn_seg_route.add_css_class("segment-btn");
        btn_seg_route.set_hexpand(true);

        seg_box.append(&btn_seg_places);
        seg_box.append(&btn_seg_route);
        panel_header.append(&seg_box);

        let panel_close = widgets::bar_button("window-close-symbolic", "Collapse sidebar (Ctrl+B)");
        panel_header.append(&panel_close);
        side_panel.append(&panel_header);

        let panel_stack = gtk::Stack::new();
        panel_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        panel_stack.set_hexpand(false);
        panel_stack.set_vexpand(true);
        side_panel.append(&panel_stack);

        body_hbox.append(&side_panel);

        // Map View
        let map_view = MapView::new(
            initial_settings.last_lat,
            initial_settings.last_lon,
            initial_settings.last_zoom,
            cur_style,
        );

        // Map overlay container for floating controls
        let map_overlay = gtk::Overlay::new();
        map_overlay.set_hexpand(true);
        map_overlay.set_vexpand(true);
        map_overlay.set_child(Some(&map_view.root));

        // Floating zoom controls on map
        let zoom_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        zoom_box.add_css_class("map-floating-controls");
        zoom_box.set_halign(gtk::Align::End);
        zoom_box.set_valign(gtk::Align::End);
        zoom_box.set_margin_end(16);
        zoom_box.set_margin_bottom(16);

        let zoom_in_btn = gtk::Button::new();
        zoom_in_btn.add_css_class("map-control-btn");
        let zin_icon = gtk::Image::from_icon_name("zoom-in-symbolic");
        zin_icon.set_pixel_size(16);
        zoom_in_btn.set_child(Some(&zin_icon));
        zoom_in_btn.set_tooltip_text(Some("Zoom in (+)"));
        let mv_zin = map_view.clone();
        zoom_in_btn.connect_clicked(move |_| mv_zin.zoom_in());
        zoom_box.append(&zoom_in_btn);

        let zoom_out_btn = gtk::Button::new();
        zoom_out_btn.add_css_class("map-control-btn");
        let zout_icon = gtk::Image::from_icon_name("zoom-out-symbolic");
        zout_icon.set_pixel_size(16);
        zoom_out_btn.set_child(Some(&zout_icon));
        zoom_out_btn.set_tooltip_text(Some("Zoom out (−)"));
        let mv_zout = map_view.clone();
        zoom_out_btn.connect_clicked(move |_| mv_zout.zoom_out());
        zoom_box.append(&zoom_out_btn);

        map_overlay.add_overlay(&zoom_box);
        body_hbox.append(&map_overlay);
        main_vbox.append(&body_hbox);

        // --- STATUS BAR ---
        let statusbar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        statusbar.add_css_class("maps-statusbar");

        let shortcuts_btn = chrome::shortcuts_button(|| {
            show_shortcuts();
        });
        statusbar.append(&shortcuts_btn);

        let status_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        status_spacer.set_hexpand(true);
        statusbar.append(&status_spacer);

        let coords_lbl = gtk::Label::new(Some(&format!("{:.4}, {:.4}", initial_settings.last_lat, initial_settings.last_lon)));
        coords_lbl.add_css_class("dim");
        statusbar.append(&coords_lbl);

        let attribution_lbl = gtk::Label::new(Some(cur_style.attribution()));
        attribution_lbl.add_css_class("dim");
        statusbar.append(&attribution_lbl);

        let overlay_gear = overlay.clone();
        let win_gear = window.clone();
        let db_gear = database.clone();
        let gear_btn = chrome::gear(move || {
            settings_dialog::open(&overlay_gear, &win_gear, db_gear.clone());
        });
        statusbar.append(&gear_btn);

        main_vbox.append(&statusbar);
        overlay.set_child(Some(&main_vbox));

        // Toast label overlay
        let toast_lbl = gtk::Label::new(None);
        toast_lbl.add_css_class("maps-toast");
        toast_lbl.set_halign(gtk::Align::Center);
        toast_lbl.set_valign(gtk::Align::End);
        toast_lbl.set_visible(false);
        overlay.add_overlay(&toast_lbl);

        window.set_child(Some(&overlay));

        let toast_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

        // --- CONSTRUCT SUB-PANELS ---
        // 1. Directions Panel
        let mv_dir = map_view.clone();
        let sp_dir = side_panel.clone();
        let ps_dir = panel_stack.clone();
        let bp_dir = btn_seg_places.clone();
        let br_dir = btn_seg_route.clone();
        let directions_panel = Rc::new(DirectionsPanel::new(
            database.clone(),
            move |route| {
                mv_dir.show_route(&route.coordinates);
                if let Some(first) = route.coordinates.first() {
                    mv_dir.center_on(first.0, first.1, None);
                }
            },
            {
                let ps = ps_dir.clone();
                let bp = bp_dir.clone();
                let br = br_dir.clone();
                move || {
                    ps.set_visible_child_name("hub");
                    br.remove_css_class("active");
                    bp.add_css_class("active");
                }
            },
            {
                let sp = sp_dir.clone();
                let mv = map_view.clone();
                move || {
                    sp.set_visible(false);
                    mv.clear_route();
                }
            },
        ));
        panel_stack.add_named(&directions_panel.root, Some("directions"));

        // 2. Place Card
        let dp_c = directions_panel.clone();
        let pstack_c = panel_stack.clone();
        let mv_pc = map_view.clone();
        let bp_pc = btn_seg_places.clone();
        let br_pc = btn_seg_route.clone();
        let place_card = Rc::new(PlaceCard::new(
            database.clone(),
            {
                let ps = pstack_c.clone();
                let bp = bp_pc.clone();
                let br = br_pc.clone();
                move || {
                    ps.set_visible_child_name("hub");
                    br.remove_css_class("active");
                    bp.add_css_class("active");
                }
            },
            {
                let dp = dp_c.clone();
                let ps = pstack_c.clone();
                let bp = bp_pc.clone();
                let br = br_pc.clone();
                move |place| {
                    dp.set_destination(place.lat, place.lon, &place.name);
                    ps.set_visible_child_name("directions");
                    bp.remove_css_class("active");
                    br.add_css_class("active");
                }
            },
            {
                let mv = mv_pc.clone();
                let ps = pstack_c.clone();
                let bp = bp_pc.clone();
                let br = br_pc.clone();
                move || {
                    mv.clear_selected_marker();
                    ps.set_visible_child_name("hub");
                    br.remove_css_class("active");
                    bp.add_css_class("active");
                }
            },
        ));
        panel_stack.add_named(&place_card.root, Some("place"));

        // 3. Places Hub
        let mv_hub = map_view.clone();
        let pc_hub = place_card.clone();
        let ps_hub = panel_stack.clone();
        let db_hub = database.clone();
        let bp_hub = btn_seg_places.clone();
        let br_hub = btn_seg_route.clone();

        let toast_lbl_c = toast_lbl.clone();
        let toast_timer_c = toast_timer.clone();
        let toast_fn = move |msg: &str| {
            toast_lbl_c.set_text(msg);
            toast_lbl_c.set_visible(true);
            if let Some(src) = toast_timer_c.borrow_mut().take() {
                src.remove();
            }
            let lbl = toast_lbl_c.clone();
            let timer = toast_timer_c.clone();
            let id = glib::timeout_add_local_once(Duration::from_millis(3500), move || {
                lbl.set_visible(false);
                *timer.borrow_mut() = None;
            });
            *toast_timer_c.borrow_mut() = Some(id);
        };

        let search_bar_cell: Rc<RefCell<Option<Rc<SearchBar>>>> = Rc::new(RefCell::new(None));
        let sbc_clone = search_bar_cell.clone();

        let places_hub_view = Rc::new(PlacesHubView::new(
            database.clone(),
            {
                let mv = mv_hub.clone();
                let pc = pc_hub.clone();
                let ps = ps_hub.clone();
                let db = db_hub.clone();
                let bp = bp_hub.clone();
                let br = br_hub.clone();
                move |place| {
                    mv.center_on(place.lat, place.lon, Some(14.0));
                    mv.set_selected_marker(place.lat, place.lon, &place.name);
                    let is_fav = db.list_favorites().map(|favs| favs.iter().any(|f| f.name == place.name)).unwrap_or(false);
                    pc.set_place(&place, is_fav);
                    ps.set_visible_child_name("place");
                    br.remove_css_class("active");
                    bp.add_css_class("active");
                }
            },
            {
                let ps = ps_hub.clone();
                let bp = bp_hub.clone();
                let br = br_hub.clone();
                move || {
                    ps.set_visible_child_name("directions");
                    bp.remove_css_class("active");
                    br.add_css_class("active");
                }
            },
            {
                let sbc = sbc_clone.clone();
                move |query| {
                    if let Some(ref sb) = *sbc.borrow() {
                        sb.set_text(query);
                        sb.focus();
                    }
                }
            },
            toast_fn,
        ));
        panel_stack.add_named(&places_hub_view.root, Some("hub"));

        // 4. Search Bar
        let mv_search = map_view.clone();
        let pc_search = place_card.clone();
        let pstack_search = panel_stack.clone();
        let sp_search = side_panel.clone();
        let phv_search = places_hub_view.clone();
        let db_search = database.clone();
        let bp_search = btn_seg_places.clone();
        let br_search = btn_seg_route.clone();
        let search_bar = Rc::new(SearchBar::new(database.clone(), move |place| {
            mv_search.center_on(place.lat, place.lon, Some(14.0));
            mv_search.set_selected_marker(place.lat, place.lon, &place.name);
            let is_fav = db_search.list_favorites().map(|favs| favs.iter().any(|f| f.name == place.name)).unwrap_or(false);
            pc_search.set_place(&place, is_fav);
            pstack_search.set_visible_child_name("place");
            br_search.remove_css_class("active");
            bp_search.add_css_class("active");
            sp_search.set_visible(true);
            phv_search.refresh();
        }));
        *search_bar_cell.borrow_mut() = Some(search_bar.clone());
        search_holder.append(&search_bar.root);

        // Wire Segment buttons
        let ps_s1 = panel_stack.clone();
        let bp_s1 = btn_seg_places.clone();
        let br_s1 = btn_seg_route.clone();
        let phv_s1 = places_hub_view.clone();
        btn_seg_places.connect_clicked(move |_| {
            phv_s1.refresh();
            ps_s1.set_visible_child_name("hub");
            br_s1.remove_css_class("active");
            bp_s1.add_css_class("active");
        });

        let ps_s2 = panel_stack.clone();
        let bp_s2 = btn_seg_places.clone();
        let br_s2 = btn_seg_route.clone();
        btn_seg_route.connect_clicked(move |_| {
            ps_s2.set_visible_child_name("directions");
            bp_s2.remove_css_class("active");
            br_s2.add_css_class("active");
        });

        let sp_close = side_panel.clone();
        panel_close.connect_clicked(move |_| {
            sp_close.set_visible(false);
        });

        let main_win = Rc::new(Self {
            window,
            overlay,
            map_view,
            side_panel,
            panel_stack,
            search_bar,
            directions_panel,
            place_card,
            places_hub_view,
            btn_seg_places,
            btn_seg_route,
            style_btn,
            attribution_lbl,
            coords_lbl,
            toast_lbl,
            toast_timer,
            db: database,
        });

        main_win.bind_events(
            sidebar_toggle,
            directions_toggle,
            locate_btn,
            btn_std,
            btn_topo,
            btn_sat,
            style_popover,
        );

        main_win
    }

    fn bind_events(
        self: &Rc<Self>,
        sidebar_toggle: gtk::Button,
        directions_toggle: gtk::Button,
        locate_btn: gtk::Button,
        btn_std: gtk::Button,
        btn_topo: gtk::Button,
        btn_sat: gtk::Button,
        style_popover: gtk::Popover,
    ) {
        // Toggle side panel
        let sp_clone = self.side_panel.clone();
        let pstack_clone = self.panel_stack.clone();
        let hub_clone = self.places_hub_view.clone();
        let bp_clone = self.btn_seg_places.clone();
        let br_clone = self.btn_seg_route.clone();

        sidebar_toggle.connect_clicked(move |_| {
            if sp_clone.is_visible() {
                sp_clone.set_visible(false);
            } else {
                hub_clone.refresh();
                if pstack_clone.visible_child_name().is_none() {
                    pstack_clone.set_visible_child_name("hub");
                    br_clone.remove_css_class("active");
                    bp_clone.add_css_class("active");
                }
                sp_clone.set_visible(true);
            }
        });

        // Toggle directions
        let sp_c2 = self.side_panel.clone();
        let pstack_c2 = self.panel_stack.clone();
        let bp_c2 = self.btn_seg_places.clone();
        let br_c2 = self.btn_seg_route.clone();
        directions_toggle.connect_clicked(move |_| {
            if sp_c2.is_visible() && pstack_c2.visible_child_name().as_deref() == Some("directions") {
                sp_c2.set_visible(false);
            } else {
                pstack_c2.set_visible_child_name("directions");
                bp_c2.remove_css_class("active");
                br_c2.add_css_class("active");
                sp_c2.set_visible(true);
            }
        });

        // Locate Me
        let self_loc = self.clone();
        locate_btn.connect_clicked(move |_| {
            self_loc.locate_user();
        });

        // Style switcher buttons
        let self_s1 = self.clone();
        let pop1 = style_popover.clone();
        btn_std.connect_clicked(move |_| {
            pop1.popdown();
            self_s1.set_map_style(MapStyle::Standard);
        });

        let self_s2 = self.clone();
        let pop2 = style_popover.clone();
        btn_topo.connect_clicked(move |_| {
            pop2.popdown();
            self_s2.set_map_style(MapStyle::Topographic);
        });

        let self_s3 = self.clone();
        let pop3 = style_popover.clone();
        btn_sat.connect_clicked(move |_| {
            pop3.popdown();
            self_s3.set_map_style(MapStyle::Satellite);
        });

        // Update coordinate readout on pan
        let coords_lbl_c = self.coords_lbl.clone();
        if let Some(vp) = self.map_view.root.viewport() {
            vp.connect_latitude_notify(move |v| {
                coords_lbl_c.set_text(&format!("{:.4}, {:.4}", v.latitude(), v.longitude()));
            });
        }

        // Keyboard navigation
        let self_keys = self.clone();
        let key_ctl = gtk::EventControllerKey::new();
        key_ctl.connect_key_pressed(move |_, key, _, modifier| {
            let ctrl = modifier.contains(gdk::ModifierType::CONTROL_MASK);

            if ctrl && key == gdk::Key::f {
                self_keys.search_bar.focus();
                return glib::Propagation::Stop;
            }
            if ctrl && key == gdk::Key::b {
                let v = self_keys.side_panel.is_visible();
                if v {
                    self_keys.side_panel.set_visible(false);
                } else {
                    self_keys.places_hub_view.refresh();
                    if self_keys.panel_stack.visible_child_name().is_none() {
                        self_keys.panel_stack.set_visible_child_name("hub");
                        self_keys.set_active_segment("places");
                    }
                    self_keys.side_panel.set_visible(true);
                }
                return glib::Propagation::Stop;
            }
            if ctrl && key == gdk::Key::r {
                self_keys.panel_stack.set_visible_child_name("directions");
                self_keys.set_active_segment("directions");
                self_keys.side_panel.set_visible(true);
                return glib::Propagation::Stop;
            }
            if ctrl && key == gdk::Key::comma {
                settings_dialog::open(&self_keys.overlay, &self_keys.window, self_keys.db.clone());
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::F1 {
                show_shortcuts();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::plus || key == gdk::Key::equal {
                self_keys.map_view.zoom_in();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::minus {
                self_keys.map_view.zoom_out();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape {
                if self_keys.side_panel.is_visible() {
                    let cur = self_keys.panel_stack.visible_child_name();
                    if cur.as_deref() == Some("place") || cur.as_deref() == Some("directions") {
                        self_keys.panel_stack.set_visible_child_name("hub");
                        self_keys.set_active_segment("places");
                    } else {
                        self_keys.side_panel.set_visible(false);
                    }
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        self.window.add_controller(key_ctl);

        // Save position on close
        let mv_close = self.map_view.clone();
        self.window.connect_close_request(move |_| {
            let (lat, lon, zoom) = mv_close.get_viewport_state();
            settings::update(|s| {
                s.last_lat = lat;
                s.last_lon = lon;
                s.last_zoom = zoom;
            });
            glib::Propagation::Proceed
        });
    }

    pub fn set_active_segment(&self, mode: &str) {
        if mode == "directions" {
            self.btn_seg_places.remove_css_class("active");
            self.btn_seg_route.add_css_class("active");
        } else {
            self.btn_seg_route.remove_css_class("active");
            self.btn_seg_places.add_css_class("active");
        }
    }

    pub fn set_map_style(&self, style: MapStyle) {
        let prefs = settings::get();
        self.map_view.set_style(style, &prefs.custom_satellite_url, &prefs.custom_topo_url);
        self.style_btn.set_label(&format!("{} ▾", style.display_name()));
        self.attribution_lbl.set_text(style.attribution());
        settings::update(|s| s.map_style = style.as_str().to_string());
    }

    pub fn locate_user(self: &Rc<Self>) {
        self.show_toast("Locating...");

        let (sender, receiver) = async_channel::bounded(1);
        crate::runtime::spawn(async move {
            let res = request_current_location().await;
            let _ = sender.send(res).await;
        });

        let self_c = self.clone();
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                match res {
                    Ok(loc) => {
                        self_c.map_view.center_on(loc.latitude, loc.longitude, Some(14.0));
                        self_c.map_view.set_user_location(loc.latitude, loc.longitude);
                        let name = loc.label.unwrap_or_else(|| "Current Location".to_string());
                        self_c.show_toast(&format!("Located: {} (±{:.0}m)", name, loc.accuracy));
                        self_c.directions_panel.set_origin(loc.latitude, loc.longitude, "My Location");
                        self_c.coords_lbl.set_text(&format!("{:.4}, {:.4}", loc.latitude, loc.longitude));
                    }
                    Err(e) => {
                        self_c.show_toast(&format!("Location unavailable: {}", e));
                    }
                }
            }
        });
    }

    pub fn show_toast(&self, message: &str) {
        self.toast_lbl.set_text(message);
        self.toast_lbl.set_visible(true);

        if let Some(src) = self.toast_timer.borrow_mut().take() {
            src.remove();
        }

        let lbl = self.toast_lbl.clone();
        let timer = self.toast_timer.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(3500), move || {
            lbl.set_visible(false);
            *timer.borrow_mut() = None;
        });
        *self.toast_timer.borrow_mut() = Some(id);
    }

    pub fn open_location(&self, lat: f64, lon: f64, label: &str) {
        self.map_view.center_on(lat, lon, Some(14.0));
        self.map_view.set_selected_marker(lat, lon, label);
        let is_fav = self.db.list_favorites().map(|favs| favs.iter().any(|f| f.name == label)).unwrap_or(false);
        self.place_card.set_place(
            &SearchResult {
                name: label.to_string(),
                formatted_address: format!("{:.5}, {:.5}", lat, lon),
                lat,
                lon,
                category: None,
            },
            is_fav,
        );
        self.set_active_segment("places");
        self.panel_stack.set_visible_child_name("place");
        self.side_panel.set_visible(true);
    }

    pub fn open_route(&self, start: (f64, f64), end: (f64, f64), mode: &str) {
        self.directions_panel.set_origin(start.0, start.1, &format!("{:.4}, {:.4}", start.0, start.1));
        self.directions_panel.set_destination(end.0, end.1, &format!("{:.4}, {:.4}", end.0, end.1));
        self.set_active_segment("directions");
        self.panel_stack.set_visible_child_name("directions");
        self.side_panel.set_visible(true);

        let mv = self.map_view.clone();
        let t_mode = crate::providers::TravelMode::from_str(mode);
        let api_key = settings::get().geoapify_api_key;

        let (sender, receiver) = async_channel::bounded(1);
        crate::runtime::spawn(async move {
            let res = crate::providers::calculate_route(start, end, t_mode, &api_key).await;
            let _ = sender.send(res).await;
        });

        glib::spawn_future_local(async move {
            if let Ok(Ok(route)) = receiver.recv().await {
                mv.show_route(&route.coordinates);
                mv.center_on(start.0, start.1, None);
            }
        });
    }

    pub fn search_query(&self, query: &str) {
        self.search_bar.set_text(query);
    }
}

fn show_shortcuts() {
    let (dialog, card) = widgets::dialog("Keyboard shortcuts", 520);
    let shortcuts: &[(&str, &str)] = &[
        ("Ctrl+F", "Focus search bar"),
        ("Ctrl+B", "Toggle side panel"),
        ("Ctrl+R", "Open directions panel"),
        ("Ctrl+,", "Open settings"),
        ("+ / =", "Zoom in"),
        ("-", "Zoom out"),
        ("F1", "Show shortcuts list"),
        ("Esc", "Back / Close panels"),
        ("Ctrl+Q", "Quit application"),
    ];

    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    list.add_css_class("group-list");

    for (key, desc) in shortcuts {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("settings-option");

        let desc_lbl = gtk::Label::new(Some(desc));
        desc_lbl.set_hexpand(true);
        desc_lbl.set_xalign(0.0);
        row.append(&desc_lbl);

        let key_lbl = gtk::Label::new(Some(key));
        key_lbl.add_css_class("key-cap");
        row.append(&key_lbl);

        list.append(&row);
    }

    card.append(&list);
    dialog.present();
}
