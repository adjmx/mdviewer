//! One window per document.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use gtk4::{self as gtk, gdk, gio, glib, prelude::*};
use webkit6::prelude::*;

use crate::nav::{self, Decision};
use crate::{offline, render, scheme, watch};

/// The script world the app's own scripts run in. Document scripts never run at all.
const WORLD: &str = "mdviewer";

const ZOOM_STEP: f64 = 0.1;
const ZOOM_RANGE: (f64, f64) = (0.3, 3.0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rules {
    Pending,
    Installed,
    Failed,
}

pub struct Doc {
    window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    webview: webkit6::WebView,
    path: RefCell<Option<PathBuf>>,
    text: RefCell<String>,
    /// What the web view currently shows, and the folder it was loaded from.
    loaded_body: RefCell<Option<String>>,
    loaded_root: RefCell<Option<PathBuf>>,
    is_loaded: Cell<bool>,
    rules: Cell<Rules>,
    monitor: RefCell<Option<gio::FileMonitor>>,
}

thread_local! {
    static DOCS: RefCell<Vec<Rc<Doc>>> = const { RefCell::new(Vec::new()) };
}

/// Opens `file`: brings its window forward if it's already open, fills the active window if
/// that one is still empty, and otherwise opens a new window.
pub fn open(app: &gtk::Application, file: &gio::File) {
    let Some(path) = file.path() else { return };
    let path = path.canonicalize().unwrap_or(path);

    if let Some(doc) = find(|doc| doc.path.borrow().as_deref() == Some(&path)) {
        return doc.window.present();
    }
    let active = app.active_window();
    let doc =
        find(|doc| doc.path.borrow().is_none() && Some(doc.window.upcast_ref()) == active.as_ref())
            .unwrap_or_else(|| Doc::new(app));
    doc.window.present();
    if let Err(error) = doc.load(&path) {
        alert(
            &doc.window,
            "Couldn’t Open the Document",
            &format!("{}: {error}", path.display()),
        );
    }
}

/// Opens a window with no document, or brings an existing window forward.
pub fn activate(app: &gtk::Application) {
    match app
        .active_window()
        .or_else(|| app.windows().into_iter().next())
    {
        Some(window) => window.present(),
        None => Doc::new(app).window.present(),
    }
}

/// Shows the Open dialog, then opens what was chosen.
pub fn choose_and_open(app: &gtk::Application) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Markdown"));
    filter.add_mime_type("text/markdown");
    filter.add_mime_type("text/x-markdown");
    for ext in ["md", "markdown", "mdown", "mkd", "mkdn"] {
        filter.add_suffix(ext);
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);

    let dialog = gtk::FileDialog::builder()
        .title("Open")
        .filters(&filters)
        .default_filter(&filter)
        .build();
    let app = app.clone();
    let parent = app.active_window();
    dialog.open_multiple(parent.as_ref(), gio::Cancellable::NONE, move |result| {
        let Ok(files) = result else { return };
        for file in files.iter::<gio::File>().flatten() {
            open(&app, &file);
        }
    });
}

/// The folder the web view's page was loaded from, for serving relative files.
pub fn root_for(webview: &webkit6::WebView) -> Option<PathBuf> {
    find(|doc| &doc.webview == webview).and_then(|doc| doc.loaded_root.borrow().clone())
}

fn find(pred: impl Fn(&Doc) -> bool) -> Option<Rc<Doc>> {
    DOCS.with_borrow(|docs| docs.iter().find(|doc| pred(doc)).cloned())
}

