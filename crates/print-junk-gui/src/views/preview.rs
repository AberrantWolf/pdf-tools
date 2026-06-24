//! Shared live-preview pane used by every mode that renders a PDF preview.
//!
//! Each mode owns an `Option<ViewerState>`; this pane shows it through the common
//! [`show_viewer`](super::show_viewer) widget — so scroll/page/zoom preservation
//! across re-renders is shared, not re-implemented per mode.
//!
//! The pane reads as a **press bed**: a deep-ink ground the proof sits on, with a
//! printer's slug above it and, when empty, a blank-sheet placeholder. It avoids
//! any prepress-mark motif (crop/registration/fold marks), since those are real
//! output the imposition mode produces — chrome must not be mistaken for output.

use eframe::egui;
use egui::{Color32, Rect, Stroke, StrokeKind, vec2};
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

use super::{ViewerState, show_viewer};
use crate::theme::{HAIRLINE, INK_DEEP, NEWSPRINT, VERMILION};

/// Render the central preview area for a mode.
///
/// - `viewer`: the mode's preview viewer; when `Some`, it is shown via [`show_viewer`].
/// - `overlay`: an optional status "slug" drawn above the preview (e.g. "N cards · …").
/// - `placeholder`: copy shown (centered, on the blank sheet) when there's nothing to preview.
pub fn show_preview_pane(
    ui: &mut egui::Ui,
    viewer: &mut Option<ViewerState>,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
    overlay: Option<String>,
    placeholder: impl FnOnce(&mut egui::Ui),
) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(INK_DEEP))
        .show_inside(ui, |ui| {
            if viewer.is_some() {
                if let Some(text) = overlay {
                    proof_slug(ui, &text);
                }
                show_viewer(ui, viewer, command_tx);
            } else {
                // A blank sheet sitting on the press bed — clearly "your page goes
                // here", not a printer's mark.
                let sheet = blank_sheet_rect(ui.max_rect());
                ui.painter()
                    .rect_filled(sheet, 2.0, Color32::from_rgb(30, 28, 21));
                ui.painter().rect_stroke(
                    sheet,
                    2.0,
                    Stroke::new(1.0, HAIRLINE),
                    StrokeKind::Inside,
                );
                // The mode's own invitation copy, centered on the sheet itself.
                ui.scope_builder(egui::UiBuilder::new().max_rect(sheet), |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(placeholder);
                    });
                });
            }
        });
}

/// A status line styled as a printer's slug: a registration tick + muted metadata.
fn proof_slug(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        ui.label(egui::RichText::new("▌").color(VERMILION));
        ui.label(egui::RichText::new(text).color(NEWSPRINT).monospace());
    });
}

/// The portrait sheet rectangle (8.5 × 11 proportion) centered on the bed.
fn blank_sheet_rect(bed: Rect) -> Rect {
    let height = (bed.height() * 0.58).clamp(120.0, (bed.height() - 96.0).max(120.0));
    let width = (height * 8.5 / 11.0).min(bed.width() * 0.66);
    Rect::from_center_size(bed.center(), vec2(width, height))
}
