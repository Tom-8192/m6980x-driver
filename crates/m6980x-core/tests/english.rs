//! Ohne cfg(test) ist Englisch die Standardsprache - Ausgaben englisch, Eingaben in beiden Sprachen.

use m6980x_core::functions::{self as fnc, by_key};
use m6980x_core::i18n;
use m6980x_core::names::{describe_target, format_led, parse_button, parse_led, standard_label};

fn h(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
}

#[test]
fn english_is_default_and_german_input_still_works() {
    assert_eq!(i18n::lang(), "en");
    assert_eq!(i18n::available(), ["en", "de"]);

    let (mode, events) = fnc::decode_record(&h("05 83 e0 88 64 84 e0 21 41 7f 7f 85 00 00 00"));
    assert_eq!(mode.as_deref(), Some("loop"));
    assert_eq!(
        events,
        [
            "press ctrl",
            "wait 100",
            "release ctrl",
            "mouse-down left",
            "mouse-up left",
            "wheel down 2",
            "consumer release"
        ]
    );
    assert_eq!(fnc::encode_events("tippe strg+c").unwrap(), fnc::encode_events("tap ctrl+c").unwrap());

    assert_eq!(by_key("kopieren").unwrap().name(), "copy");
    assert_eq!(by_key("copy").unwrap().label(), "Copy (Ctrl+C)");
    assert_eq!(parse_button("hinten").unwrap(), parse_button("back").unwrap());
    assert_eq!(parse_led(&["rot"]).unwrap(), parse_led(&["red"]).unwrap());
    assert_eq!(format_led([0, 2, 0]), "R 0  G 2  B 0   green");
    assert_eq!(standard_label(Some(3)), "Default (Back, mouse button 4)");
    assert_eq!(describe_target(Some(0x170), &[], None), "Macro 2");

    let err = fnc::encode_events("springe").unwrap_err().to_string();
    assert!(err.starts_with("unknown command 'springe'"), "{err}");
}
