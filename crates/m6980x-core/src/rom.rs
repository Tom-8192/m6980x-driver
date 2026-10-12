//! Aufbau des 8-KB-Onboard-ROMs (siehe docs/PROTOCOL.md; das Original rechnet genauso, 0x42ebd8).
//!
//! ```text
//!   0x0000-0x1CBF  20 Makro-Slots zu 0x170 Bytes (Slot n bei n * 0x170)
//!   0x1CC1-0x1D88  Makro-Info-Tabelle, 20 Eintraege zu 10 Bytes (MacroInfoSize=10)
//!   0x1D8C-0x1EEF  Funktionsdatensaetze an festen Adressen, inkl. der Klicks fuer die Radtasten (functions.rs)
//!   0x1F00-0x1FFF  von der Firmware verwaltet: 0x1F00 = Marke 0x12; Tasten-Flags (80 belegt / 00 Standard)
//!                  bei BUTTON_FLAGS + Profil * 12 + Taste; Tastenadressen bei
//!                  BUTTON_TABLE + Profil * 0x18 + Taste * 2 (bleiben nach "Standard" stehen)
//! ```

pub const MACRO_SLOTS: usize = 20;
pub const MACRO_SLOT_SIZE: usize = 0x170;
pub const MACRO_INFO_BASE: u16 = 0x1CC1;
pub const MACRO_INFO_SIZE: usize = 10;
pub const BUTTON_FLAGS: usize = 0x1F01;
pub const BUTTON_FLAGS_STRIDE: usize = 12;
pub const BUTTON_TABLE: usize = 0x1F3D;
pub const BUTTON_TABLE_STRIDE: usize = 0x18;
/// Namensbytes; Byte 9 des Eintrags ist das Bild-Byte.
pub const MACRO_NAME_LEN: usize = 9;
/// Bild-Byte (Macro.exe 0x4133a0): neue Makros bekommen Slot + 1 (kein Bild, die GUI zeigt "MACRO n"),
/// der Dialog "Change Icon" schreibt 100 + n fuer Bild n seines 100er-Rasters. Bild 1-40 sind in
/// Macro.exe eingebettet, 41-100 kommen zur Laufzeit aus bmp\1.bmp ... bmp\60.bmp (leere Platzhalter).
/// Bestaetigt: Toms "Klicker" (0x68) zeigt Bild 4.
pub const ICON_OFFSET: u8 = 100;
pub const ICON_COUNT: u8 = 100;
pub const ICONS_EMBEDDED: u8 = 40;

/// Bildnummer 1-100 eines Makros oder None (kein Bild / Standard).
pub fn macro_icon(info: &[u8]) -> Option<u8> {
    let value = info.get(MACRO_NAME_LEN).copied().unwrap_or(0xFF);
    let n = value.wrapping_sub(ICON_OFFSET);
    (value >= ICON_OFFSET && (1..=ICON_COUNT).contains(&n)).then_some(n)
}

/// Herkunft von Bild n: im Original eingebettet oder eigene Datei bmp\<n-40>.bmp im Programmordner des Originals.
pub fn icon_source(n: u8) -> String {
    if (1..=ICONS_EMBEDDED).contains(&n) {
        "eingebaut".into()
    } else {
        format!("eigenes bmp\\{}.bmp", n - ICONS_EMBEDDED)
    }
}

/// Datensatzadresse einer Taste in einem ROM-Abbild, None wenn das Flag "Standard" (00) sagt.
/// Der Tabelleneintrag allein reicht nicht: die Firmware laesst ihn nach "Standard" stehen.
pub fn button_assignment(rom: &[u8], profile: usize, button: usize) -> Option<u16> {
    if rom[BUTTON_FLAGS + profile * BUTTON_FLAGS_STRIDE + button] & 0x80 == 0 {
        return None;
    }
    let a = BUTTON_TABLE + profile * BUTTON_TABLE_STRIDE + button * 2;
    Some(u16::from(rom[a]) | u16::from(rom[a + 1]) << 8)
}

pub fn macro_slot_addr(slot: usize) -> u16 {
    (slot * MACRO_SLOT_SIZE) as u16
}

pub fn macro_info_addr(slot: usize) -> u16 {
    MACRO_INFO_BASE + (slot * MACRO_INFO_SIZE) as u16
}

pub fn macro_info(rom: &[u8], slot: usize) -> &[u8] {
    let start = macro_info_addr(slot) as usize;
    &rom[start..start + MACRO_INFO_SIZE]
}

pub fn macro_slot(rom: &[u8], slot: usize) -> &[u8] {
    let start = macro_slot_addr(slot) as usize;
    &rom[start..start + MACRO_SLOT_SIZE]
}

pub fn printable(data: &[u8]) -> String {
    data.iter().map(|&c| if (32..127).contains(&c) { c as char } else { '.' }).collect()
}

pub fn is_erased(data: &[u8]) -> bool {
    data.iter().all(|&c| c == 0xFF)
}

pub fn macro_slot_of(addr: u16) -> Option<usize> {
    let addr = addr as usize;
    (addr < MACRO_SLOTS * MACRO_SLOT_SIZE && addr.is_multiple_of(MACRO_SLOT_SIZE)).then_some(addr / MACRO_SLOT_SIZE)
}

/// Bytes als Latin-1-Text (wie Pythons `decode("latin-1")`).
pub fn latin1(data: &[u8]) -> String {
    data.iter().map(|&c| c as char).collect()
}

/// Namensteil eines Info-Eintrags; geloeschte (FF) Eintraege haben keinen Namen.
pub fn macro_name(info: &[u8]) -> String {
    let raw = &info[..MACRO_NAME_LEN.min(info.len())];
    if is_erased(raw) {
        return String::new();
    }
    latin1(raw.split(|&c| c == 0).next().unwrap_or_default())
}

/// Alle 20 Makronamen aus der Info-Tabelle (MACRO_SLOTS * MACRO_INFO_SIZE Bytes ab MACRO_INFO_BASE).
pub fn macro_names(info_table: &[u8]) -> Vec<String> {
    info_table.chunks(MACRO_INFO_SIZE).take(MACRO_SLOTS).map(macro_name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_byte_and_source() {
        let klicker_info = [0x4b, 0x6c, 0x69, 0x63, 0x6b, 0x65, 0x72, 0x00, 0x2c, 0x68]; // echt, von Tom bestaetigt
        assert_eq!(macro_icon(&klicker_info), Some(4));
        assert_eq!(macro_name(&klicker_info), "Klicker");
        assert_eq!(icon_source(4), "eingebaut");
        assert_eq!(icon_source(41), "eigenes bmp\\1.bmp");
        let mut new_macro = [0u8; 10];
        new_macro[9] = 3; // neues Makro: Slot + 1
        assert_eq!(macro_icon(&new_macro), None);
        assert_eq!(macro_name(&[0xFF; 10]), "");
    }
}
