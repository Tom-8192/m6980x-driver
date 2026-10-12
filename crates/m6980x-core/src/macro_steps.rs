//! Macro events as a list of editable steps (step editor and recorder of the GUI).
//! Same byte format as `functions::encode_events` / `decode_record`.

use crate::functions::{
    self as fnc, EV_CONSUMER, EV_END, EV_KEY_DOWN, EV_KEY_UP, EV_MOUSE_DOWN, EV_MOUSE_UP, EV_WAIT8, EV_WAIT16,
    EV_WHEEL, EV_WHEEL_DOWN, EV_WHEEL_UP, MODE_ONCE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// HID keyboard usage
    KeyDown(u8),
    KeyUp(u8),
    /// milliseconds, 1-65535
    Wait(u16),
    /// mouse button mask (`functions::MOUSE_BUTTONS`)
    MouseDown(u8),
    MouseUp(u8),
    /// notches, positive = up; one event per notch
    Wheel(i8),
    /// consumer usage, 0 = release
    Consumer(u16),
    /// byte the codec does not know, kept so that a record survives editing unchanged
    Raw(u8),
}

impl Step {
    /// Event bytes of this step.
    pub fn bytes(&self) -> Vec<u8> {
        match *self {
            Step::KeyDown(k) => vec![EV_KEY_DOWN, k],
            Step::KeyUp(k) => vec![EV_KEY_UP, k],
            Step::Wait(ms) => {
                let ms = ms.max(1);
                if ms <= 0xFF { vec![EV_WAIT8, ms as u8] } else { vec![EV_WAIT16, (ms & 0xFF) as u8, (ms >> 8) as u8] }
            }
            Step::MouseDown(m) => vec![EV_MOUSE_DOWN | (m & 0x1F)],
            Step::MouseUp(m) => vec![EV_MOUSE_UP | (m & 0x1F)],
            Step::Wheel(n) => {
                let ev = if n > 0 { EV_WHEEL_UP } else { EV_WHEEL_DOWN };
                vec![ev; usize::from(n.unsigned_abs())]
            }
            Step::Consumer(u) => vec![EV_CONSUMER, (u & 0xFF) as u8, (u >> 8) as u8],
            Step::Raw(b) => vec![b],
        }
    }

    /// Text of this step in the current language, as in the macro text ("press ctrl").
    pub fn label(&self) -> String {
        let mut record = vec![MODE_ONCE];
        record.extend(self.bytes());
        record.push(EV_END);
        fnc::decode_record(&record).1.join(", ")
    }
}

/// Event bytes (without mode byte; stops at the end mark) -> steps. Consecutive wheel notches in the
/// same direction become one step.
pub fn steps_from_events(events: &[u8]) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut i = 0;
    let arg = |i: usize| events.get(i).copied();
    while let Some(b) = arg(i) {
        if b == EV_END {
            break;
        }
        let (step, size) = match (b, arg(i + 1), arg(i + 2)) {
            (EV_KEY_DOWN, Some(k), _) => (Step::KeyDown(k), 2),
            (EV_KEY_UP, Some(k), _) => (Step::KeyUp(k), 2),
            (EV_WAIT8, Some(ms), _) => (Step::Wait(u16::from(ms)), 2),
            (EV_WAIT16, Some(lo), Some(hi)) => (Step::Wait(u16::from(lo) | u16::from(hi) << 8), 3),
            (EV_CONSUMER, Some(lo), Some(hi)) => (Step::Consumer(u16::from(lo) | u16::from(hi) << 8), 3),
            _ if b & 0xE0 == EV_WHEEL && b & 0x1F != 0 => {
                let raw = (b & 0x1F) as i8;
                let notches = if raw & 0x10 != 0 { raw - 32 } else { raw };
                let dir = notches.signum();
                match steps.last_mut() {
                    Some(Step::Wheel(n)) if n.signum() == dir && n.checked_add(dir).is_some() => {
                        *n += dir;
                        i += 1;
                        continue;
                    }
                    _ => (Step::Wheel(dir), 1),
                }
            }
            _ if b & 0xE0 == EV_MOUSE_DOWN && b & 0x1F != 0 => (Step::MouseDown(b & 0x1F), 1),
            _ if b & 0xE0 == EV_MOUSE_UP && b & 0x1F != 0 => (Step::MouseUp(b & 0x1F), 1),
            _ => (Step::Raw(b), 1),
        };
        steps.push(step);
        i += size;
    }
    steps
}

