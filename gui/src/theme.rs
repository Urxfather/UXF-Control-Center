use eframe::egui::{self, Color32, CornerRadius, Stroke, Vec2};

// Centralized Dark-Neon Theme Constants (Clevo / UXF Dark-Neon Style)
pub const BACKGROUND: Color32 = Color32::from_rgb(0x12, 0x15, 0x1b); // #12151b
pub const SURFACE: Color32 = Color32::from_rgb(0x19, 0x1e, 0x27);    // #191e27
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x22, 0x29, 0x36);
pub const SURFACE_ACTIVE: Color32 = Color32::from_rgb(0x2a, 0x33, 0x44);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x2c, 0x37);     // #262c37
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(0x3b, 0x46, 0x57);

pub const PRIMARY_ACCENT: Color32 = Color32::from_rgb(0xa3, 0xe6, 0x35); // #a3e635 Neon Lime
pub const SECONDARY_ACCENT: Color32 = Color32::from_rgb(0x06, 0xb6, 0xd4); // #06b6d4 Cyan

pub const PRIMARY_TEXT: Color32 = Color32::from_rgb(0xf1, 0xf5, 0xf9);   // #f1f5f9
pub const SECONDARY_TEXT: Color32 = Color32::from_rgb(0x94, 0xa3, 0xb8); // #94a3b8
pub const MUTED_TEXT: Color32 = Color32::from_rgb(0x64, 0x74, 0x8b);

// Spacing & Layout
pub const SPACING_XS: f32 = 4.0;
pub const SPACING_SM: f32 = 8.0;
pub const SPACING_MD: f32 = 12.0;
pub const SPACING_LG: f32 = 16.0;
pub const SPACING_XL: f32 = 24.0;

// Corner Radii
pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 12.0;

pub const ITEM_SPACING: Vec2 = Vec2::new(SPACING_SM, SPACING_SM);
pub const BUTTON_PADDING: Vec2 = Vec2::new(14.0, 8.0);

/// Standard Dark-Neon Card Frame
#[allow(dead_code)]
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(SPACING_MD as i8))
}

/// Configures the Dark-Neon visuals for egui
pub fn configure_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.override_text_color = Some(PRIMARY_TEXT);
    visuals.window_fill = SURFACE;
    visuals.panel_fill = BACKGROUND;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = SURFACE;

    visuals.selection.bg_fill = Color32::from_rgba_premultiplied(163, 230, 53, 35);
    visuals.selection.stroke = Stroke::new(1.0_f32, PRIMARY_ACCENT);

    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, SECONDARY_TEXT);

    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, PRIMARY_TEXT);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(RADIUS_MD as u8);

    visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, PRIMARY_ACCENT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, PRIMARY_TEXT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(RADIUS_MD as u8);

    visuals.widgets.active.bg_fill = SURFACE_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, PRIMARY_ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, PRIMARY_TEXT);
    visuals.widgets.active.corner_radius = CornerRadius::same(RADIUS_MD as u8);

    visuals.widgets.open.bg_fill = SURFACE_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, BORDER_LIGHT);
    visuals.widgets.open.corner_radius = CornerRadius::same(RADIUS_MD as u8);

    visuals.window_corner_radius = CornerRadius::same(RADIUS_LG as u8);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = ITEM_SPACING;
    style.spacing.button_padding = BUTTON_PADDING;
    ctx.set_style(style);
}
