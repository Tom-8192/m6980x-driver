//! Macro icons: own pictures 1-40 (made with an image AI, no Gigabyte material, see `assets/icons/`) and number
//! tiles 41-100 drawn on an empty frame. Built from `assets/icons/make_icons.py`, embedded into the binary.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Rect, TextureHandle, TextureOptions, pos2, vec2};
use m6980x_core::rom::ICONS_EMBEDDED;

macro_rules! icon_files {
    ($($n:literal)*) => {
        [$(include_bytes!(concat!("../../../assets/icons/macro/", $n, ".webp")) as &[u8]),*]
    };
}

/// 0 = empty slot (honeycomb), 1-40 = pictures, last = empty frame for the number tiles
const FILES: [&[u8]; 42] = icon_files!(0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27
    28 29 30 31 32 33 34 35 36 37 38 39 40 "frame");
const EMPTY: usize = 0;
const FRAME: usize = 41;

/// Textures of the icons, decoded on first use.
#[derive(Default)]
pub struct MacroIcons {
    textures: Vec<TextureHandle>,
}

impl MacroIcons {
    fn texture(&mut self, ctx: &egui::Context, index: usize) -> &TextureHandle {
        if self.textures.is_empty() {
            self.textures = FILES
                .iter()
                .enumerate()
                .map(|(i, bytes)| {
                    let image = image::load_from_memory(bytes).expect("embedded icon").to_rgba8();
                    let size = [image.width() as usize, image.height() as usize];
                    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                    ctx.load_texture(format!("macro_icon_{i}"), color, TextureOptions::LINEAR)
                })
                .collect();
        }
        &self.textures[index]
    }

    /// Draws the icon into `rect`: pictures 1-40, frame with the number for 41-100, empty slot for None.
    pub fn paint(&mut self, painter: &Painter, rect: Rect, icon: Option<u8>) {
        let index = match icon {
            Some(n) if (1..=ICONS_EMBEDDED).contains(&n) => usize::from(n),
            Some(_) => FRAME,
            None => EMPTY,
        };
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let id = self.texture(painter.ctx(), index).id();
        painter.image(id, rect, uv, Color32::WHITE);
        if let Some(n) = icon.filter(|&n| n > ICONS_EMBEDDED) {
            paint_number(painter, rect, n);
        }
    }
}

/// Silver number in the frame's inner field: dark drop shadow, light upper edge, slightly widened main layer.
fn paint_number(painter: &Painter, rect: Rect, n: u8) {
    let text = n.to_string();
    let side = rect.width();
    let font = FontId::proportional(side * if text.len() > 2 { 0.36 } else { 0.44 });
    let center = rect.center() + vec2(0.0, side * 0.01);
    let shadow = (side / 40.0).max(1.0);
    let layer = |offset: egui::Vec2, color: Color32| {
        painter.text(center + offset, Align2::CENTER_CENTER, &text, font.clone(), color);
    };
    layer(vec2(shadow, shadow), Color32::from_black_alpha(170));
    layer(vec2(0.0, -0.6), Color32::from_white_alpha(45));
    let silver = Color32::from_rgb(0xD9, 0xE1, 0xEA);
    let bold = (side / 100.0).max(0.3);
    layer(vec2(-bold, 0.0), silver);
    layer(vec2(bold, 0.0), silver);
}

/// State of the open icon dialog.
pub struct IconPicker {
    /// icon chosen so far (applied with "Apply" or a double click)
    pub draft: Option<u8>,
    /// scroll the chosen tile into view (first frame after opening)
    pub scroll: bool,
}

impl IconPicker {
    pub fn open(current: Option<u8>) -> Self {
        Self { draft: current, scroll: true }
    }
}
