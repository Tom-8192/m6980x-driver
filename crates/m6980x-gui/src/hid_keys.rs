//! Physical keys of egui -> HID keyboard usages (page 0x07). egui names physical keys after their place on a
//! US keyboard, which is exactly what a HID usage means; the macro text then names them in the German layout.

use eframe::egui::Key;

pub fn usage(key: Key) -> Option<u8> {
    use Key::*;
    let letters = [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z];
    if let Some(i) = letters.iter().position(|k| *k == key) {
        return Some(0x04 + i as u8);
    }
    let digits = [Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9, Num0];
    if let Some(i) = digits.iter().position(|k| *k == key) {
        return Some(0x1E + i as u8);
    }
    let f_keys = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    if let Some(i) = f_keys.iter().position(|k| *k == key) {
        return Some(0x3A + i as u8);
    }
    let f_high = [F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24];
    if let Some(i) = f_high.iter().position(|k| *k == key) {
        return Some(0x68 + i as u8);
    }
    Some(match key {
        Enter => 0x28,
        Escape => 0x29,
        Backspace => 0x2A,
        Tab => 0x2B,
        Space => 0x2C,
        Minus => 0x2D,
        Equals => 0x2E,
        OpenBracket => 0x2F,
        CloseBracket => 0x30,
        Backslash => 0x31,
        Semicolon => 0x33,
        Quote => 0x34,
        Backtick => 0x35,
        Comma => 0x36,
        Period => 0x37,
        Slash => 0x38,
        Insert => 0x49,
        Home => 0x4A,
        PageUp => 0x4B,
        Delete => 0x4C,
        End => 0x4D,
        PageDown => 0x4E,
        ArrowRight => 0x4F,
        ArrowLeft => 0x50,
        ArrowDown => 0x51,
        ArrowUp => 0x52,
        Plus => 0x57,
        IntlBackslash => 0x64,
        ControlLeft => 0xE0,
        ShiftLeft => 0xE1,
        AltLeft => 0xE2,
        SuperLeft => 0xE3,
        ControlRight => 0xE4,
        ShiftRight => 0xE5,
        AltRight => 0xE6,
        SuperRight => 0xE7,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use m6980x_core::functions::key_name;

    #[test]
    fn positions_follow_the_german_names() {
        // US Y position is the German Z key
        assert_eq!(usage(Key::Y).and_then(key_name).as_deref(), Some("z"));
        assert_eq!(usage(Key::Num0), Some(0x27));
        assert_eq!(usage(Key::F12), Some(0x45));
        assert_eq!(usage(Key::ControlLeft), m6980x_core::functions::key("ctrl"));
        // every mapped key has a name the macro text understands
        for k in Key::ALL {
            if let Some(u) = usage(*k) {
                let name = key_name(u).unwrap_or_else(|| panic!("{k:?}"));
                assert_eq!(m6980x_core::functions::key(&name), Some(u), "{k:?}");
            }
        }
    }
}