/// Steps -> event bytes (without mode byte and end mark).
pub fn events_from_steps(steps: &[Step]) -> Vec<u8> {
    steps.iter().flat_map(Step::bytes).collect()
}

/// Bytes a record of these steps needs: mode byte + events + end mark.
pub fn record_size(steps: &[Step]) -> usize {
    2 + steps.iter().map(|s| s.bytes().len()).sum::<usize>()
}

/// Releases for the keys and mouse buttons the steps leave pressed (last pressed key first).
pub fn held_releases(steps: &[Step]) -> Vec<Step> {
    let mut keys: Vec<u8> = Vec::new();
    let mut mouse = 0u8;
    for step in steps {
        match *step {
            Step::KeyDown(k) if !keys.contains(&k) => keys.push(k),
            Step::KeyUp(k) => keys.retain(|&x| x != k),
            Step::MouseDown(m) => mouse |= m,
            Step::MouseUp(m) => mouse &= !m,
            _ => {}
        }
    }
    let buttons = fnc::MOUSE_BUTTONS.iter().filter(|(_, m)| mouse & m != 0).map(|(_, m)| Step::MouseUp(*m));
    keys.into_iter().rev().map(Step::KeyUp).chain(buttons).collect()
}

/// Drops steps from the end until the record fits into `max` bytes, keeping no key or button pressed.
pub fn fit(steps: &mut Vec<Step>, max: usize) {
    loop {
        let releases = held_releases(steps);
        if record_size(steps) + releases.iter().map(|s| s.bytes().len()).sum::<usize>() <= max {
            steps.extend(releases);
            return;
        }
        if steps.pop().is_none() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(text: &str) -> Vec<u8> {
        text.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    #[test]
    fn round_trip_keeps_bytes() {
        let events = h("83 e0 88 64 84 e0 21 41 7f 7f 61 85 e9 00 85 00 00 89 e8 03 99");
        let steps = steps_from_events(&events);
        assert_eq!(
            steps,
            [
                Step::KeyDown(0xE0),
                Step::Wait(100),
                Step::KeyUp(0xE0),
                Step::MouseDown(1),
                Step::MouseUp(1),
                Step::Wheel(-2),
                Step::Wheel(1),
                Step::Consumer(0xE9),
                Step::Consumer(0),
                Step::Wait(1000),
                Step::Raw(0x99),
            ]
        );
        assert_eq!(events_from_steps(&steps), events);
        assert_eq!(record_size(&steps), events.len() + 2);
    }

    #[test]
    fn matches_text_codec() {
        let text = "press ctrl, tap c, release ctrl, wait 300, click right, wheel up 3, consumer 0x00E9";
        let events = fnc::encode_events(text).unwrap();
        let steps = steps_from_events(&events);
        assert_eq!(events_from_steps(&steps), events);
        let labels: Vec<String> = steps.iter().map(Step::label).collect();
        let (_, decoded) = fnc::decode_record(&[&[MODE_ONCE][..], &events, &[EV_END]].concat());
        assert_eq!(labels, decoded);
    }

    #[test]
    fn fit_releases_what_is_still_held() {
        let mut steps =
            vec![Step::KeyDown(0xE0), Step::MouseDown(1), Step::KeyDown(4), Step::KeyUp(4), Step::Wait(500)];
        assert_eq!(held_releases(&steps), [Step::KeyUp(0xE0), Step::MouseUp(1)]);
        // mode + end 2, steps without the wait 7, releases 3: 12 bytes
        fit(&mut steps, 12);
        assert_eq!(
            steps,
            [
                Step::KeyDown(0xE0),
                Step::MouseDown(1),
                Step::KeyDown(4),
                Step::KeyUp(4),
                Step::KeyUp(0xE0),
                Step::MouseUp(1)
            ]
        );
        assert_eq!(record_size(&steps), 12);
    }

    #[test]
    fn stops_at_end_mark_and_clamps_wait() {
        assert_eq!(steps_from_events(&h("83 04 00 83 05")), [Step::KeyDown(4)]);
        assert_eq!(Step::Wait(0).bytes(), [EV_WAIT8, 1]);
    }
}