impl Doc {
    fn new(app: &gtk::Application) -> Rc<Doc> {
        let content = webkit6::UserContentManager::new();
        for source in [
            include_str!("../resources/highlight.min.js"),
            include_str!("../resources/viewer.js"),
        ] {
            content.add_script(&webkit6::UserScript::for_world(
                source,
                webkit6::UserContentInjectedFrames::TopFrame,
                webkit6::UserScriptInjectionTime::End,
                WORLD,
                &[],
                &[],
            ));
        }
        content.register_script_message_handler("copyCode", Some(WORLD));

        let webview = webkit6::WebView::builder()
            .user_content_manager(&content)
            .network_session(&webkit6::NetworkSession::new_ephemeral())
            .build();
        if let Some(settings) = WebViewExt::settings(&webview) {
            // Markdown can embed raw HTML; never run scripts from documents. The app's own
            // scripts run in their own world, unaffected by this.
            settings.set_enable_javascript_markup(false);
            settings.set_javascript_can_open_windows_automatically(false);
            settings.set_enable_developer_extras(cfg!(debug_assertions));
            settings.set_enable_webgl(false);
            settings.set_enable_media_stream(false);
            settings.set_enable_back_forward_navigation_gestures(false);
        }

        let stack = gtk::Stack::new();
        stack.add_named(&empty_page(), Some("empty"));
        stack.add_named(&webview, Some("doc"));
        stack.add_named(&blocked_page(), Some("blocked"));
        stack.set_visible_child_name("empty");

        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("mdviewer")
            .default_width(820)
            .default_height(900)
            .child(&stack)
            .build();
        window.set_titlebar(Some(&header_bar()));

        let doc = Rc::new(Doc {
            window,
            stack,
            webview,
            path: RefCell::default(),
            text: RefCell::default(),
            loaded_body: RefCell::default(),
            loaded_root: RefCell::default(),
            is_loaded: Cell::new(false),
            rules: Cell::new(Rules::Pending),
            monitor: RefCell::default(),
        });
        DOCS.with_borrow_mut(|docs| docs.push(doc.clone()));

        doc.connect_signals(app, &content);
        doc.add_actions();

        // Nothing is shown until the offline rules are installed (fails closed).
        let weak = Rc::downgrade(&doc);
        offline::load(move |filter| {
            let Some(doc) = weak.upgrade() else { return };
            match filter {
                Some(filter) => {
                    content.add_filter(&filter);
                    doc.rules.set(Rules::Installed);
                }
                None => doc.rules.set(Rules::Failed),
            }
            doc.show();
        });
        doc
    }

