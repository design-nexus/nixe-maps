use crate::settings;
pub use nixe_theme::*;

pub fn install() {
    nixe_theme::install(Style {
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
