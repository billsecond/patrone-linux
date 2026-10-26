//! Keyboard and pointer input: the compositor's shortcuts, click to focus,
//! and everything else passed on to the window under the pointer.

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
    KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::input::keyboard::{FilterResult, Keysym, ModifiersState};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, CursorIcon, CursorImageStatus, GrabStartData, MotionEvent,
};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use crate::cli::Modifier;
use crate::grabs::Edges;
use crate::state::Patrone;

/// Linux input event codes, from linux/input-event-codes.h.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;

/// Scroll distance for one wheel notch, as the libinput backend reports it.
const WHEEL_STEP: f64 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    CloseWindow,
    OpenTerminal,
    ToggleMaximized,
    ToggleFullscreen,
    Quit,
    SwitchVt(i32),
}

impl Modifier {
    fn is_held(self, modifiers: &ModifiersState) -> bool {
        match self {
            Modifier::Super => modifiers.logo,
            Modifier::Alt => modifiers.alt,
        }
    }
}

/// The shortcut for `keysym` pressed with `modifiers`, if there is one.
fn shortcut(modifier: Modifier, modifiers: &ModifiersState, keysym: Keysym) -> Option<Action> {
    if !modifier.is_held(modifiers) {
        return None;
    }
    match keysym {
        Keysym::e | Keysym::E if modifiers.shift => Some(Action::Quit),
        Keysym::q | Keysym::Q => Some(Action::CloseWindow),
        Keysym::Return | Keysym::KP_Enter => Some(Action::OpenTerminal),
        Keysym::Up => Some(Action::ToggleMaximized),
        // Not F: Alt+F opens apps' File menus when the modifier is Alt.
        Keysym::F11 => Some(Action::ToggleFullscreen),
        _ => None,
    }
}

/// The virtual terminal that Ctrl+Alt+F1 to F12 switch to. The keyboard
/// layout turns those keys into the XF86Switch_VT keysyms.
fn vt_number(keysym: Keysym) -> Option<i32> {
    let first = Keysym::XF86_Switch_VT_1.raw();
    let last = Keysym::XF86_Switch_VT_12.raw();
    (first..=last)
        .contains(&keysym.raw())
        .then(|| (keysym.raw() - first + 1) as i32)
}

