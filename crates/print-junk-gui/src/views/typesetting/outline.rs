//! The document-structure rail for imported documents: one row per section with
//! an include toggle and a "start on a new page" toggle. Overrides are applied
//! as a cheap string pass over the cached conversion at compile time (see
//! `pdf_typeset::assemble_body`), so toggling never re-runs the import.

use eframe::egui;
use pdf_async_runtime::{FRONT_MATTER_ID, SectionOverrides};

use super::TypesettingState;

/// Show the "Structure" rail between the controls panel and the preview when an
/// imported document with headings is active. Toggles flag the preview for
/// regeneration; the overrides live in the import session, so they persist and
/// are re-applied after the offline re-conversion on restore.
pub(super) fn show_outline_rail(ui: &mut egui::Ui, state: &mut TypesettingState) {
    let Some(import) = &mut state.import else {
        return;
    };
    let Some(conv) = &import.converted else {
        return;
    };
    if conv.outline.is_empty() {
        return;
    }
    let outline = conv.outline.clone();
    let overrides = &mut import.overrides;
    let mut changed = false;

    egui::Panel::left("typesetting_outline")
        .min_size(230.0)
        .show_inside(ui, |ui| {
            ui.heading("Structure");
            ui.label(
                egui::RichText::new("Choose the sections to include; ⤵ starts one on a new page.")
                    .small()
                    .weak(),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("typesetting_outline_scroll")
                .show(ui, |ui| {
                    // Content before the first heading: authors, abstract preamble…
                    if outline[0].offset > 0 {
                        changed |= section_row(
                            ui,
                            overrides,
                            FRONT_MATTER_ID,
                            1,
                            "(front matter)",
                            false,
                            true,
                        );
                    }
                    // A hidden section swallows its subtree, so descendant rows are
                    // disabled until the next heading at or above the hidden level.
                    let mut hidden_at: Option<u8> = None;
                    for entry in outline.iter() {
                        if hidden_at.is_some_and(|l| entry.level <= l) {
                            hidden_at = None;
                        }
                        let inside_hidden = hidden_at.is_some();
                        changed |= section_row(
                            ui,
                            overrides,
                            &entry.id,
                            entry.level,
                            &entry.title,
                            true,
                            !inside_hidden,
                        );
                        if !inside_hidden && overrides.get(&entry.id).is_some_and(|ov| ov.hidden) {
                            hidden_at = Some(entry.level);
                        }
                    }
                });
        });

    if changed {
        state.needs_regeneration = true;
    }
}

/// One outline row: include checkbox, optional page-break toggle, indented
/// title. Only non-default overrides are stored, keeping the persisted map
/// minimal. Returns whether anything changed.
fn section_row(
    ui: &mut egui::Ui,
    overrides: &mut SectionOverrides,
    id: &str,
    level: u8,
    title: &str,
    allow_break: bool,
    enabled: bool,
) -> bool {
    let mut ov = overrides.get(id).copied().unwrap_or_default();
    let mut changed = false;

    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(f32::from(level.saturating_sub(1)) * 12.0);

            let mut include = !ov.hidden;
            if ui
                .checkbox(&mut include, "")
                .on_hover_text("Include this section (and its subsections) in the output")
                .changed()
            {
                ov.hidden = !include;
                changed = true;
            }

            if allow_break
                && ui
                    .toggle_value(&mut ov.break_before, "⤵")
                    .on_hover_text("Start this section on a new page")
                    .changed()
            {
                changed = true;
            }

            let display = if title.is_empty() {
                "(untitled)"
            } else {
                title
            };
            let mut text = egui::RichText::new(truncated(display));
            if ov.hidden {
                text = text.weak().strikethrough();
            }
            ui.label(text).on_hover_text(display);
        });
    });

    if changed {
        if ov.is_default() {
            overrides.remove(id);
        } else {
            overrides.insert(id.to_string(), ov);
        }
    }
    changed
}

/// Cap a title for the rail; the full text is in the hover.
fn truncated(s: &str) -> String {
    const MAX: usize = 34;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let cut: String = s.chars().take(MAX - 1).collect();
        format!("{cut}…")
    }
}