    fn connect_signals(
        self: &Rc<Self>,
        app: &gtk::Application,
        content: &webkit6::UserContentManager,
    ) {
        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            if let Some(doc) = weak.upgrade() {
                doc.monitor.borrow_mut().take();
                DOCS.with_borrow_mut(|docs| docs.retain(|d| !Rc::ptr_eq(d, &doc)));
            }
            glib::Propagation::Proceed
        });

        let window = self.window.clone();
        content.connect_script_message_received(Some("copyCode"), move |_, value| {
            window.clipboard().set_text(&value.to_str());
        });

        let weak = Rc::downgrade(self);
        self.webview.connect_load_changed(move |_, event| {
            if event == webkit6::LoadEvent::Finished
                && let Some(doc) = weak.upgrade()
            {
                doc.is_loaded.set(true);
            }
        });

        let window = self.window.clone();
        self.webview
            .connect_decide_policy(move |webview, decision, kind| {
                use webkit6::PolicyDecisionType::*;
                if !matches!(kind, NavigationAction | NewWindowAction) {
                    return false;
                }
                let Some(action) = decision
                    .downcast_ref::<webkit6::NavigationPolicyDecision>()
                    .and_then(|decision| decision.navigation_action())
                else {
                    decision.ignore();
                    return true;
                };
                let uri = action
                    .request()
                    .and_then(|request| request.uri())
                    .unwrap_or_default();
                let clicked = action.navigation_type() == webkit6::NavigationType::LinkClicked
                    || (kind == NewWindowAction && action.is_user_gesture());
                let current = webview.uri();
                let current = if kind == NewWindowAction {
                    None
                } else {
                    current.as_deref()
                };

                match nav::decide(&uri, clicked, current) {
                    Decision::Allow => decision.use_(),
                    Decision::Block => decision.ignore(),
                    Decision::OpenExternally(uri) => {
                        decision.ignore();
                        gtk::UriLauncher::new(&uri).launch(
                            Some(&window),
                            gio::Cancellable::NONE,
                            |_| {},
                        );
                    }
                }
                true
            });

        // A short context menu: copying only. Nothing that navigates, downloads or inspects.
        self.webview.connect_context_menu(|_, menu, _| {
            use webkit6::ContextMenuAction::*;
            for item in menu.items() {
                if !matches!(
                    item.stock_action(),
                    Copy | CopyLinkToClipboard
                        | CopyImageToClipboard
                        | CopyImageUrlToClipboard
                        | SelectAll
                ) {
                    menu.remove(&item);
                }
            }
            menu.n_items() == 0
        });

        // Ctrl + scroll zooms, as in a browser.
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        scroll.connect_scroll(move |controller, _, dy| {
            if !controller
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            if let Some(doc) = weak.upgrade() {
                doc.zoom(if dy < 0.0 { ZOOM_STEP } else { -ZOOM_STEP });
            }
            glib::Propagation::Stop
        });
        self.webview.add_controller(scroll);

        // Drop Markdown files anywhere in the window to open them.
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        drop.set_propagation_phase(gtk::PropagationPhase::Capture);
        let app = app.clone();
        drop.connect_drop(move |_, value, _, _| {
            let Ok(files) = value.get::<gdk::FileList>() else {
                return false;
            };
            for file in files.files() {
                open(&app, &file);
            }
            true
        });
        self.stack.add_controller(drop);
    }

    fn add_actions(self: &Rc<Self>) {
        let action = |name: &str, run: fn(&Rc<Doc>)| {
            let weak: Weak<Doc> = Rc::downgrade(self);
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(move |_, _| {
                if let Some(doc) = weak.upgrade() {
                    run(&doc);
                }
            });
            self.window.add_action(&action);
            action
        };
        action("reload", |doc| doc.reload(true)).set_enabled(false);
        action("print", |doc| {
            if doc.path.borrow().is_some() {
                webkit6::PrintOperation::new(&doc.webview).run_dialog(Some(&doc.window));
            }
        });
        action("zoom-in", |doc| doc.zoom(ZOOM_STEP));
        action("zoom-out", |doc| doc.zoom(-ZOOM_STEP));
        action("zoom-reset", |doc| doc.webview.set_zoom_level(1.0));
    }

    fn load(self: &Rc<Self>, path: &Path) -> std::io::Result<()> {
        let text = read(path)?;
        *self.path.borrow_mut() = Some(path.to_path_buf());
        *self.text.borrow_mut() = text;

        let name = path
            .file_name()
            .map_or_else(|| "Untitled".into(), |n| n.to_string_lossy().into_owned());
        self.window.set_title(Some(&name));
        if let Some(reload) = self.window.lookup_action("reload") {
            reload
                .downcast::<gio::SimpleAction>()
                .unwrap()
                .set_enabled(true);
        }

        // Auto-refresh whenever the file changes on disk.
        let weak = Rc::downgrade(self);
        *self.monitor.borrow_mut() = watch::watch(path, move || {
            if let Some(doc) = weak.upgrade() {
                doc.reload(false);
            }
        });

        self.show();
        Ok(())
    }

    /// Re-reads the file from disk, e.g. after it was changed in another editor.
    fn reload(self: &Rc<Self>, report_errors: bool) {
        let Some(path) = self.path.borrow().clone() else {
            return;
        };
        match read(&path) {
            Ok(text) if text != *self.text.borrow() => {
                *self.text.borrow_mut() = text;
                self.show();
            }
            Ok(_) => {}
            Err(error) if report_errors => {
                alert(&self.window, "Couldn’t Reload", &error.to_string())
            }
            Err(_) => {}
        }
    }

    fn show(self: &Rc<Self>) {
        let Some(path) = self.path.borrow().clone() else {
            return;
        };
        match self.rules.get() {
            Rules::Pending => return,
            Rules::Failed => return self.stack.set_visible_child_name("blocked"),
            Rules::Installed => {}
        }

        let body = render::html(&self.text.borrow());
        let root = path.parent().map(Path::to_path_buf);
        let same_root = *self.loaded_root.borrow() == root;
        if same_root && self.loaded_body.borrow().as_deref() == Some(body.as_str()) {
            return;
        }
        let in_place = same_root && self.is_loaded.get();
        *self.loaded_body.borrow_mut() = Some(body.clone());
        *self.loaded_root.borrow_mut() = root;
        self.stack.set_visible_child_name("doc");

        if !in_place {
            return self.load_page();
        }
        // Swap the content without reloading, so the scroll position is kept and nothing flashes.
        let args = glib::VariantDict::new(None);
        args.insert("html", &body);
        let weak = Rc::downgrade(self);
        self.webview.call_async_javascript_function(
            "mdviewer.update(html)",
            Some(&args.end()),
            Some(WORLD),
            None,
            gio::Cancellable::NONE,
            move |result| {
                if result.is_err()
                    && let Some(doc) = weak.upgrade()
                {
                    doc.load_page();
                }
            },
        );
    }

    fn load_page(&self) {
        let body = self.loaded_body.borrow().clone().unwrap_or_default();
        let Some(root) = self.loaded_root.borrow().clone() else {
            return;
        };
        let title = self.window.title().unwrap_or_default();
        self.is_loaded.set(false);
        self.webview
            .load_html(&render::page(&body, &title), Some(&scheme::base_uri(&root)));
    }

    fn zoom(&self, delta: f64) {
        let level = (self.webview.zoom_level() + delta).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
        self.webview.set_zoom_level((level * 10.0).round() / 10.0);
    }
}

