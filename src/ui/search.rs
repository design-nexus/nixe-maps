use crate::providers::{SearchResult, distance_m, format_distance, parse_coordinates, search_places_near};
use crate::settings;
use crate::storage::Database;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// Where results should be near (the map's centre, or you).
pub type Near = Rc<dyn Fn() -> Option<(f64, f64)>>;

/// A search field with a list of matching places under it, nearest first. The header's
/// search, and the directions panel's start and destination.
pub struct SearchBar {
    pub root: gtk::Box,
    pub entry: gtk::Entry,
    popover: gtk::Popover,
    results_list: gtk::ListBox,
    current_search_id: Rc<RefCell<u64>>,
    suppress_search: Rc<RefCell<bool>>,
    db: Database,
    on_select: Rc<dyn Fn(SearchResult)>,
    near: Rc<RefCell<Option<Near>>>,
    /// A field (start, destination) shows the picked place's name in the entry.
    fills: bool,
    /// The places in the list now, for Enter.
    shown: Rc<RefCell<Vec<SearchResult>>>,
    picked: Rc<Cell<bool>>,
}

impl SearchBar {
    /// The header's search.
    pub fn new(db: Database, on_select: impl Fn(SearchResult) + 'static) -> Self {
        let bar = Self::build(db, "Search places, addresses or coordinates", false, Rc::new(on_select));
        bar.entry.add_css_class("maps-search-entry");
        bar.entry.set_width_chars(16);
        bar.entry.set_max_width_chars(34);
        bar
    }

    /// A start or destination field: picking a place puts its name in the entry.
    pub fn field(db: Database, placeholder: &str, near: Near, on_select: impl Fn(SearchResult) + 'static) -> Self {
        let bar = Self::build(db, placeholder, true, Rc::new(on_select));
        bar.entry.add_css_class("waypoint-entry");
        bar.entry.set_hexpand(true);
        bar.entry.set_width_chars(12);
        bar.set_near(near);
        bar
    }

    fn build(db: Database, placeholder: &str, fills: bool, on_select: Rc<dyn Fn(SearchResult)>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(placeholder));
        entry.set_icon_from_icon_name(gtk::EntryIconPosition::Primary, Some("system-search-symbolic"));
        entry.set_icon_activatable(gtk::EntryIconPosition::Primary, false);
        root.append(&entry);

        let popover = gtk::Popover::new();
        popover.set_parent(&entry);
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_autohide(true);
        popover.set_has_arrow(false);

        let pop_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        pop_box.set_size_request(340, -1);
        pop_box.set_margin_start(4);
        pop_box.set_margin_end(4);
        pop_box.set_margin_top(4);
        pop_box.set_margin_bottom(4);

        let results_list = gtk::ListBox::new();
        results_list.add_css_class("search-results-list");
        results_list.set_selection_mode(gtk::SelectionMode::None);
        pop_box.append(&results_list);
        popover.set_child(Some(&pop_box));

        let search_bar = Self {
            root,
            entry,
            popover,
            results_list,
            current_search_id: Rc::new(RefCell::new(0u64)),
            suppress_search: Rc::new(RefCell::new(false)),
            db,
            on_select,
            near: Rc::new(RefCell::new(None)),
            fills,
            shown: Rc::new(RefCell::new(Vec::new())),
            picked: Rc::new(Cell::new(false)),
        };

