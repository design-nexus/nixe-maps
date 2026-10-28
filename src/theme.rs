use crate::settings;
pub use nyx_theme::*;

pub fn install_icons() {
    let dir = nyx_core::paths::cache_home().join("nyx-maps/icons");
    let _ = std::fs::create_dir_all(&dir);
    const ICONS: &[(&str, &str)] = &[
        ("directions-symbolic", include_str!("../data/icons/directions-symbolic.svg")),
        ("route-symbolic", include_str!("../data/icons/route-symbolic.svg")),
        ("drive-symbolic", include_str!("../data/icons/drive-symbolic.svg")),
        ("walk-symbolic", include_str!("../data/icons/walk-symbolic.svg")),
        ("bicycle-symbolic", include_str!("../data/icons/bicycle-symbolic.svg")),
        ("work-symbolic", include_str!("../data/icons/work-symbolic.svg")),
        ("layers-symbolic", include_str!("../data/icons/layers-symbolic.svg")),
    ];
    for (name, svg) in ICONS {
        let path = dir.join(format!("{name}.svg"));
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*svg) {
            let _ = std::fs::write(&path, svg);
        }
    }
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_search_path(&dir);
    }
}

pub fn install() {
    install_icons();
    nyx_theme::use_design_2();

    nyx_theme::install(Style {
        window_class: "maps-window",
        base_skip: &[],
        css: include_str!("style.css"),
        no_glow_css: "",
        extra_colors: None,
        prefs: || {
            let p = settings::get();
            ThemePrefs {
                mode: p.mode,
                theme: p.theme,
                glow: p.glow,
                reduce_motion: p.reduce_motion,
            }
        },
        set_theme: Some(|mode, theme| {
            settings::update(|p| {
                p.mode = mode;
                p.theme = theme;
            });
        }),
    });
}
