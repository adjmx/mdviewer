//! WebKit content-blocking rules that block every load except local content, so documents
//! can never reach the network (even though the app itself may check GitHub for updates).
//!
//! Same rule format as Safari content blockers, and the same four rules as the macOS app.

use std::cell::RefCell;

use gtk4::glib;
use webkit6::UserContentFilter;

const RULES: &str = r#"[
  { "trigger": { "url-filter": ".*" }, "action": { "type": "block" } },
  { "trigger": { "url-filter": "^mdviewer-file:" }, "action": { "type": "ignore-previous-rules" } },
  { "trigger": { "url-filter": "^data:" }, "action": { "type": "ignore-previous-rules" } },
  { "trigger": { "url-filter": "^about:" }, "action": { "type": "ignore-previous-rules" } }
]"#;

type Waiter = Box<dyn FnOnce(Option<UserContentFilter>)>;

enum State {
    Idle,
    Compiling(Vec<Waiter>),
    Done(Option<UserContentFilter>),
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::Idle) };
}

/// Calls `completion` with the compiled rules, or `None` if they couldn't be compiled.
pub fn load(completion: impl FnOnce(Option<UserContentFilter>) + 'static) {
    let start = STATE.with_borrow_mut(|state| match state {
        State::Done(filter) => {
            let filter = filter.clone();
            glib::idle_add_local_once(move || completion(filter));
            false
        }
        State::Compiling(waiters) => {
            waiters.push(Box::new(completion));
            false
        }
        State::Idle => {
            *state = State::Compiling(vec![Box::new(completion)]);
            true
        }
    });
    if !start {
        return;
    }

    let dir = glib::user_cache_dir()
        .join(crate::DATA_DIR)
        .join("content-filters");
    let store = webkit6::UserContentFilterStore::new(&dir.to_string_lossy());
    store.save(
        "mdviewer-offline",
        &glib::Bytes::from_static(RULES.as_bytes()),
        gtk4::gio::Cancellable::NONE,
        |result| {
            if let Err(error) = &result {
                eprintln!("mdviewer: couldn't compile the offline rules: {error}");
            }
            let filter = result.ok();
            let waiters = STATE.with_borrow_mut(|state| {
                match std::mem::replace(state, State::Done(filter.clone())) {
                    State::Compiling(waiters) => waiters,
                    _ => Vec::new(),
                }
            });
            for waiter in waiters {
                waiter(filter.clone());
            }
        },
    );
}