        search_bar.setup_signals();
        search_bar
    }

    /// Results nearest to what `near` gives come first.
    pub fn set_near(&self, near: Near) {
        *self.near.borrow_mut() = Some(near);
    }

    fn picker(&self) -> Picker {
        Picker {
            entry: self.entry.clone(),
            popover: self.popover.clone(),
            list: self.results_list.clone(),
            on_select: self.on_select.clone(),
            db: self.db.clone(),
            near: self.near.borrow().as_ref().and_then(|n| n()),
            fills: self.fills,
            suppress: self.suppress_search.clone(),
            shown: self.shown.clone(),
            picked: self.picked.clone(),
        }
    }

    fn setup_signals(&self) {
        let search_id = self.current_search_id.clone();
        let suppress = self.suppress_search.clone();
        let popover = self.popover.clone();
        let near = self.near.clone();
        let picked = self.picked.clone();
        let me = self.clone_parts();

        self.entry.connect_changed(move |e| {
            if *suppress.borrow() {
                *suppress.borrow_mut() = false;
                return;
            }
            picked.set(false);

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
                me.picker(&near).show(vec![place]);
                return;
            }

            let next_id = {
                let mut id = search_id.borrow_mut();
                *id += 1;
                *id
            };
            let id_check = search_id.clone();
            let picker = me.picker(&near);
            glib::timeout_add_local_once(Duration::from_millis(350), move || {
                if *id_check.borrow() != next_id {
                    return;
                }
                picker.search(text, Some((id_check, next_id)));
            });
        });

        // Enter picks the first place in the list.
        let shown = self.shown.clone();
        let picker_parts = self.clone_parts();
        let near = self.near.clone();
        self.entry.connect_activate(move |_| {
            let first = shown.borrow().first().cloned();
            if let Some(place) = first {
                picker_parts.picker(&near).pick(place);
            }
        });
    }

    fn clone_parts(&self) -> Parts {
        Parts {
            entry: self.entry.clone(),
            popover: self.popover.clone(),
            list: self.results_list.clone(),
            on_select: self.on_select.clone(),
            db: self.db.clone(),
            fills: self.fills,
            suppress: self.suppress_search.clone(),
            shown: self.shown.clone(),
            picked: self.picked.clone(),
        }
    }

    pub fn focus(&self) {
        self.entry.grab_focus();
    }

    /// Set the text without searching for it.
    pub fn set_text(&self, text: &str) {
        *self.suppress_search.borrow_mut() = true;
        self.entry.set_text(text);
        if self.entry.text() != text {
            *self.suppress_search.borrow_mut() = false;
        }
    }

    pub fn text(&self) -> String {
        self.entry.text().trim().to_string()
    }

    /// True once a place from the list is in the field (until it's edited).
    pub fn has_pick(&self) -> bool {
        self.picked.get()
    }

    /// Mark the field as holding a known place (set from elsewhere: a place card,
    /// My Location).
    pub fn set_picked(&self, on: bool) {
        self.picked.set(on);
    }

    /// Search for what's typed now and show the list to choose from.
    pub fn search_now(&self) {
        let text = self.text();
        if !text.is_empty() {
            self.entry.grab_focus();
            self.picker().search(text, None);
        }
    }

    pub fn show_results(&self, query_label: &str, places: Vec<SearchResult>) {
        self.set_text(query_label);
        *self.current_search_id.borrow_mut() += 1;
        self.picker().show(places);
    }

    pub fn select_place(&self, place: SearchResult) {
        (self.on_select)(place);
    }
}

#[derive(Clone)]
struct Parts {
    entry: gtk::Entry,
    popover: gtk::Popover,
    list: gtk::ListBox,
    on_select: Rc<dyn Fn(SearchResult)>,
    db: Database,
    fills: bool,
    suppress: Rc<RefCell<bool>>,
    shown: Rc<RefCell<Vec<SearchResult>>>,
    picked: Rc<Cell<bool>>,
}

impl Parts {
    fn picker(&self, near: &Rc<RefCell<Option<Near>>>) -> Picker {
        Picker {
            entry: self.entry.clone(),
            popover: self.popover.clone(),
            list: self.list.clone(),
            on_select: self.on_select.clone(),
            db: self.db.clone(),
            near: near.borrow().as_ref().and_then(|n| n()),
            fills: self.fills,
            suppress: self.suppress.clone(),
            shown: self.shown.clone(),
            picked: self.picked.clone(),
        }
    }
}

