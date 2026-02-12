use bevy::prelude::*;

pub(super) const UI_WIDTH: f32 = 900.0;
pub(super) const UI_HEIGHT: f32 = 520.0;

pub(super) const OVERLAY: Color = Color::srgb(0.02, 0.02, 0.05);
pub(super) const SCREEN_BG: Color = Color::srgb(0.11, 0.12, 0.15);
pub(super) const PANEL_BG: Color = Color::srgb(0.70, 0.70, 0.66);
pub(super) const PANEL_ALT: Color = Color::srgb(0.60, 0.60, 0.56);
pub(super) const TEXT_DARK: Color = Color::srgb(0.05, 0.05, 0.05);
pub(super) const TEXT_LIGHT: Color = Color::srgb(0.93, 0.94, 0.91);
pub(super) const BUTTON_BG: Color = Color::srgb(0.73, 0.73, 0.69);
pub(super) const BUTTON_HOVER: Color = Color::srgb(0.82, 0.82, 0.79);
pub(super) const BUTTON_DISABLED: Color = Color::srgb(0.57, 0.57, 0.54);
pub(super) const BORDER_LIGHT: Color = Color::srgb(0.92, 0.92, 0.88);
pub(super) const BORDER_DARK: Color = Color::srgb(0.18, 0.18, 0.18);
pub(super) const CRT_GREEN: Color = Color::srgb(0.67, 0.44, 0.92);
pub(super) const ACCENT_PRIMARY: Color = Color::srgb(0.64, 0.43, 0.90);
pub(super) const CURSOR_TINT: Color = Color::srgb(0.87, 0.73, 1.0);
pub(super) const CURSOR_TINT_PRESSED: Color = Color::srgb(0.93, 0.82, 1.0);

pub(super) fn border(raised: bool) -> BorderColor {
    let (top_left, bottom_right) = if raised {
        (BORDER_LIGHT, BORDER_DARK)
    } else {
        (BORDER_DARK, BORDER_LIGHT)
    };
    BorderColor {
        top: top_left,
        left: top_left,
        right: bottom_right,
        bottom: bottom_right,
    }
}
