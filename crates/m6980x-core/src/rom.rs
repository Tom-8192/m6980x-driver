//! Layout of the 8 KB onboard ROM (see docs/PROTOCOL.md; the original computes the same, 0x42ebd8).
//!
//! ```text
//!   0x0000-0x1CBF  20 macro slots of 0x170 bytes (slot n at n * 0x170)
//!   0x1CC1-0x1D88  macro info table, 20 entries of 10 bytes (MacroInfoSize=10)
//!   0x1D8C-0x1EEF  function records at fixed addresses, incl. the clicks for the wheel buttons (functions.rs)
//!   0x1F00-0x1FFF  managed by the firmware: 0x1F00 = marker 0x12; button flags (80 assigned / 00 default)
//!                  at BUTTON_FLAGS + profile * 12 + button; button addresses at
//!                  BUTTON_TABLE + profile * 0x18 + button * 2 (left in place after "default")
//! ```

use crate::tr;

pub const MACRO_SLOTS: usize = 20;
pub const MACRO_SLOT_SIZE: usize = 0x170;
pub const MACRO_INFO_BASE: u16 = 0x1CC1;
pub const MACRO_INFO_SIZE: usize = 10;
pub const BUTTON_FLAGS: usize = 0x1F01;
pub const BUTTON_FLAGS_STRIDE: usize = 12;
pub const BUTTON_TABLE: usize = 0x1F3D;
pub const BUTTON_TABLE_STRIDE: usize = 0x18;
/// name bytes; byte 9 of the entry is the icon byte.
pub const MACRO_NAME_LEN: usize = 9;
/// Icon byte (Macro.exe 0x4133a0): new macros get slot + 1 (no icon, the GUI shows "MACRO n"),
/// the "Change Icon" dialog writes 100 + n for icon n of its grid of 100. Icons 1-40 are embedded in
/// Macro.exe, 41-100 are loaded at runtime from bmp\1.bmp ... bmp\60.bmp (empty placeholders).
/// Confirmed: Tom's "Klicker" (0x68) shows icon 4.
pub const ICON_OFFSET: u8 = 100;
pub const ICON_COUNT: u8 = 100;
pub const ICONS_EMBEDDED: u8 = 40;

/// Icon number 1-100 of a macro, or None (no icon / default).
pub fn macro_icon(info: &[u8]) -> Option<u8> {
    let value = info.get(MACRO_NAME_LEN).copied().unwrap_or(0xFF);
    let n = value.wrapping_sub(ICON_OFFSET);
    (value >= ICON_OFFSET && (1..=ICON_COUNT).contains(&n)).then_some(n)
}

/// Source of icon n: embedded in the original or a custom file bmp\<n-40>.bmp in the original's program folder.
pub fn icon_source(n: u8) -> String {
    if (1..=ICONS_EMBEDDED).contains(&n) { tr!("icon.built_in") } else { tr!("icon.custom", n = n - ICONS_EMBEDDED) }
}

/// Record address of a button in a ROM image, None if the flag says "default" (00).
/// The table entry alone is not enough: the firmware leaves it in place after "default".
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

/// Record of a slot in use, empty for a free slot. Like the original, a slot without a name is free:
/// "Release" only clears name byte 0, the old data and icon byte stay in the ROM.
pub fn used_macro_slot(rom: &[u8], slot: usize) -> &[u8] {
    if macro_name(macro_info(rom, slot)).is_empty() { &[] } else { macro_slot(rom, slot) }
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

/// Bytes as Latin-1 text (like Python `decode("latin-1")`).
pub fn latin1(data: &[u8]) -> String {
    data.iter().map(|&c| c as char).collect()
}

/// Name part of an info entry; erased (FF) entries have no name.
pub fn macro_name(info: &[u8]) -> String {
    let raw = &info[..MACRO_NAME_LEN.min(info.len())];
    if is_erased(raw) {
        return String::new();
    }
    latin1(raw.split(|&c| c == 0).next().unwrap_or_default())
}

/// All 20 macro names from the info table (MACRO_SLOTS * MACRO_INFO_SIZE bytes from MACRO_INFO_BASE).
pub fn macro_names(info_table: &[u8]) -> Vec<String> {
    info_table.chunks(MACRO_INFO_SIZE).take(MACRO_SLOTS).map(macro_name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_byte_and_source() {
        let klicker_info = [0x4b, 0x6c, 0x69, 0x63, 0x6b, 0x65, 0x72, 0x00, 0x2c, 0x68]; // real, confirmed by Tom
        assert_eq!(macro_icon(&klicker_info), Some(4));
        assert_eq!(macro_name(&klicker_info), "Klicker");
    }

    #[test]
    fn released_slot_is_free() {
        let mut rom = vec![0xFF; 0x2000];
        let (data, info) = (macro_slot_addr(1) as usize, macro_info_addr(1) as usize);
        rom[data..data + 4].copy_from_slice(&[0x05, 0x83, 0x17, 0x00]);
        rom[info..info + 3].copy_from_slice(b"ab\0");
        assert_eq!(used_macro_slot(&rom, 1)[..4], [0x05, 0x83, 0x17, 0x00]);
        rom[info] = 0x00; // "Release"
        assert!(used_macro_slot(&rom, 1).is_empty());
        assert!(used_macro_slot(&rom, 2).is_empty()); // never written (erased)
        assert_eq!(icon_source(4), "eingebaut");
        assert_eq!(icon_source(41), "eigenes bmp\\1.bmp");
        let mut new_macro = [0u8; 10];
        new_macro[9] = 3; // new macro: slot + 1
        assert_eq!(macro_icon(&new_macro), None);
        assert_eq!(macro_name(&[0xFF; 10]), "");
    }
}
