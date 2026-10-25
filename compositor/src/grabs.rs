//! Moving and resizing windows with the pointer, for clients that ask (GTK's
//! title bars and borders) and for the modifier+drag shortcuts.

use smithay::desktop::Window;
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, Focus, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
    GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab,
    PointerInnerHandle, RelativeMotionEvent,
};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Serial, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::SurfaceCachedState;

use crate::state::Patrone;

/// The edges of a window that a resize moves.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl Edges {
    /// The corner of `rect` nearest to `point`.
    pub fn nearest(rect: Rectangle<i32, Logical>, point: Point<f64, Logical>) -> Self {
        let left = point.x < rect.loc.x as f64 + rect.size.w as f64 / 2.0;
        let top = point.y < rect.loc.y as f64 + rect.size.h as f64 / 2.0;
        Edges {
            left,
            right: !left,
            top,
            bottom: !top,
        }
    }
}

impl From<xdg_toplevel::ResizeEdge> for Edges {
    fn from(edge: xdg_toplevel::ResizeEdge) -> Self {
        // The protocol's values are bit flags: top 1, bottom 2, left 4, right 8.
        let bits = edge as u32;
        Edges {
            top: bits & 1 != 0,
            bottom: bits & 2 != 0,
            left: bits & 4 != 0,
            right: bits & 8 != 0,
        }
    }
}

/// A window being resized. Resizing from the top or left moves the window
/// as well, once the client has drawn at the new size.
pub struct Resize {
    window: Window,
    edges: Edges,
    initial: Rectangle<i32, Logical>,
    finished: bool,
}

/// Keeps the opposite edges of a window being resized in place. Called on
/// every commit.
pub fn commit(state: &mut Patrone, surface: &WlSurface) {
    let Some(resize) = &state.resize else {
        return;
    };
    let Some(toplevel) = resize.window.toplevel() else {
        return;
    };
    if toplevel.wl_surface() != surface {
        return;
    }
    if let Some(location) = state.space.element_location(&resize.window) {
        let size = resize.window.geometry().size;
        let new_location = location_after_resize(resize.initial, resize.edges, location, size);
        if new_location != location {
            state
                .space
                .map_element(resize.window.clone(), new_location, false);
        }
    }
    let resizing = toplevel
        .current_state()
        .states
        .contains(xdg_toplevel::State::Resizing);
    if resize.finished && !resizing {
        state.resize = None;
    }
}

enum Kind {
    Move {
        initial_location: Point<i32, Logical>,
    },
    Resize {
        edges: Edges,
        initial_size: Size<i32, Logical>,
        last_size: Size<i32, Logical>,
    },
}

pub struct WindowGrab {
    start_data: GrabStartData<Patrone>,
    window: Window,
    kind: Kind,
}

impl Patrone {
    pub fn start_move(&mut self, window: Window, start_data: GrabStartData<Self>, serial: Serial) {
        let Some(initial_location) = self.space.element_location(&window) else {
            return;
        };
        self.set_grab(
            WindowGrab {
                start_data,
                window,
                kind: Kind::Move { initial_location },
            },
            serial,
        );
    }

    pub fn start_resize(
        &mut self,
        window: Window,
        edges: Edges,
        start_data: GrabStartData<Self>,
        serial: Serial,
    ) {
        let (Some(location), Some(toplevel)) =
            (self.space.element_location(&window), window.toplevel())
        else {
            return;
        };
        let initial = Rectangle::new(location, window.geometry().size);
        toplevel.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
        });
        toplevel.send_pending_configure();
        self.resize = Some(Resize {
            window: window.clone(),
            edges,
            initial,
            finished: false,
        });
        self.set_grab(
            WindowGrab {
                start_data,
                window,
                kind: Kind::Resize {
                    edges,
                    initial_size: initial.size,
                    last_size: initial.size,
                },
            },
            serial,
        );
    }

    fn set_grab(&mut self, grab: WindowGrab, serial: Serial) {
        if let Some(pointer) = self.seat.get_pointer() {
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }
}

impl PointerGrab<Patrone> for WindowGrab {
    fn motion(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // No client gets pointer events during the grab.
        handle.motion(data, None, event);
        let delta = event.location - self.start_data.location;
        match &mut self.kind {
            Kind::Move { initial_location } => {
                let location = (initial_location.to_f64() + delta).to_i32_round();
                data.space.map_element(self.window.clone(), location, true);
            }
            Kind::Resize {
                edges,
                initial_size,
                last_size,
            } => {
                let (min, max) = size_limits(&self.window);
                *last_size = resized(*initial_size, *edges, delta, min, max);
                if let Some(toplevel) = self.window.toplevel() {
                    toplevel.with_pending_state(|state| {
                        state.states.set(xdg_toplevel::State::Resizing);
                        state.size = Some(*last_size);
                    });
                    toplevel.send_pending_configure();
                }
            }
        }
        data.redraw_needed = true;
    }

