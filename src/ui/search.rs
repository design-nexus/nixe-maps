use crate::providers::{SearchResult, parse_coordinates, search_places};
use crate::settings;
use crate::storage::Database;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct SearchBar {
    pub root: gtk::Box,
    pub entry: gtk::Entry,
    popover: gtk::Popover,
    results_list: gtk::ListBox,
    current_search_id: Rc<RefCell<u64>>,
    suppress_search: Rc<RefCell<bool>>,
    db: Database,
    on_select: Rc<dyn Fn(SearchResult)>,
}

impl SearchBar {
    pub fn new(db: Database, on_select: impl Fn(SearchResult) + 'static) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("Search places, addresses, or coordinates (lat, lon)..."));
        entry.set_icon_from_icon_name(gtk::EntryIconPosition::Primary, Some("system-search-symbolic"));
        entry.set_icon_activatable(gtk::EntryIconPosition::Primary, false);
        entry.add_css_class("maps-search-entry");
        entry.set_hexpand(true);
        root.append(&entry);

        let popover = gtk::Popover::new();
        popover.set_parent(&entry);
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_autohide(true);

        let pop_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        pop_box.set_size_request(340, -1);
        pop_box.set_margin_start(4);
        pop_box.set_margin_end(4);
        pop_box.set_margin_top(4);
        pop_box.set_margin_bottom(4);

        let results_list = gtk::ListBox::new();
        results_list.add_css_class("search-results-list");
        pop_box.append(&results_list);
        popover.set_child(Some(&pop_box));

        let current_search_id = Rc::new(RefCell::new(0u64));
        let suppress_search = Rc::new(RefCell::new(false));
        let on_select: Rc<dyn Fn(SearchResult)> = Rc::new(on_select);

        let search_bar = Self {
            root,
            entry,
            popover,
            results_list,
            current_search_id,
            suppress_search,
            db,
            on_select,
        };

        search_bar.setup_signals();
        search_bar
    }

    fn setup_signals(&self) {
        let entry = self.entry.clone();
        let search_id = self.current_search_id.clone();
        let suppress = self.suppress_search.clone();
        let popover = self.popover.clone();
        let results_list = self.results_list.clone();
        let on_sel = self.on_select.clone();
        let db_c = self.db.clone();

        entry.connect_changed(move |e| {
            if *suppress.borrow() {
                *suppress.borrow_mut() = false;
                return;
            }

            let text = e.text().trim().to_string();
            if text.is_empty() {
                popover.popdown();
                return;
            }

            // Direct coordinates check - can center without waiting for network debounce
            if let Some((lat, lon)) = parse_coordinates(&text) {
                let place = SearchResult {
                    name: format!("{:.5}, {:.5}", lat, lon),
                    formatted_address: format!("Latitude: {:.5}, Longitude: {:.5}", lat, lon),
                    lat,
                    lon,
                    category: Some("Coordinate".to_string()),
                };
                populate_results(&results_list, &popover, vec![place], on_sel.clone(), &db_c);
                popover.popup();
                return;
            }

            let next_id = {
                let mut id = search_id.borrow_mut();
                *id += 1;
                *id
            };

            let id_check = search_id.clone();
            let pop_c = popover.clone();
            let res_list_c = results_list.clone();
            let sel_c = on_sel.clone();
            let db_inner = db_c.clone();

            glib::timeout_add_local_once(Duration::from_millis(300), move || {
                if *id_check.borrow() != next_id {
                    return; // Stale debounced query
                }

                let api_key = settings::get().geoapify_api_key;
                let (sender, receiver) = async_channel::bounded(1);

                crate::runtime::spawn(async move {
                    let res = search_places(&text, &api_key).await;
                    let _ = sender.send((next_id, res)).await;
                });

                glib::spawn_future_local(async move {
                    if let Ok((res_id, Ok(places))) = receiver.recv().await {
                        if *id_check.borrow() == res_id {
                            if !places.is_empty() {
                                populate_results(&res_list_c, &pop_c, places, sel_c, &db_inner);
                                pop_c.popup();
                            } else {
                                pop_c.popdown();
                            }
                        }
                    }
                });
            });
        });
    }

    pub fn focus(&self) {
        self.entry.grab_focus();
    }

    pub fn set_text(&self, text: &str) {
        self.entry.set_text(text);
    }

    pub fn show_results(&self, query_label: &str, places: Vec<SearchResult>) {
        *self.suppress_search.borrow_mut() = true;
        self.entry.set_text(query_label);

        // Invalidate any existing debounced timer
        let mut id = self.current_search_id.borrow_mut();
        *id += 1;

        if !places.is_empty() {
            populate_results(&self.results_list, &self.popover, places, self.on_select.clone(), &self.db);
            self.popover.popup();
        } else {
            self.popover.popdown();
        }
    }
}

fn populate_results(
    list: &gtk::ListBox,
    popover: &gtk::Popover,
    places: Vec<SearchResult>,
    on_select: Rc<dyn Fn(SearchResult)>,
    db: &Database,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    for place in places {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("search-result-row");

        let icon = gtk::Image::from_icon_name(if place.category.as_deref() == Some("Coordinate") {
            "find-location-symbolic"
        } else {
            "mark-location-symbolic"
        });
        icon.set_pixel_size(16);
        row.append(&icon);

        let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text_box.set_hexpand(true);

        let name_lbl = gtk::Label::new(Some(&place.name));
        name_lbl.add_css_class("place-title");
        name_lbl.set_xalign(0.0);
        name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text_box.append(&name_lbl);

        let addr_lbl = gtk::Label::new(Some(&place.formatted_address));
        addr_lbl.add_css_class("place-desc");
        addr_lbl.set_xalign(0.0);
        addr_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text_box.append(&addr_lbl);

        row.append(&text_box);

        let pl_clone = place.clone();
        let on_sel_clone = on_select.clone();
        let pop_clone = popover.clone();
        let db_c = db.clone();

        let gesture = gtk::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| {
            pop_clone.popdown();
            let _ = db_c.add_recent_search(&pl_clone.name, &pl_clone.formatted_address, pl_clone.lat, pl_clone.lon);
            on_sel_clone(pl_clone.clone());
        });
        row.add_controller(gesture);

        list.append(&row);
    }
}
