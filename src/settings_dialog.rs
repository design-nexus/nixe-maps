use crate::settings;
use crate::storage::Database;
use crate::theme;
use crate::ui::widgets::{self, Page};
use gtk::prelude::*;
use nyx_ui::settings_card::{self, Spec};

const GROUPS: &[(&str, &str, &str)] = &[
    ("This window", "applications-graphics-symbolic", "Theme, glow, motion"),
    ("Providers", "preferences-system-network-symbolic", "Geoapify API key, tile servers"),
    ("Navigation", "emblem-favorite-symbolic", "Units, travel modes"),
    ("Places", "user-home-symbolic", "Home, work, history"),
    ("Keyboard", "input-keyboard-symbolic", "Shortcuts"),
];

const SPEC: Spec = Spec {
    groups: GROUPS,
    search_text: None,
};

pub fn open(overlay: &gtk::Overlay, win: &gtk::ApplicationWindow, db: Database) {
    let db_c = db.clone();
    settings_card::open(overlay, win, SPEC, move || {
        let p = widgets::page("settings");
        build_content(&p, &db_c);
        (p.root.upcast(), p.body)
    });
}

fn build_content(p: &Page, db: &Database) {
    window_group(p);
    providers_group(p);
    navigation_group(p);
    places_group(p, db);
    keyboard_group(p);
}

fn window_group(p: &Page) {
    let group = p.group("This window");
    group.add(&nyx_ui::widgets::start_row());
    let current_prefs = settings::get();

    let options: Vec<(String, String)> = theme::all()
        .iter()
        .map(|t| (t.id.clone(), t.name.clone()))
        .collect();

    let (theme_row, theme_dd) = widgets::choice_row(
        "Theme",
        "A bundled theme, or one of your own.",
        options,
        &current_prefs.theme,
        |id| {
            settings::update(|s| {
                s.theme = id;
                s.mode = settings::ThemeMode::Theme;
            });
            theme::apply();
        },
    );

    theme_dd.set_sensitive(current_prefs.mode == settings::ThemeMode::Theme || !theme::omarchy_available());

    if theme::omarchy_available() {
        let dd = theme_dd.clone();
        let (row, _) = widgets::switch_row(
            "Follow Omarchy theme",
            "Match the desktop's colors and update when the Omarchy theme changes.",
            current_prefs.mode == settings::ThemeMode::Omarchy,
            move |on| {
                settings::update(|s| {
                    s.mode = if on {
                        settings::ThemeMode::Omarchy
                    } else {
                        settings::ThemeMode::Theme
                    };
                });
                dd.set_sensitive(!on);
                theme::apply();
            },
        );
        group.add(&row);
    }
    group.add(&theme_row);

    let (row, _) = widgets::switch_row(
        "Glow",
        "Soft accent glow around focused and selected elements.",
        current_prefs.glow,
        |on| {
            settings::update(|s| s.glow = on);
            theme::apply();
        },
    );
    group.add(&row);

    let (row, _) = widgets::switch_row(
        "Reduce motion",
        "Turn off transitions in this window.",
        current_prefs.reduce_motion,
        |on| {
            settings::update(|s| s.reduce_motion = on);
            theme::apply();
        },
    );
    group.add(&row);
}

fn providers_group(p: &Page) {
    let group = p.group("Providers");
    let current = settings::get();

    // Geoapify API key entry
    let key_entry = gtk::Entry::new();
    key_entry.set_text(&current.geoapify_api_key);
    key_entry.set_placeholder_text(Some("Enter 32-character API key"));
    key_entry.set_width_chars(28);
    key_entry.add_css_class("maps-search-entry");

    key_entry.connect_changed(|e| {
        let text = e.text().trim().to_string();
        settings::update(|s| s.geoapify_api_key = text);
    });

    let row = widgets::row(
        "Geoapify API key",
        "Used for location search and routing (3,000 free credits/day).",
        Some(key_entry.upcast_ref()),
    );
    group.add(&row);

    // Custom Satellite Tile URL
    let sat_entry = gtk::Entry::new();
    sat_entry.set_text(&current.custom_satellite_url);
    sat_entry.set_placeholder_text(Some("https://.../{z}/{x}/{y} (optional)"));
    sat_entry.set_width_chars(28);
    sat_entry.add_css_class("maps-search-entry");

    sat_entry.connect_changed(|e| {
        let text = e.text().trim().to_string();
        settings::update(|s| s.custom_satellite_url = text);
    });

    let row = widgets::row(
        "Custom satellite tiles",
        "Tile URL template. Leave blank to use default Esri World Imagery.",
        Some(sat_entry.upcast_ref()),
    );
    group.add(&row);

    // Custom Topo Tile URL
    let topo_entry = gtk::Entry::new();
    topo_entry.set_text(&current.custom_topo_url);
    topo_entry.set_placeholder_text(Some("https://.../{z}/{x}/{y}.png (optional)"));
    topo_entry.set_width_chars(28);
    topo_entry.add_css_class("maps-search-entry");

    topo_entry.connect_changed(|e| {
        let text = e.text().trim().to_string();
        settings::update(|s| s.custom_topo_url = text);
    });

    let row = widgets::row(
        "Custom topographic tiles",
        "Tile URL template. Leave blank to use default OpenTopoMap.",
        Some(topo_entry.upcast_ref()),
    );
    group.add(&row);
}

fn navigation_group(p: &Page) {
    let group = p.group("Navigation");
    let current = settings::get();

    let unit_options = vec![
        ("imperial".into(), "Imperial (miles, feet)".into()),
        ("metric".into(), "Metric (kilometers, meters)".into()),
    ];

    let (row, _) = widgets::choice_row(
        "Measurement units",
        "Unit system used for distances and route instructions.",
        unit_options,
        &current.units,
        |val| {
            settings::update(|s| s.units = val);
        },
    );
    group.add(&row);
}

fn places_group(p: &Page, db: &Database) {
    let group = p.group("Places");

    let db_c = db.clone();
    let (row, _) = widgets::button_row(
        "Clear recent searches",
        "Remove all past search history from the local database.",
        "Clear",
        move |btn| {
            let _ = db_c.clear_recent_searches();
            btn.set_sensitive(false);
            btn.set_label("Cleared");
        },
    );
    group.add(&row);
}

fn keyboard_group(p: &Page) {
    let group = p.group("Keyboard");
    let shortcuts: &[(&[&str], &str)] = &[
        (&["Ctrl", "F"], "Focus search bar"),
        (&["Ctrl", "B"], "Toggle side panel"),
        (&["Ctrl", "R"], "Open directions"),
        (&["Ctrl", ","], "Open settings"),
        (&["+", "="], "Zoom in"),
        (&["-"], "Zoom out"),
        (&["F1"], "Show keyboard shortcuts"),
        (&["Esc"], "Dismiss dialog, panel, or search results"),
        (&["Ctrl", "Q"], "Quit application"),
    ];

    for (keys, action) in shortcuts {
        let key_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        key_box.set_halign(gtk::Align::End);
        for k in *keys {
            let cap = gtk::Label::new(Some(k));
            cap.add_css_class("key-cap");
            key_box.append(&cap);
        }
        let r = widgets::row(action, "", Some(key_box.upcast_ref()));
        group.add(&r);
    }
}