/// One search's list: fetching, showing and picking a place.
#[derive(Clone)]
struct Picker {
    entry: gtk::Entry,
    popover: gtk::Popover,
    list: gtk::ListBox,
    on_select: Rc<dyn Fn(SearchResult)>,
    db: Database,
    near: Option<(f64, f64)>,
    fills: bool,
    suppress: Rc<RefCell<bool>>,
    shown: Rc<RefCell<Vec<SearchResult>>>,
    picked: Rc<Cell<bool>>,
}

impl Picker {
    /// Fetch places for `text`; a stale answer (`id` moved on) is dropped.
    fn search(self, text: String, id: Option<(Rc<RefCell<u64>>, u64)>) {
        let api_key = settings::get().geoapify_api_key;
        let near = self.near;
        let (sender, receiver) = async_channel::bounded(1);
        crate::runtime::spawn(async move {
            let _ = sender.send(search_places_near(&text, &api_key, near).await).await;
        });
        glib::spawn_future_local(async move {
            let Ok(result) = receiver.recv().await else { return };
            if let Some((current, mine)) = &id
                && *current.borrow() != *mine
            {
                return;
            }
            match result {
                Ok(places) if !places.is_empty() => self.show(places),
                _ => self.show_empty(),
            }
        });
    }

    fn clear(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
    }

    fn show_empty(&self) {
        self.clear();
        self.shown.borrow_mut().clear();
        let note = gtk::Label::new(Some("No places found. Try another name or an address."));
        note.add_css_class("place-desc");
        note.set_margin_top(8);
        note.set_margin_bottom(8);
        self.list.append(&note);
        self.popover.popup();
    }

    fn show(&self, places: Vec<SearchResult>) {
        self.clear();
        let units = settings::get().units;
        for place in &places {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.add_css_class("search-result-row");

            let icon = gtk::Image::from_icon_name(if place.category.as_deref() == Some("Coordinate") {
                "find-location-symbolic"
            } else {
                "mark-location-symbolic"
            });
            icon.set_pixel_size(16);
            icon.set_valign(gtk::Align::Start);
            icon.set_margin_top(2);
            row.append(&icon);

            let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text_box.set_hexpand(true);
            let name_lbl = gtk::Label::new(Some(&place.name));
            name_lbl.add_css_class("place-title");
            name_lbl.set_xalign(0.0);
            name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text_box.append(&name_lbl);
            let kind = place
                .category
                .as_deref()
                .filter(|c| *c != "Coordinate" && !c.contains('.'))
                .map(|c| {
                    let words = c.replace('_', " ");
                    let mut chars = words.chars();
                    chars.next().map(|f| f.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
                });
            let detail = match kind {
                Some(k) if !place.formatted_address.is_empty() => format!("{k} · {}", place.formatted_address),
                Some(k) => k,
                None => place.formatted_address.clone(),
            };
            let addr_lbl = gtk::Label::new(Some(&detail));
            addr_lbl.add_css_class("place-desc");
            addr_lbl.set_xalign(0.0);
            addr_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text_box.append(&addr_lbl);
            row.append(&text_box);

            if let Some(here) = self.near {
                let away = gtk::Label::new(Some(&format_distance(distance_m(here, (place.lat, place.lon)), &units)));
                away.add_css_class("place-distance");
                away.set_valign(gtk::Align::Start);
                row.append(&away);
            }

            let gesture = gtk::GestureClick::new();
            let (me, pl) = (self.clone(), place.clone());
            gesture.connect_released(move |_, _, _, _| me.pick(pl.clone()));
            row.add_controller(gesture);
            self.list.append(&row);
        }
        *self.shown.borrow_mut() = places;
        self.popover.popup();
    }

    fn pick(&self, place: SearchResult) {
        self.popover.popdown();
        let _ = self.db.add_recent_search(&place.name, &place.formatted_address, place.lat, place.lon);
        if self.fills {
            *self.suppress.borrow_mut() = true;
            self.entry.set_text(&place.name);
            if self.entry.text() != place.name.as_str() {
                *self.suppress.borrow_mut() = false;
            }
            self.entry.set_position(-1);
        }
        self.picked.set(true);
        (self.on_select)(place);
    }
}
