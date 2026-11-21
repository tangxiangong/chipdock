//! Rust-opened windows in the mixed app are declared under WorkbenchHost.
//! NSH's implicit Rust window host is installed only for a Rust application root.
use super::Store;
use nsh_core::{
    element::desktop::{Window, current_window},
    prelude::*,
};
use std::{cell::RefCell, rc::Rc};

struct Entry {
    id: String,
    window: RefCell<Option<Window>>,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

#[derive(Clone, Copy)]
pub(super) struct WindowQueue(Signal<Vec<Rc<Entry>>>);

impl WindowQueue {
    pub(super) fn new() -> Self {
        Self(signal(Vec::new()))
    }

    pub(super) fn host(self) -> impl IntoElement {
        each(
            self.0,
            |entry| Rc::as_ptr(entry) as usize,
            |entry, _| entry.get_untracked().window.borrow_mut().take(),
        )
    }
}

#[derive(Clone)]
struct Content(Rc<dyn Fn() -> AnyElement>);

// Native descendants created by TSX inherit the nearest native component owner's
// context. Run the factory in this owner so it also supplies window-specific state.
#[component]
fn WindowContent() -> impl IntoElement {
    let content = use_context::<Content>().expect("window content has a factory");
    (content.0)()
}

pub(super) fn open<E: IntoElement + 'static>(
    store: Store,
    id: &str,
    window: Window,
    on_close: impl Fn() + 'static,
    content: impl Fn() -> E + 'static,
) -> bool {
    if !os().is_desktop() {
        return false;
    }
    let queue = store.window_queue.0;
    if queue.with_untracked(|entries| entries.iter().any(|entry| entry.id == id)) {
        if let Some(window) = store.windows.with(|windows| windows.get(id).cloned()) {
            window.focus();
        }
        return true;
    }
    let closed_id = id.to_owned();
    let opened_id = id.to_owned();
    let content = Content(Rc::new(move || content().into_element()));
    let window = window
        .id(id.to_owned())
        .on_open_change(move |event| {
            if !event.open {
                store.windows.with(|windows| windows.remove(&closed_id));
                queue.update(|entries| entries.retain(|entry| entry.id != closed_id));
                on_close();
            }
        })
        .content(move || {
            store
                .windows
                .with(|windows| windows.insert(opened_id.clone(), current_window()));
            provide_context(content.clone());
            window_content()
        });
    queue.update(|entries| {
        entries.push(Rc::new(Entry {
            id: id.to_owned(),
            window: RefCell::new(Some(window)),
        }))
    });
    true
}