fn read(path: &Path) -> std::io::Result<String> {
    std::fs::read(path).map(|data| String::from_utf8_lossy(&data).into_owned())
}

fn header_bar() -> gtk::HeaderBar {
    let bar = gtk::HeaderBar::new();

    let open = gtk::Button::from_icon_name("document-open-symbolic");
    open.set_action_name(Some("app.open"));
    open.set_tooltip_text(Some("Open (Ctrl+O)"));
    bar.pack_start(&open);

    let menu = gio::Menu::new();
    let section = |items: &[(&str, &str)]| {
        let section = gio::Menu::new();
        for (label, action) in items {
            section.append(Some(label), Some(action));
        }
        menu.append_section(None, &section);
    };
    section(&[("Open…", "app.open"), ("Reload", "win.reload")]);
    section(&[
        ("Zoom In", "win.zoom-in"),
        ("Zoom Out", "win.zoom-out"),
        ("Actual Size", "win.zoom-reset"),
    ]);
    section(&[("Print…", "win.print")]);
    section(&[
        ("Check for Updates…", "app.check-updates"),
        ("About mdviewer", "app.about"),
    ]);

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .build();
    menu_button.set_tooltip_text(Some("Menu"));
    bar.pack_end(&menu_button);

    let reload = gtk::Button::from_icon_name("view-refresh-symbolic");
    reload.set_action_name(Some("win.reload"));
    reload.set_tooltip_text(Some("Reload from disk (Ctrl+R)"));
    bar.pack_end(&reload);

    bar
}

fn empty_page() -> gtk::Box {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .build();
    let icon = gtk::Image::from_icon_name("text-x-generic-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("dim-label");
    let label = gtk::Label::new(Some("Open a Markdown file, or drop one here"));
    label.add_css_class("dim-label");
    let button = gtk::Button::with_label("Open…");
    button.set_action_name(Some("app.open"));
    button.set_halign(gtk::Align::Center);
    button.add_css_class("suggested-action");
    page.append(&icon);
    page.append(&label);
    page.append(&button);
    page
}

fn blocked_page() -> gtk::Label {
    let label = gtk::Label::new(Some(
        "Couldn’t enable offline protection, so the document wasn’t displayed.",
    ));
    label.set_wrap(true);
    label.set_margin_start(24);
    label.set_margin_end(24);
    label
}

pub fn alert(parent: &impl IsA<gtk::Window>, message: &str, detail: &str) {
    gtk::AlertDialog::builder()
        .message(message)
        .detail(detail)
        .modal(true)
        .build()
        .show(Some(parent));
}
