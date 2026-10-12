//! Macro recording: keyboard input of the whole window and mouse input inside the recording pad become steps.
//! While recording, keyboard events are taken out of egui's input so they do not operate the window.

use std::time::Instant;

use eframe::egui::{self, Event, MouseWheelUnit, PointerButton, Rect};
use m6980x_core::macro_steps::Step;

use crate::hid_keys;

/// Scroll distance in points that counts as one wheel notch (touchpads and smooth-scrolling mice).
const POINTS_PER_NOTCH: f32 = 50.0;
/// HID usages of C, X and V: Ctrl+C/X/V arrive as copy/cut/paste events instead of key presses.
const KEY_C: u8 = 0x06;
const KEY_X: u8 = 0x1B;
const KEY_V: u8 = 0x19;

pub struct Recorder {
    pub active: bool,
    /// record the real pauses between inputs
    pub with_delays: bool,
    /// pause between inputs when `with_delays` is off (0 = none)
    pub fixed_delay: u16,
    last: Option<Instant>,
    keys_down: Vec<u8>,
    mouse_down: u8,
    scroll: f32,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            active: false,
            with_delays: true,
            fixed_delay: 0,
            last: None,
            keys_down: Vec::new(),
            mouse_down: 0,
            scroll: 0.0,
        }
    }
}

impl Recorder {
    pub fn start(&mut self) {
        *self = Self { active: true, with_delays: self.with_delays, fixed_delay: self.fixed_delay, ..Self::default() };
    }

    /// Stops and returns the releases of keys and mouse buttons that are still held.
    pub fn stop(&mut self) -> Vec<Step> {
        self.active = false;
        let mut out: Vec<Step> = self.keys_down.drain(..).rev().map(Step::KeyUp).collect();
        for (_, mask) in m6980x_core::functions::MOUSE_BUTTONS {
            if self.mouse_down & mask != 0 {
                out.push(Step::MouseUp(mask));
            }
        }
        self.mouse_down = 0;
        out
    }

    /// Pause step before the next input.
    fn pause(&mut self, out: &mut Vec<Step>) {
        let now = Instant::now();
        if let Some(last) = self.last {
            let ms = if self.with_delays {
                last.elapsed().as_millis().clamp(1, u128::from(u16::MAX)) as u16
            } else {
                self.fixed_delay
            };
            if ms > 0 {
                out.push(Step::Wait(ms));
            }
        }
        self.last = Some(now);
    }

    fn key(&mut self, usage: u8, pressed: bool, out: &mut Vec<Step>) {
        let held = self.keys_down.contains(&usage);
        if pressed && !held {
            self.pause(out);
            self.keys_down.push(usage);
            out.push(Step::KeyDown(usage));
        } else if !pressed && held {
            self.pause(out);
            self.keys_down.retain(|&k| k != usage);
            out.push(Step::KeyUp(usage));
        }
    }

    /// Takes this frame's keyboard events (call before any widget is drawn) and returns the new steps.
    pub fn capture_keys(&mut self, ctx: &egui::Context) -> Vec<Step> {
        let mut out = Vec::new();
        if !self.active {
            return out;
        }
        let events = ctx.input_mut(|i| {
            let (keys, rest): (Vec<Event>, Vec<Event>) = std::mem::take(&mut i.events).into_iter().partition(|e| {
                matches!(e, Event::Key { .. } | Event::Copy | Event::Cut | Event::Paste(_) | Event::Text(_))
            });
            i.events = rest;
            keys
        });
        for e in events {
            match e {
                Event::Key { physical_key: Some(k), pressed, repeat: false, .. } => {
                    if let Some(u) = hid_keys::usage(k) {
                        self.key(u, pressed, &mut out);
                    }
                }
                Event::Copy => self.key(KEY_C, true, &mut out),
                Event::Cut => self.key(KEY_X, true, &mut out),
                Event::Paste(_) => self.key(KEY_V, true, &mut out),
                _ => {}
            }
        }
        // no widget may keep the focus, otherwise Enter/Space would press it
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        out
    }

