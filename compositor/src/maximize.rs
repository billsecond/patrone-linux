//! Maximized and fullscreen windows. Both fill the output; restoring puts a
//! window back where it was, at its old size.

use std::cell::Cell;

use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::state::Patrone;

/// What a window filling the output goes back to.
#[derive(Default)]
struct Saved {
    /// Where it was, and its size, before it filled the output.
    place: Cell<Option<Rectangle<i32, Logical>>>,
    /// Whether it was maximized before it went fullscreen.
    maximized: Cell<bool>,
    /// Restored without an old place (it opened maximized), so it's placed
    /// like a new window once it has drawn at its own size.
    place_when_drawn: Cell<bool>,
}

fn saved(window: &Window) -> &Saved {
    window.user_data().insert_if_missing(Saved::default);
    window.user_data().get::<Saved>().expect("just inserted")
}

fn has(toplevel: &ToplevelSurface, state: State) -> bool {
    toplevel.with_pending_state(|pending| pending.states.contains(state))
}

/// Whether the toplevel is maximized or fullscreen, or about to be.
pub fn fills_output(toplevel: &ToplevelSurface) -> bool {
    has(toplevel, State::Maximized) || has(toplevel, State::Fullscreen)
}

impl Patrone {
    pub fn maximize(&mut self, toplevel: &ToplevelSurface) {
        self.fill(toplevel, State::Maximized);
    }

    pub fn unmaximize(&mut self, toplevel: &ToplevelSurface) {
        if has(toplevel, State::Maximized) {
            self.restore(toplevel);
        }
    }

    pub fn fullscreen(&mut self, toplevel: &ToplevelSurface) {
        let was_maximized = has(toplevel, State::Maximized);
        self.fill(toplevel, State::Fullscreen);
        if let Some(window) = self.window_for(toplevel.wl_surface()) {
            saved(&window).maximized.set(was_maximized);
        }
    }

    /// Leaving fullscreen goes back to maximized if the window was before.
    pub fn unfullscreen(&mut self, toplevel: &ToplevelSurface) {
        if !has(toplevel, State::Fullscreen) {
            return;
        }
        let was_maximized = self
            .window_for(toplevel.wl_surface())
            .is_some_and(|window| saved(&window).maximized.replace(false));
        if was_maximized {
            self.fill(toplevel, State::Maximized);
        } else {
            self.restore(toplevel);
        }
    }

    pub fn toggle_maximized(&mut self, toplevel: &ToplevelSurface) {
        if has(toplevel, State::Maximized) {
            self.unmaximize(toplevel);
        } else {
            self.maximize(toplevel);
        }
    }

    pub fn toggle_fullscreen(&mut self, toplevel: &ToplevelSurface) {
        if has(toplevel, State::Fullscreen) {
            self.unfullscreen(toplevel);
        } else {
            self.fullscreen(toplevel);
        }
    }

    /// Dragging a maximized window's title bar restores it first, keeping the
    /// pointer at the same place across the title bar.
    pub fn restore_under_pointer(&mut self, window: &Window, pointer: Point<f64, Logical>) {
        let (Some(toplevel), Some(output)) = (window.toplevel(), self.output_geometry()) else {
            return;
        };
        let place = saved(window).place.get();
        self.restore(toplevel);
        if let Some(place) = place {
            let across = ((pointer.x - output.loc.x as f64) / output.size.w as f64).clamp(0.0, 1.0);
            let x = (pointer.x - across * place.size.w as f64).round() as i32;
            self.space
                .map_element(window.clone(), (x, output.loc.y), true);
        }
    }

    /// Called on every commit: places a restored window that had no old
    /// place, once it no longer fills the output.
    pub fn place_if_restored(&mut self, window: &Window) {
        let Some(toplevel) = window.toplevel() else {
            return;
        };
        let saved = saved(window);
        let output_size = self.output_geometry().unwrap_or_default().size;
        if !saved.place_when_drawn.get()
            || fills_output(toplevel)
            || window.geometry().size == output_size
        {
            return;
        }
        saved.place_when_drawn.set(false);
        let location = self.placement(window);
        self.space.map_element(window.clone(), location, false);
        self.redraw_needed = true;
    }

    /// Keeps maximized and fullscreen windows the size of the output when
    /// the output changes size.
    pub fn output_resized(&mut self) {
        let Some(output) = self.output_geometry() else {
            return;
        };
        let filling: Vec<Window> = self
            .space
            .elements()
            .filter(|window| window.toplevel().is_some_and(fills_output))
            .cloned()
            .collect();
        for window in filling {
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|pending| pending.size = Some(output.size));
                toplevel.send_pending_configure();
            }
            self.space.map_element(window, output.loc, false);
        }
    }

    /// Fills the output with the toplevel. One that isn't mapped yet gets the
    /// state in its first configure, and is placed when it maps.
    fn fill(&mut self, toplevel: &ToplevelSurface, state: State) {
        let Some(output) = self.output_geometry() else {
            return;
        };
        if let Some(window) = self.window_for(toplevel.wl_surface()) {
            if !fills_output(toplevel)
                && let Some(location) = self.space.element_location(&window)
            {
                let place = Rectangle::new(location, window.geometry().size);
                saved(&window).place.set(Some(place));
            }
            self.space.map_element(window, output.loc, true);
        }
        toplevel.with_pending_state(|pending| {
            pending.states.unset(State::Maximized);
            pending.states.unset(State::Fullscreen);
            pending.states.set(state);
            pending.size = Some(output.size);
        });
        // The protocol wants an answer even when nothing changes.
        if toplevel.is_initial_configure_sent() {
            toplevel.send_configure();
        }
        self.redraw_needed = true;
    }

    /// Back to the size and place from before filling the output. A window
    /// that opened maximized has none, so the client picks its size.
    fn restore(&mut self, toplevel: &ToplevelSurface) {
        let window = self.window_for(toplevel.wl_surface());
        let place = window
            .as_ref()
            .and_then(|window| saved(window).place.take());
        toplevel.with_pending_state(|pending| {
            pending.states.unset(State::Maximized);
            pending.states.unset(State::Fullscreen);
            pending.size = place.map(|place| place.size);
        });
        if toplevel.is_initial_configure_sent() {
            toplevel.send_configure();
        }
        match (window, place) {
            (Some(window), Some(place)) => self.space.map_element(window, place.loc, true),
            (Some(window), None) => saved(&window).place_when_drawn.set(true),
            (None, _) => {}
        }
        self.redraw_needed = true;
    }
}
