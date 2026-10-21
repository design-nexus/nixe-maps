use crate::providers::SearchResult;
use crate::storage::{Database, FavoritePlace};
use gtk::prelude::*;
use nixe_ui::widgets;
use std::cell::RefCell;
use std::rc::Rc;

pub struct PlaceCard {
    pub root: gtk::Box,
    title_label: gtk::Label,
    address_label: gtk::Label,
    coords_label: gtk::Label,
    category_label: gtk::Label,
    fav_btn: gtk::Button,
    #[allow(dead_code)]
    directions_btn: gtk::Button,
    current_place: Rc<RefCell<Option<SearchResult>>>,
}

impl PlaceCard {
    pub fn new(
        db: Database,
        on_directions: impl Fn(SearchResult) + 'static,
        on_close: impl Fn() + 'static,
    ) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.add_css_class("place-card");

        // Header with title and close button
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title_label = gtk::Label::new(None);
        title_label.add_css_class("place-title");
        title_label.set_hexpand(true);
        title_label.set_xalign(0.0);
        title_label.set_wrap(true);
        header.append(&title_label);

        let close_btn = widgets::bar_button("window-close-symbolic", "Close place card");
        close_btn.connect_clicked(move |_| on_close());
        header.append(&close_btn);
        root.append(&header);

        // Category tag
        let category_label = gtk::Label::new(None);
        category_label.add_css_class("place-desc");
        category_label.set_xalign(0.0);
        root.append(&category_label);

        // Address
        let address_label = gtk::Label::new(None);
        address_label.add_css_class("place-desc");
        address_label.set_xalign(0.0);
        address_label.set_wrap(true);
        root.append(&address_label);

        // Coordinates
        let coords_label = gtk::Label::new(None);
        coords_label.add_css_class("dim");
        coords_label.set_xalign(0.0);
        root.append(&coords_label);

        // Actions row
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(8);

        let directions_btn = gtk::Button::with_label("Directions");
        directions_btn.add_css_class("suggested-action");
        actions.append(&directions_btn);

        let fav_btn = gtk::Button::with_label("★ Save");
        fav_btn.add_css_class("flat");
        actions.append(&fav_btn);

        root.append(&actions);

        let current_place: Rc<RefCell<Option<SearchResult>>> = Rc::new(RefCell::new(None));
        let cp_dir = current_place.clone();
        directions_btn.connect_clicked(move |_| {
            if let Some(ref p) = *cp_dir.borrow() {
                on_directions(p.clone());
            }
        });

        let cp_fav = current_place.clone();
        let db_clone = db.clone();
        let fb_clone = fav_btn.clone();
        fav_btn.connect_clicked(move |_| {
            if let Some(ref p) = *cp_fav.borrow() {
                let _ = db_clone.add_favorite(&p.name, "favorite", p.lat, p.lon, &p.formatted_address);
                fb_clone.set_label("★ Saved");
                fb_clone.set_sensitive(false);
            }
        });

        Self {
            root,
            title_label,
            address_label,
            coords_label,
            category_label,
            fav_btn,
            directions_btn,
            current_place,
        }
    }

    pub fn set_place(&self, place: &SearchResult, is_favorite: bool) {
        *self.current_place.borrow_mut() = Some(place.clone());
        self.title_label.set_text(&place.name);
        self.address_label.set_text(&place.formatted_address);
        self.coords_label.set_text(&format!("{:.5}, {:.5}", place.lat, place.lon));

        if let Some(ref cat) = place.category {
            self.category_label.set_text(cat);
            self.category_label.set_visible(true);
        } else {
            self.category_label.set_visible(false);
        }

        if is_favorite {
            self.fav_btn.set_label("★ Saved");
            self.fav_btn.set_sensitive(false);
        } else {
            self.fav_btn.set_label("★ Save");
            self.fav_btn.set_sensitive(true);
        }

        self.root.set_visible(true);
    }

    #[allow(dead_code)]
    pub fn hide(&self) {
        self.root.set_visible(false);
        *self.current_place.borrow_mut() = None;
    }
}

pub struct SavedPlacesView {
    pub root: gtk::Box,
    list_box: gtk::ListBox,
    db: Database,
}

impl SavedPlacesView {
    pub fn new(db: Database, on_select: impl Fn(FavoritePlace) + 'static) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.set_hexpand(true);

        let title = gtk::Label::new(Some("SAVED PLACES"));
        title.add_css_class("group-title");
        title.set_xalign(0.0);
        title.set_margin_bottom(4);
        root.append(&title);

        let list_box = gtk::ListBox::new();
        list_box.add_css_class("group-list");
        root.append(&list_box);

        let db_c = db.clone();
        let view = Self {
            root,
            list_box,
            db: db_c,
        };

        view.refresh(on_select);
        view
    }

    pub fn refresh(&self, on_select: impl Fn(FavoritePlace) + 'static) {
        // Clear children
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        let on_select = Rc::new(on_select);
        if let Ok(places) = self.db.list_favorites() {
            if places.is_empty() {
                let empty = gtk::Label::new(Some("No saved places yet."));
                empty.add_css_class("dim");
                empty.set_margin_top(12);
                empty.set_margin_bottom(12);
                self.list_box.append(&empty);
                return;
            }

            for place in places {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row.add_css_class("search-result-row");

                let icon_name = match place.label.as_str() {
                    "home" => "user-home-symbolic",
                    "work" => "applications-office-symbolic",
                    _ => "starred-symbolic",
                };

                let icon = gtk::Image::from_icon_name(icon_name);
                icon.set_pixel_size(16);
                row.append(&icon);

                let info_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
                info_box.set_hexpand(true);

                let name_lbl = gtk::Label::new(Some(&place.name));
                name_lbl.add_css_class("place-title");
                name_lbl.set_xalign(0.0);
                info_box.append(&name_lbl);

                let addr_lbl = gtk::Label::new(Some(&place.address));
                addr_lbl.add_css_class("place-desc");
                addr_lbl.set_xalign(0.0);
                addr_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                info_box.append(&addr_lbl);

                row.append(&info_box);

                let pl_clone = place.clone();
                let os_clone = on_select.clone();
                let gesture = gtk::GestureClick::new();
                gesture.connect_released(move |_, _, _, _| {
                    os_clone(pl_clone.clone());
                });
                row.add_controller(gesture);

                self.list_box.append(&row);
            }
        }
    }
}
