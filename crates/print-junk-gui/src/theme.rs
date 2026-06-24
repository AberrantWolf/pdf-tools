//! "Pressroom" theme — a cohesive visual identity for the app, drawn from the
//! print/bindery world it serves: an ink ground, paper-toned text, and spot-ink
//! accents (registration vermilion + brass furniture) instead of stock egui blue.
//!
//! This is applied once at startup over `egui`'s defaults via [`apply`].

use eframe::egui;
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, vec2};

// --- Palette: spot inks on an ink ground ------------------------------------
/// Warm near-black ground for panels.
const INK: Color32 = Color32::from_rgb(21, 20, 15);
/// Slightly raised ink for windows / popups (subtle depth against [`INK`]).
const INK_RAISED: Color32 = Color32::from_rgb(30, 28, 22);
/// Deepest ink for insets: text fields, slider troughs.
const INK_DEEP: Color32 = Color32::from_rgb(14, 13, 10);
/// Resting fill for interactive widgets (buttons, combo boxes).
const INK_WIDGET: Color32 = Color32::from_rgb(38, 36, 28);
/// Primary text — warm paper stock.
const PAPER: Color32 = Color32::from_rgb(231, 224, 207);
/// Secondary text / labels — newsprint gray.
const NEWSPRINT: Color32 = Color32::from_rgb(154, 148, 134);
/// Hairline rules and widget outlines — warm, low-contrast.
const HAIRLINE: Color32 = Color32::from_rgb(58, 55, 45);
/// Registration vermilion — the active/selected spot ink.
const VERMILION: Color32 = Color32::from_rgb(210, 59, 42);
/// A darker vermilion that paper text still reads on (selection fills).
const VERMILION_INK: Color32 = Color32::from_rgb(120, 40, 30);
/// Brass furniture — the hover/secondary highlight.
const BRASS: Color32 = Color32::from_rgb(181, 137, 60);
/// Tints for the hovered/active widget grounds.
const BRASS_INK: Color32 = Color32::from_rgb(58, 46, 22);
const VERMILION_TINT: Color32 = Color32::from_rgb(58, 26, 18);
/// Cool press blue for links/info — a counterpoint to the warm spots.
const PRESS_BLUE: Color32 = Color32::from_rgb(94, 137, 160);

/// Apply the Pressroom theme to an egui context.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();

    // --- A real type scale: size carries hierarchy (one weight available) ---
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(21.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        // Mono for measurements — tabular figures line up in columns.
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    // --- Breathing room, consistent across modules --------------------------
    style.spacing.item_spacing = vec2(8.0, 7.0);
    style.spacing.button_padding = vec2(9.0, 4.0);
    style.spacing.indent = 16.0;
    style.spacing.interact_size.y = 22.0;

    // --- Visuals: ink ground, paper text, spot-ink interaction --------------
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(PAPER);
    v.panel_fill = INK;
    v.window_fill = INK_RAISED;
    v.extreme_bg_color = INK_DEEP;
    v.faint_bg_color = Color32::from_rgb(26, 24, 19);
    v.hyperlink_color = PRESS_BLUE;
    v.warn_fg_color = BRASS;
    v.error_fg_color = VERMILION;
    v.window_stroke = Stroke::new(1.0, HAIRLINE);

    // Selection / "active pill" = registration ink.
    v.selection.bg_fill = VERMILION_INK;
    v.selection.stroke = Stroke::new(1.0, PAPER);

    // Crisp, lightly trimmed corners; drop egui's heavy default shadows.
    let radius = CornerRadius::same(3);
    v.window_corner_radius = radius;
    v.menu_corner_radius = radius;
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };

    // Widget states: resting ink → brass on hover → vermilion when active/open.
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = INK;
    w.noninteractive.weak_bg_fill = INK;
    w.noninteractive.bg_stroke = Stroke::new(1.0, HAIRLINE); // separators / rules
    w.noninteractive.fg_stroke = Stroke::new(1.0, NEWSPRINT); // muted/weak labels
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = INK_WIDGET;
    w.inactive.weak_bg_fill = INK_WIDGET;
    w.inactive.bg_stroke = Stroke::new(1.0, HAIRLINE);
    w.inactive.fg_stroke = Stroke::new(1.0, PAPER);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = BRASS_INK;
    w.hovered.weak_bg_fill = BRASS_INK;
    w.hovered.bg_stroke = Stroke::new(1.0, BRASS);
    w.hovered.fg_stroke = Stroke::new(1.0, PAPER);
    w.hovered.corner_radius = radius;

    w.active.bg_fill = VERMILION_TINT;
    w.active.weak_bg_fill = VERMILION_TINT;
    w.active.bg_stroke = Stroke::new(1.0, VERMILION);
    w.active.fg_stroke = Stroke::new(1.0, PAPER);
    w.active.corner_radius = radius;

    w.open.bg_fill = INK_WIDGET;
    w.open.weak_bg_fill = INK_WIDGET;
    w.open.bg_stroke = Stroke::new(1.0, HAIRLINE);
    w.open.fg_stroke = Stroke::new(1.0, PAPER);
    w.open.corner_radius = radius;

    style.visuals = v;
    ctx.set_global_style(style);
}
