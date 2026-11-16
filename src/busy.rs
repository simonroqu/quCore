//! Lade-Kreis in der Nav-Bubble (Eintrag `busy` in `qu.toml`): dreht sich, solange ein `Busy` lebt.

use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::{Cell, OnceCell};

thread_local! {
    static COUNT: Cell<u32> = const { Cell::new(0) };
    static ICON: OnceCell<gtk::Image> = const { OnceCell::new() };
}

/// Für jedes Laden (Seiten, Nachladen, Stream) eins anlegen und in den Ladevorgang mitnehmen.
pub struct Busy(());

impl Busy {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        COUNT.with(|c| c.set(c.get() + 1));
        update();
        Busy(())
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        COUNT.with(|c| c.set(c.get().saturating_sub(1)));
        update();
    }
}

pub(crate) fn set_icon(icon: &gtk::Image) {
    ICON.with(|i| i.set(icon.clone()).ok());
}

fn update() {
    let on = COUNT.with(|c| c.get()) > 0;
    ICON.with(|i| {
        if let Some(i) = i.get() {
            i.set_visible(on);
        }
    });
}
