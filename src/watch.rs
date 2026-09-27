//! Calls back when the watched file changes on disk.
//!
//! GIO watches the file through its folder, so editors that save atomically (write a
//! temporary file, then rename it over the original) are followed without re-arming.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

/// Coalesces the burst of events a single save produces.
const SETTLE: Duration = Duration::from_millis(150);

pub fn watch(path: &Path, on_change: impl Fn() + 'static) -> Option<gio::FileMonitor> {
    let monitor = gio::File::for_path(path)
        .monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        .ok()?;

    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    let on_change = Rc::new(on_change);
    monitor.connect_changed(move |_, _, _, event| {
        if event == gio::FileMonitorEvent::AttributeChanged {
            return;
        }
        if let Some(source) = pending.borrow_mut().take() {
            source.remove();
        }
        let (pending_inner, on_change) = (pending.clone(), on_change.clone());
        let source = glib::timeout_add_local_once(SETTLE, move || {
            pending_inner.borrow_mut().take();
            on_change();
        });
        *pending.borrow_mut() = Some(source);
    });
    Some(monitor)
}
