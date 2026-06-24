use eframe::egui;

use super::state::ImposeState;
use crate::ui_components::section;

pub fn show(ui: &mut egui::Ui, state: &ImposeState) {
    section(ui, "imp_stats_sec", "Statistics", true, |ui| {
        if let Some(stats) = &state.stats {
            ui.label(format!("Source pages: {}", stats.source_pages));
            ui.label(format!("Output sheets: {}", stats.output_sheets));
            ui.label(format!("Output pages: {}", stats.output_pages));

            if stats.blank_pages_added > 0 {
                ui.label(format!("Blank pages added: {}", stats.blank_pages_added));
            }

            if let Some(sig_count) = stats.signatures {
                ui.label(format!("Number of signatures: {sig_count}"));
            }

            if let Some(cells) = stats.cascade_cells_per_sheet {
                ui.label(format!("Cascade cells per sheet: {cells}"));
            }

            if let Some(ref pages_per_sig) = stats.pages_per_signature
                && !pages_per_sig.is_empty()
            {
                let pages_display = format_pages_per_signature(pages_per_sig);
                ui.label(format!("Pages per signature: {pages_display}"));
            }

            for warning in &stats.warnings {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 200, 80),
                    format!("⚠ {warning}"),
                );
            }
        } else {
            ui.label("No statistics available");
            ui.label("Add input files and configure options to see statistics");
        }
    });
}

fn format_pages_per_signature(pages_per_sig: &[usize]) -> String {
    if pages_per_sig.iter().all(|&p| p == pages_per_sig[0]) {
        format!("{} pages each", pages_per_sig[0])
    } else {
        format!("{pages_per_sig:?}")
    }
}