    fn relative_motion(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut Patrone, handle: &mut PointerInnerHandle<'_, Patrone>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut Patrone,
        handle: &mut PointerInnerHandle<'_, Patrone>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &GrabStartData<Patrone> {
        &self.start_data
    }

    /// Ends a resize: the client gets its final size without the resizing state.
    fn unset(&mut self, data: &mut Patrone) {
        let Kind::Resize { last_size, .. } = self.kind else {
            return;
        };
        if let Some(toplevel) = self.window.toplevel() {
            toplevel.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Resizing);
                state.size = Some(last_size);
            });
            toplevel.send_pending_configure();
        }
        if let Some(resize) = &mut data.resize {
            resize.finished = true;
        }
    }
}

/// The smallest and largest size the client allows. Zero means no limit.
fn size_limits(window: &Window) -> (Size<i32, Logical>, Size<i32, Logical>) {
    let Some(toplevel) = window.toplevel() else {
        return Default::default();
    };
    with_states(toplevel.wl_surface(), |states| {
        let mut cached = states.cached_state.get::<SurfaceCachedState>();
        let current = cached.current();
        (current.min_size, current.max_size)
    })
}

/// The size of a window of `initial` size after dragging `edges` by `delta`.
fn resized(
    initial: Size<i32, Logical>,
    edges: Edges,
    delta: Point<f64, Logical>,
    min: Size<i32, Logical>,
    max: Size<i32, Logical>,
) -> Size<i32, Logical> {
    let dx = delta.x.round() as i32;
    let dy = delta.y.round() as i32;
    let mut width = initial.w;
    let mut height = initial.h;
    if edges.left {
        width -= dx;
    } else if edges.right {
        width += dx;
    }
    if edges.top {
        height -= dy;
    } else if edges.bottom {
        height += dy;
    }
    let limit = |value: i32, min: i32, max: i32| {
        let max = if max > 0 { max } else { i32::MAX };
        value.min(max).max(min.max(1))
    };
    (limit(width, min.w, max.w), limit(height, min.h, max.h)).into()
}

/// Where a window resized from `initial` to `size` goes, so the edges that
/// weren't dragged stay put.
fn location_after_resize(
    initial: Rectangle<i32, Logical>,
    edges: Edges,
    location: Point<i32, Logical>,
    size: Size<i32, Logical>,
) -> Point<i32, Logical> {
    let mut location = location;
    if edges.left {
        location.x = initial.loc.x + initial.size.w - size.w;
    }
    if edges.top {
        location.y = initial.loc.y + initial.size.h - size.h;
    }
    location
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_limit() -> Size<i32, Logical> {
        (0, 0).into()
    }

    #[test]
    fn protocol_edges_are_bit_flags() {
        let edges = Edges::from(xdg_toplevel::ResizeEdge::TopLeft);
        assert!(edges.top && edges.left && !edges.bottom && !edges.right);
        let edges = Edges::from(xdg_toplevel::ResizeEdge::Right);
        assert!(edges.right && !edges.left && !edges.top && !edges.bottom);
    }

    #[test]
    fn nearest_corner() {
        let rect = Rectangle::new((100, 100).into(), (200, 100).into());
        let edges = Edges::nearest(rect, (120.0, 190.0).into());
        assert_eq!(
            edges,
            Edges {
                left: true,
                bottom: true,
                ..Edges::default()
            }
        );
    }

    #[test]
    fn dragging_the_right_edge_grows_and_the_left_edge_shrinks() {
        let size = (400, 300).into();
        let delta = (50.0, 20.0).into();
        let right = Edges {
            right: true,
            ..Edges::default()
        };
        let left = Edges {
            left: true,
            ..Edges::default()
        };
        assert_eq!(
            resized(size, right, delta, no_limit(), no_limit()),
            (450, 300).into()
        );
        assert_eq!(
            resized(size, left, delta, no_limit(), no_limit()),
            (350, 300).into()
        );
    }

    #[test]
    fn resizing_respects_the_clients_limits() {
        let edges = Edges {
            right: true,
            bottom: true,
            ..Edges::default()
        };
        let size = (400, 300).into();
        let min = (360, 0).into();
        let max = (0, 320).into();
        assert_eq!(
            resized(size, edges, (-100.0, 100.0).into(), min, max),
            (360, 320).into()
        );
        assert_eq!(
            resized(
                size,
                edges,
                (-1000.0, -1000.0).into(),
                no_limit(),
                no_limit()
            ),
            (1, 1).into()
        );
    }

    #[test]
    fn resizing_from_the_top_left_keeps_the_bottom_right_in_place() {
        let initial = Rectangle::new((100, 100).into(), (400, 300).into());
        let edges = Edges {
            left: true,
            top: true,
            ..Edges::default()
        };
        let location = location_after_resize(initial, edges, initial.loc, (450, 250).into());
        assert_eq!(location, (50, 150).into());
        let bottom_right = Edges {
            right: true,
            bottom: true,
            ..Edges::default()
        };
        let location = location_after_resize(initial, bottom_right, initial.loc, (450, 250).into());
        assert_eq!(location, initial.loc);
    }
}
