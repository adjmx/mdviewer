//! Light & dark: follow the desktop's colour-scheme preference.
//!
//! Without libadwaita, GTK only goes dark when asked to. GNOME publishes the choice in
//! `org.gnome.desktop.interface color-scheme`; mirroring it into GTK also flips the web
//! view's `prefers-color-scheme`, which the stylesheet keys on.

use gtk4::{self as gtk, gio, prelude::*};

const SCHEMA: &str = "org.gnome.desktop.interface";
const KEY: &str = "color-scheme";

pub fn follow_system() {
    let has_key = gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup(SCHEMA, true))
        .is_some_and(|schema| schema.has_key(KEY));
    if !has_key {
        return;
    }

    let settings = gio::Settings::new(SCHEMA);
    let apply = |settings: &gio::Settings| {
        if let Some(gtk_settings) = gtk::Settings::default() {
            gtk_settings
                .set_gtk_application_prefer_dark_theme(settings.string(KEY) == "prefer-dark");
        }
    };
    apply(&settings);
    settings.connect_changed(Some(KEY), move |settings, _| apply(settings));
    // The handler lives as long as the settings object; keep it for the app's lifetime.
    std::mem::forget(settings);
}
