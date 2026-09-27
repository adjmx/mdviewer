//! mdviewer — a tiny Markdown reader for Linux. GTK4 + WebKitGTK; documents never touch
//! the network.

mod nav;
mod offline;
mod render;
mod scheme;
mod theme;
mod update;
mod watch;
mod window;

use gtk4::{self as gtk, gio, glib, prelude::*};

/// Debug builds run as a separate app, so a dev run never hands its files to (or takes
/// them from) an installed, already-running mdviewer.
pub const APP_ID: &str = if cfg!(debug_assertions) {
    "uk.fizx.mdviewer.dev"
} else {
    "uk.fizx.mdviewer"
};
pub const DATA_DIR: &str = if cfg!(debug_assertions) {
    "mdviewer-dev"
} else {
    "mdviewer"
};
pub const UPDATE_REPOSITORY: &str = "adjmx/mdviewer";
const ICON_NAME: &str = "mdviewer";

fn main() -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|app| {
        gtk::Window::set_default_icon_name(ICON_NAME);
        theme::follow_system();
        register_scheme();
        add_actions(app);
    });
    app.connect_activate(window::activate);
    app.connect_open(|app, files, _| {
        for file in files {
            window::open(app, file);
        }
    });
    app.run()
}

fn register_scheme() {
    let Some(context) = webkit6::WebContext::default() else {
        return;
    };
    context.register_uri_scheme(scheme::SCHEME, |request| {
        let root = request
            .web_view()
            .and_then(|webview| window::root_for(&webview));
        scheme::serve(request, root.as_deref());
    });
}

fn add_actions(app: &gtk::Application) {
    let add = |name: &str, run: fn(&gtk::Application)| {
        let action = gio::SimpleAction::new(name, None);
        let app_weak = app.downgrade();
        action.connect_activate(move |_, _| {
            if let Some(app) = app_weak.upgrade() {
                run(&app);
            }
        });
        app.add_action(&action);
    };
    add("open", window::choose_and_open);
    add("check-updates", |app| update::check(app.active_window()));
    add("about", about);
    add("quit", |app| app.quit());

    for (action, accels) in [
        ("app.open", &["<Control>o"][..]),
        ("app.quit", &["<Control>q"]),
        ("win.reload", &["<Control>r", "F5"]),
        ("win.print", &["<Control>p"]),
        (
            "win.zoom-in",
            &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
        ),
        ("win.zoom-out", &["<Control>minus", "<Control>KP_Subtract"]),
        ("win.zoom-reset", &["<Control>0", "<Control>KP_0"]),
        ("window.close", &["<Control>w"]),
    ] {
        app.set_accels_for_action(action, accels);
    }
}

fn about(app: &gtk::Application) {
    let dialog = gtk::AboutDialog::builder()
        .program_name("mdviewer")
        .version(env!("CARGO_PKG_VERSION"))
        .comments("A tiny Markdown reader. Documents never touch the network.")
        .logo_icon_name(ICON_NAME)
        .website(format!("https://github.com/{UPDATE_REPOSITORY}"))
        .license(
            "No license has been chosen yet, so all rights are reserved for now.\n\n\
             highlight.js, bundled with the app, is under its own BSD-3-Clause license.",
        )
        .modal(true)
        .build();
    dialog.set_transient_for(app.active_window().as_ref());
    dialog.present();
}