impl Patrone {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => self.on_key::<I>(event),
            InputEvent::PointerMotion { event } => self.on_pointer_motion_relative::<I>(event),
            InputEvent::PointerMotionAbsolute { event } => self.on_pointer_motion::<I>(event),
            InputEvent::PointerButton { event } => self.on_pointer_button::<I>(event),
            InputEvent::PointerAxis { event } => self.on_pointer_axis::<I>(event),
            _ => {}
        }
    }

    fn on_key<I: InputBackend>(&mut self, event: I::KeyboardKeyEvent) {
        let Some(keyboard) = self.seat.get_keyboard() else {
            return;
        };
        let keycode = event.key_code();
        let key_state = event.state();
        let modifier = self.modifier;
        let action = keyboard.input(
            self,
            keycode,
            key_state,
            SERIAL_COUNTER.next_serial(),
            event.time_msec(),
            |state, modifiers, keysym| {
                if key_state == KeyState::Released {
                    let pressed_for_shortcut = state.suppressed_keys.contains(&keycode);
                    state.suppressed_keys.retain(|key| *key != keycode);
                    return if pressed_for_shortcut {
                        FilterResult::Intercept(None)
                    } else {
                        FilterResult::Forward
                    };
                }
                if let Some(vt) = vt_number(keysym.modified_sym()) {
                    state.suppressed_keys.push(keycode);
                    return FilterResult::Intercept(Some(Action::SwitchVt(vt)));
                }
                let keysym = keysym.raw_latin_sym_or_raw_current_sym();
                match keysym.and_then(|keysym| shortcut(modifier, modifiers, keysym)) {
                    Some(action) => {
                        state.suppressed_keys.push(keycode);
                        FilterResult::Intercept(Some(action))
                    }
                    None => FilterResult::Forward,
                }
            },
        );
        let focused = self.focused_window();
        let toplevel = focused.as_ref().and_then(|window| window.toplevel());
        match (action.flatten(), toplevel) {
            (Some(Action::Quit), _) => self.loop_signal.stop(),
            (Some(Action::SwitchVt(vt)), _) => self.vt_switch = Some(vt),
            (Some(Action::OpenTerminal), _) => self.launcher.launch(),
            (Some(Action::CloseWindow), Some(toplevel)) => toplevel.send_close(),
            (Some(Action::ToggleMaximized), Some(toplevel)) => self.toggle_maximized(toplevel),
            (Some(Action::ToggleFullscreen), Some(toplevel)) => self.toggle_fullscreen(toplevel),
            _ => {}
        }
    }

    /// Absolute devices (Hyper-V's mouse, tablets, the nested window) give a
    /// position on the output.
    fn on_pointer_motion<I: InputBackend>(&mut self, event: I::PointerMotionAbsoluteEvent) {
        let Some(output) = self.output_geometry() else {
            return;
        };
        let location = event.position_transformed(output.size) + output.loc.to_f64();
        self.move_pointer(location, event.time_msec());
    }

    /// Mice report how far they moved. The pointer stays on the output.
    fn on_pointer_motion_relative<I: InputBackend>(&mut self, event: I::PointerMotionEvent) {
        let (Some(output), Some(pointer)) = (self.output_geometry(), self.seat.get_pointer())
        else {
            return;
        };
        let moved = pointer.current_location() + event.delta();
        let x = moved.x.clamp(
            output.loc.x as f64,
            (output.loc.x + output.size.w - 1) as f64,
        );
        let y = moved.y.clamp(
            output.loc.y as f64,
            (output.loc.y + output.size.h - 1) as f64,
        );
        self.move_pointer((x, y).into(), event.time_msec());
    }

    pub fn move_pointer(&mut self, location: Point<f64, Logical>, time: u32) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        let under = self.surface_under(location);
        if under.is_none() && !pointer.is_grabbed() {
            // Over the wallpaper: no client sets the cursor there.
            self.cursor = CursorImageStatus::Named(CursorIcon::Default);
        }
        pointer.motion(
            self,
            under,
            &MotionEvent {
                location,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
        pointer.frame(self);
        self.redraw_needed = true;
    }

    /// Clicking a window focuses and raises it. With the modifier held, the
    /// left button moves it and the right button resizes it.
    fn on_pointer_button<I: InputBackend>(&mut self, event: I::PointerButtonEvent) {
        let (Some(pointer), Some(keyboard)) = (self.seat.get_pointer(), self.seat.get_keyboard())
        else {
            return;
        };
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let state = event.state();
        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            let location = pointer.current_location();
            let window = self.space.element_under(location).map(|(w, _)| w.clone());
            self.focus(window.as_ref(), serial);
            if let Some(window) = window
                && self.modifier.is_held(&keyboard.modifier_state())
            {
                let start_data = GrabStartData {
                    focus: None,
                    button,
                    location,
                };
                match button {
                    BTN_LEFT => self.start_move(window, start_data, serial),
                    BTN_RIGHT => {
                        if let Some(geometry) = self.space.element_geometry(&window) {
                            let edges = Edges::nearest(geometry, location);
                            self.start_resize(window, edges, start_data, serial);
                        }
                    }
                    _ => {}
                }
            }
        }
        pointer.button(
            self,
            &ButtonEvent {
                button,
                state,
                serial,
                time: event.time_msec(),
            },
        );
        pointer.frame(self);
    }

    fn on_pointer_axis<I: InputBackend>(&mut self, event: I::PointerAxisEvent) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        let source = event.source();
        let mut frame = AxisFrame::new(event.time_msec()).source(source);
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let v120 = event.amount_v120(axis);
            let amount = event
                .amount(axis)
                .unwrap_or_else(|| v120.unwrap_or(0.0) * WHEEL_STEP / 120.0);
            if amount != 0.0 {
                frame = frame.value(axis, amount);
                if let Some(v120) = v120 {
                    frame = frame.v120(axis, v120 as i32);
                }
            }
            if source == AxisSource::Finger && event.amount(axis) == Some(0.0) {
                frame = frame.stop(axis);
            }
        }
        pointer.axis(self, frame);
        pointer.frame(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(logo: bool, alt: bool) -> ModifiersState {
        ModifiersState {
            logo,
            alt,
            ..ModifiersState::default()
        }
    }

    #[test]
    fn shortcuts_need_the_chosen_modifier() {
        let super_held = held(true, false);
        let alt_held = held(false, true);
        assert_eq!(
            shortcut(Modifier::Super, &super_held, Keysym::q),
            Some(Action::CloseWindow)
        );
        assert_eq!(shortcut(Modifier::Super, &alt_held, Keysym::q), None);
        assert_eq!(
            shortcut(Modifier::Alt, &alt_held, Keysym::Return),
            Some(Action::OpenTerminal)
        );
        assert_eq!(
            shortcut(Modifier::Super, &held(false, false), Keysym::q),
            None
        );
        assert_eq!(shortcut(Modifier::Super, &super_held, Keysym::w), None);
    }

    #[test]
    fn maximize_and_fullscreen_shortcuts() {
        let super_held = held(true, false);
        assert_eq!(
            shortcut(Modifier::Super, &super_held, Keysym::Up),
            Some(Action::ToggleMaximized)
        );
        assert_eq!(
            shortcut(Modifier::Alt, &held(false, true), Keysym::F11),
            Some(Action::ToggleFullscreen)
        );
        assert_eq!(shortcut(Modifier::Super, &super_held, Keysym::f), None);
    }

    #[test]
    fn quitting_needs_shift_too() {
        let super_shift = ModifiersState {
            logo: true,
            shift: true,
            ..ModifiersState::default()
        };
        assert_eq!(
            shortcut(Modifier::Super, &super_shift, Keysym::E),
            Some(Action::Quit)
        );
        assert_eq!(
            shortcut(Modifier::Super, &held(true, false), Keysym::e),
            None
        );
    }

    #[test]
    fn virtual_terminal_keys() {
        assert_eq!(vt_number(Keysym::XF86_Switch_VT_1), Some(1));
        assert_eq!(vt_number(Keysym::XF86_Switch_VT_12), Some(12));
        assert_eq!(vt_number(Keysym::F1), None);
    }
}