    /// Mouse buttons and wheel inside `pad`; releases are taken anywhere so no button stays held.
    pub fn capture_pad(&mut self, ctx: &egui::Context, pad: Rect) -> Vec<Step> {
        let mut out = Vec::new();
        if !self.active {
            return out;
        }
        let (events, hover) = ctx.input(|i| (i.events.clone(), i.pointer.hover_pos()));
        for e in events {
            match e {
                Event::PointerButton { pos, button, pressed, .. } => {
                    let mask = match button {
                        PointerButton::Primary => 0x01,
                        PointerButton::Secondary => 0x02,
                        PointerButton::Middle => 0x04,
                        PointerButton::Extra1 => 0x08,
                        PointerButton::Extra2 => 0x10,
                    };
                    if pressed && pad.contains(pos) && self.mouse_down & mask == 0 {
                        self.pause(&mut out);
                        self.mouse_down |= mask;
                        out.push(Step::MouseDown(mask));
                    } else if !pressed && self.mouse_down & mask != 0 {
                        self.pause(&mut out);
                        self.mouse_down &= !mask;
                        out.push(Step::MouseUp(mask));
                    }
                }
                Event::MouseWheel { unit, delta, .. } if hover.is_some_and(|p| pad.contains(p)) => {
                    self.scroll += match unit {
                        MouseWheelUnit::Point => delta.y / POINTS_PER_NOTCH,
                        MouseWheelUnit::Line => delta.y,
                        MouseWheelUnit::Page => delta.y * 3.0,
                    };
                    let notches = self.scroll.trunc().clamp(-15.0, 15.0) as i8;
                    if notches != 0 {
                        self.scroll -= f32::from(notches);
                        self.pause(&mut out);
                        out.push(Step::Wheel(notches));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Key, Modifiers, pos2, vec2};

    fn key(k: Key, pressed: bool) -> Event {
        Event::Key { key: k, physical_key: Some(k), pressed, repeat: false, modifiers: Modifiers::NONE }
    }

    /// Runs one frame with these events and returns what the recorder made of them.
    fn frame(rec: &mut Recorder, events: Vec<Event>, pad: Option<Rect>) -> Vec<Step> {
        let ctx = egui::Context::default();
        let input = egui::RawInput { events, ..Default::default() };
        let mut out = Vec::new();
        let mut output = ctx.run_ui(input, |ui| {
            out = rec.capture_keys(ui.ctx());
            if let Some(pad) = pad {
                out.extend(rec.capture_pad(ui.ctx(), pad));
            }
        });
        output.textures_delta.clear(); // no renderer in tests
        out
    }

    #[test]
    fn keys_with_ctrl_c_and_without_repeats() {
        let mut rec = Recorder { with_delays: false, ..Recorder::default() };
        rec.start();
        let repeat = Event::Key {
            key: Key::A,
            physical_key: Some(Key::A),
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        // Ctrl+C arrives as a copy event instead of a press of C
        let events = vec![key(Key::ControlLeft, true), Event::Copy, key(Key::C, false), key(Key::ControlLeft, false)];
        let steps = frame(&mut rec, events, None);
        assert_eq!(steps, [Step::KeyDown(0xE0), Step::KeyDown(0x06), Step::KeyUp(0x06), Step::KeyUp(0xE0)]);
        // a release of a key pressed before recording is ignored, repeats too; held keys are released at the end
        let steps = frame(&mut rec, vec![key(Key::B, false), key(Key::A, true), repeat], None);
        assert_eq!(steps, [Step::KeyDown(0x04)]);
        assert_eq!(rec.stop(), [Step::KeyUp(0x04)]);
    }

    #[test]
    fn fixed_pause_between_inputs() {
        let mut rec = Recorder { with_delays: false, fixed_delay: 30, ..Recorder::default() };
        rec.start();
        let steps = frame(&mut rec, vec![key(Key::Enter, true), key(Key::Enter, false)], None);
        assert_eq!(steps, [Step::KeyDown(0x28), Step::Wait(30), Step::KeyUp(0x28)]);
    }

    #[test]
    fn mouse_only_inside_the_pad() {
        let mut rec = Recorder { with_delays: false, ..Recorder::default() };
        rec.start();
        let pad = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        let click = |x: f32, pressed: bool| Event::PointerButton {
            pos: pos2(x, 10.0),
            button: PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        // pressed outside (e.g. the stop button): nothing
        assert!(frame(&mut rec, vec![click(300.0, true), click(300.0, false)], Some(pad)).is_empty());
        let steps = frame(&mut rec, vec![click(10.0, true), click(300.0, false)], Some(pad));
        assert_eq!(steps, [Step::MouseDown(0x02), Step::MouseUp(0x02)]);
    }
}
