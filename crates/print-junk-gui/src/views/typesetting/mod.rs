//! Typesetting mode: lay out a text file (Plaintext / Markdown / HTML) into a
//! typeset PDF, with a live preview. Desktop-only (Typst is a native dependency).

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use pdf_async_runtime::{
    ArchiveStatus, AssetReport, PdfCommand, SectionOverrides, SharedAssets, SharedOutline,
};
use pdf_typeset::{
    BreakPosition, Color, HAlign, ImportStats, InputFormat, PageBreakRule, TableBorder,
    TypesetConfig, TypesetInput, available_font_families,
};
use pdf_units::Orientation;
use tokio::sync::mpsc;

mod outline;

use super::ViewerState;
use crate::ui_components::{
    STANDARD_PAPER_SIZES, enum_combo, enum_selector, form, form_row, form_row_info, num_field,
    section, section_heading,
};

/// A document imported from a URL / arXiv id / file. The raw HTML and the assets
/// fetched during conversion are persisted (so restore is offline and re-applies
/// importer improvements); the converted Typst artifact is cached in-memory only,
/// for cheap recompiles on settings changes.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ImportSession {
    /// What the user imported (URL / arXiv id / path) — shown and re-importable.
    pub source: String,
    /// The fetched raw payload (HTML or Markdown per [`ImportSession::kind`]),
    /// re-converted on restore.
    #[serde(alias = "html")]
    pub payload: String,
    /// Which importer the payload feeds. Defaults to HTML so pre-`kind` saves
    /// (all of which were URL/arXiv HTML imports) restore correctly.
    #[serde(default = "default_import_kind")]
    pub kind: InputFormat,
    /// Assets fetched during conversion, keyed by `<img src>` (base64 in the save).
    #[serde(with = "asset_base64")]
    pub raw_assets: Vec<(String, Vec<u8>)>,
    /// Per-section hide / page-break overrides, keyed by stable section id —
    /// persisted, and re-applied after the offline re-conversion on restore.
    pub overrides: SectionOverrides,
    /// What the asset pipeline did at import time (source archive, figure
    /// upgrades) — persisted so the status row survives a restore.
    pub asset_report: Option<AssetReport>,
    /// In-memory converted artifact (Typst body + assets + stats); not persisted.
    #[serde(skip)]
    pub converted: Option<ConvertedImport>,
    /// Set once a reconvert has been requested on restore, so it isn't re-sent
    /// every frame while the worker is busy.
    #[serde(skip)]
    pub reconvert_requested: bool,
}

/// Back-compat default for [`ImportSession::kind`]: saves written before the
/// field existed were all HTML (URL/arXiv) imports.
fn default_import_kind() -> InputFormat {
    InputFormat::Html
}

/// The converted, ready-to-compile form of an import, cached in memory. `Arc`s
/// let recompile commands share the body/assets without copying.
#[derive(Clone)]
pub struct ConvertedImport {
    pub body: Arc<String>,
    pub assets: SharedAssets,
    pub outline: SharedOutline,
    pub title: Option<String>,
    pub stats: ImportStats,
}

/// Serde for `raw_assets`: each asset's bytes are base64-encoded so the embedded
/// cache stays compact text in both the `.pjproj` (JSON) and eframe auto-storage.
mod asset_base64 {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize as _, Deserializer, Serialize as _, Serializer};

    pub fn serialize<S: Serializer>(assets: &[(String, Vec<u8>)], s: S) -> Result<S::Ok, S::Error> {
        let encoded: Vec<(&str, String)> = assets
            .iter()
            .map(|(name, bytes)| (name.as_str(), STANDARD.encode(bytes)))
            .collect();
        encoded.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<(String, Vec<u8>)>, D::Error> {
        Vec::<(String, String)>::deserialize(d)?
            .into_iter()
            .map(|(name, b64)| {
                STANDARD
                    .decode(b64)
                    .map(|bytes| (name, bytes))
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

/// UI state for the typesetting mode.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TypesettingState {
    pub source_path: Option<PathBuf>,
    /// Document content (re-loaded from `source_path`; not persisted).
    #[serde(skip)]
    pub source_text: String,
    pub format: InputFormat,
    pub config: TypesetConfig,

    /// An imported document, when the source is a fetched URL/arXiv/HTML rather
    /// than the editable text above. Persisted (raw HTML + assets) so it restores
    /// offline.
    pub import: Option<ImportSession>,
    /// The URL / arXiv id / path typed into the import field (not persisted).
    #[serde(skip)]
    pub import_input: String,
    /// True while an import fetch/convert is in flight.
    #[serde(skip)]
    pub importing: bool,
    /// The last import error, shown beneath the import field.
    #[serde(skip)]
    pub import_error: Option<String>,

    /// Installed font families, for the font pickers (re-enumerated; not persisted).
    #[serde(skip)]
    pub available_fonts: Vec<String>,

    /// Which heading level (1..=6) the headings section is currently editing.
    #[serde(skip)]
    pub heading_edit_level: u8,

    #[serde(skip)]
    pub preview_viewer: Option<ViewerState>,
    #[serde(skip)]
    pub needs_regeneration: bool,
    #[serde(skip)]
    pub preview_page_count: usize,
}

impl Default for TypesettingState {
    fn default() -> Self {
        Self {
            source_path: None,
            source_text: String::new(),
            format: InputFormat::Markdown,
            config: TypesetConfig::default(),
            import: None,
            import_input: String::new(),
            importing: false,
            import_error: None,
            available_fonts: available_font_families(),
            heading_edit_level: 1,
            preview_viewer: None,
            needs_regeneration: false,
            preview_page_count: 0,
        }
    }
}

impl TypesettingState {
    fn to_input(&self) -> TypesetInput {
        TypesetInput {
            text: self.source_text.clone(),
            format: self.format,
        }
    }

    /// Prompt for a source file and load it. Shared by the "Open..." button and
    /// the Cmd+O shortcut.
    pub fn open_file_dialog(&mut self, command_tx: &mpsc::UnboundedSender<PdfCommand>) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Text documents",
                &[
                    "md", "markdown", "mdown", "mkd", "txt", "text", "html", "htm", "xhtml",
                ],
            )
            .pick_file()
        {
            self.load_source_file(path, command_tx);
        }
    }

    /// Load a source file by extension. Markdown/HTML documents are *imported*
    /// (read-only, with resolved images and the Structure rail), exactly like a
    /// URL/arXiv import; plain text stays editable. Shared by the Open dialog, the
    /// Open button, and drag-and-drop.
    pub fn load_source_file(
        &mut self,
        path: PathBuf,
        command_tx: &mpsc::UnboundedSender<PdfCommand>,
    ) {
        let fmt = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(InputFormat::from_extension);
        match fmt {
            Some(InputFormat::Markdown | InputFormat::Html) => {
                // Route through the import pipeline. Clearing the editable buffer
                // stops `maybe_regenerate` from firing a stale text preview while
                // the import is in flight.
                self.source_text.clear();
                self.source_path = Some(path.clone());
                self.importing = true;
                self.import_error = None;
                let _ = command_tx.send(PdfCommand::TypesetImport {
                    source: path.to_string_lossy().into_owned(),
                    config: self.config.clone(),
                });
            }
            _ => match std::fs::read_to_string(&path) {
                Ok(text) => {
                    if let Some(f) = fmt {
                        self.format = f;
                    }
                    self.import = None;
                    self.source_text = text;
                    self.source_path = Some(path);
                    self.needs_regeneration = true;
                }
                Err(e) => log::error!("Failed to read {}: {e}", path.display()),
            },
        }
    }
}

pub fn show_typesetting(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::Panel::left("typesetting_controls")
        .min_size(320.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Typesetting");
                ui.add_space(2.0);

                source_section(ui, state, command_tx);
                page_section(ui, state);
                margins_section(ui, state);
                fonts_section(ui, state);
                headings_section(ui, state);
                spacing_section(ui, state);
                tables_section(ui, state);
                page_breaks_section(ui, state);
                document_section(ui, state);
                actions_section(ui, state, command_tx);

                // Regenerate the preview once per frame, AFTER every section has
                // had a chance to flag a change. This must run last — if it ran
                // earlier, changes made by later sections (fonts, margins, …)
                // would be missed until some future frame.
                maybe_regenerate(state, command_tx);
            });
        });

    // The document outline rail (imported documents only) sits between the
    // controls and the preview. Shown after `maybe_regenerate` ran for this
    // frame, so its toggles surface on the next frame's regenerate pass.
    outline::show_outline_rail(ui, state);

    let page_count = state.preview_page_count;
    let has_content = !state.source_text.trim().is_empty() || state.import.is_some();
    let overlay = (page_count > 0).then(|| format!("Preview: {page_count} page(s)"));
    super::preview::show_preview_pane(ui, &mut state.preview_viewer, command_tx, overlay, |ui| {
        if has_content {
            ui.heading("Ready to Typeset");
            ui.label("Adjust settings to update the preview");
        } else {
            ui.heading("No Document");
            ui.label("Open a file, or import from a URL / arXiv id");
        }
    });
}

// =============================================================================
// Sections
// =============================================================================

fn source_section(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    section(ui, "ts_source_sec", "Source", true, |ui| {
        import_row(ui, state, command_tx);
        if state.import.is_some() {
            imported_source(ui, state, command_tx);
        } else {
            editable_source(ui, state, command_tx);
        }
    });
}

/// The "import a document" control: a URL/arXiv/path field plus a button, shared
/// by both the text and imported modes so an import can always be started.
fn import_row(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.label("Import a document:");
    let mut go = false;
    ui.horizontal(|ui| {
        let field = ui.add(
            egui::TextEdit::singleline(&mut state.import_input)
                .desired_width(180.0)
                .hint_text("URL, arXiv id, or file"),
        );
        let ready = !state.import_input.trim().is_empty() && !state.importing;
        go = (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && ready)
            || (ui.add_enabled(ready, egui::Button::new("Import")).clicked());
    });
    if go {
        state.importing = true;
        state.import_error = None;
        let _ = command_tx.send(PdfCommand::TypesetImport {
            source: state.import_input.trim().to_string(),
            config: state.config.clone(),
        });
    }
    if state.importing {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Importing…");
        });
    }
    if let Some(err) = &state.import_error {
        ui.colored_label(egui::Color32::from_rgb(200, 60, 60), err);
    }
}

/// Source UI when a document has been imported: a summary + stats, the asset
/// pipeline's status (with a retry for network failures), plus a button to
/// discard the import and return to editing text.
fn imported_source(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let Some(import) = &state.import else { return };
    ui.add_space(4.0);
    let source = import.source.clone();
    if let Some(conv) = &import.converted {
        if let Some(title) = &conv.title {
            ui.label(egui::RichText::new(title).strong());
        }
        ui.label(
            egui::RichText::new(format!(
                "Imported from {source} — {}",
                import_stats_summary(&conv.stats)
            ))
            .small()
            .weak(),
        );
    } else {
        ui.label(
            egui::RichText::new(format!("Imported from {source} (preparing…)"))
                .small()
                .weak(),
        );
    }

    // A failed source-archive fetch (arXiv hi-res figures) offers a retry.
    let mut reimport = import
        .asset_report
        .as_ref()
        .is_some_and(|report| asset_status_row(ui, report));

    let mut clear = false;
    ui.horizontal(|ui| {
        reimport |= ui
            .button("⟳ Reload")
            .on_hover_text("Re-import from the source (picks up edits to the file)")
            .clicked();
        clear = ui.button("✖ Clear import").clicked();
    });

    // `import`'s borrow has ended; apply the chosen action.
    if reimport {
        state.importing = true;
        state.import_error = None;
        let _ = command_tx.send(PdfCommand::TypesetImport {
            source,
            config: state.config.clone(),
        });
    } else if clear {
        state.import = None;
        state.preview_page_count = 0;
    }
}

/// One-line import summary: math tiers (only when present), figures (with any
/// unresolved count), and citations (only when present).
fn import_stats_summary(s: &ImportStats) -> String {
    let mut parts = Vec::new();
    if s.math_tex + s.math_image + s.math_raw > 0 {
        parts.push(format!(
            "math {} native/{} image/{} raw",
            s.math_tex, s.math_image, s.math_raw
        ));
    }
    parts.push(if s.images_failed > 0 {
        format!("{} figures ({} unresolved)", s.images_ok, s.images_failed)
    } else {
        format!("{} figures", s.images_ok)
    });
    if s.citations > 0 {
        parts.push(format!("{} citations", s.citations));
    }
    parts.join(", ")
}

/// One-line status of the hi-res figure pipeline, with explanatory tooltips.
/// Returns `true` when the user asked to retry a failed source-archive fetch.
fn asset_status_row(ui: &mut egui::Ui, report: &AssetReport) -> bool {
    let mut retry = false;
    ui.horizontal(|ui| {
        match &report.archive {
            // Non-arXiv sources have no e-print archive — nothing to report.
            ArchiveStatus::NotApplicable => {}
            ArchiveStatus::Disabled => {
                ui.label(
                    egui::RichText::new("⛶ hi-res figures unavailable")
                        .small()
                        .weak(),
                )
                .on_hover_text(
                    "This build has no hi-res figure support \
                         (the hires-import feature was disabled).",
                );
            }
            ArchiveStatus::Fetched {
                files,
                vector_figures,
            } => {
                let (text, color) = if report.figures_upgraded > 0 {
                    (
                        format!(
                            "⛶ {} figure(s) at print resolution",
                            report.figures_upgraded
                        ),
                        egui::Color32::from_rgb(110, 190, 120),
                    )
                } else {
                    (
                        "⛶ no upgradable figures".to_string(),
                        ui.visuals().weak_text_color(),
                    )
                };
                ui.colored_label(color, egui::RichText::new(text).small())
                    .on_hover_text(format!(
                        "LaTeX source fetched ({files} files, {vector_figures} vector \
                         figure(s)); {} re-rasterized at print resolution.\n\
                         Figures that are bitmaps in the source (photos, screenshots) \
                         have no higher-resolution original to use.",
                        report.figures_upgraded
                    ));
            }
            ArchiveStatus::Failed(e) => {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 170, 80),
                    egui::RichText::new("⛶ ⚠ figures at web resolution").small(),
                )
                .on_hover_text(format!(
                    "The LaTeX source couldn't be fetched, so vector figures keep \
                     the HTML's preview resolution:\n{e}"
                ));
                if ui
                    .small_button("⟳ Retry")
                    .on_hover_text("Re-import and try fetching the source archive again")
                    .clicked()
                {
                    retry = true;
                }
            }
        }
    });
    retry
}

/// Source UI for the editable text path: open a file, pick a format, edit text.
fn editable_source(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.add_space(4.0);
    ui.label("Source document:");
    ui.horizontal(|ui| {
        let name = state
            .source_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| "(untitled)".to_string(), |n| n.to_string_lossy().into());
        ui.label(name);
        if ui.button("Open...").clicked() {
            state.open_file_dialog(command_tx);
        }
    });

    let formats = [
        (InputFormat::Markdown, "Markdown"),
        (InputFormat::Plaintext, "Plain text"),
        (InputFormat::Html, "HTML"),
    ];
    if enum_selector(ui, "ts_format", "Format:", &mut state.format, &formats) {
        state.needs_regeneration = true;
    }

    ui.add_space(4.0);
    ui.label("Content (editable):");
    let edited = ui
        .add(
            egui::TextEdit::multiline(&mut state.source_text)
                .desired_rows(6)
                .desired_width(f32::INFINITY)
                .code_editor(),
        )
        .changed();
    if edited {
        state.needs_regeneration = true;
    }
}

/// Send a fresh preview request if any setting changed this frame. Call once,
/// after all sections, so no section's change is missed. Routes to the import
/// recompile/reconvert path when a document is imported, else the text path.
fn maybe_regenerate(state: &mut TypesettingState, command_tx: &mpsc::UnboundedSender<PdfCommand>) {
    if let Some(import) = &mut state.import {
        match &import.converted {
            // Settings change with the converted artifact in hand: cheap recompile.
            Some(conv) if state.needs_regeneration => {
                state.needs_regeneration = false;
                let _ = command_tx.send(PdfCommand::TypesetCompileImported {
                    body: conv.body.clone(),
                    assets: conv.assets.clone(),
                    outline: conv.outline.clone(),
                    overrides: import.overrides.clone(),
                    config: state.config.clone(),
                });
            }
            // Restored from disk: re-convert the cached raw payload once (offline).
            None if !import.reconvert_requested => {
                import.reconvert_requested = true;
                state.needs_regeneration = false;
                let _ = command_tx.send(PdfCommand::TypesetReconvert {
                    payload: Arc::new(import.payload.clone()),
                    kind: import.kind,
                    raw_assets: Arc::new(import.raw_assets.clone()),
                    overrides: import.overrides.clone(),
                    config: state.config.clone(),
                });
            }
            _ => {}
        }
    } else if state.needs_regeneration && !state.source_text.trim().is_empty() {
        state.needs_regeneration = false;
        let _ = command_tx.send(PdfCommand::TypesetGeneratePreview {
            input: state.to_input(),
            config: state.config.clone(),
        });
    }
}

fn page_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    let c = &mut state.config;
    let mut changed = false;
    let orientations = [
        (Orientation::Portrait, "Portrait"),
        (Orientation::Landscape, "Landscape"),
    ];
    section(ui, "ts_page_sec", "Page", true, |ui| {
        form(ui, "ts_page", |ui| {
            changed |= form_row(ui, "Page size", |ui| {
                enum_combo(ui, "ts_paper", &mut c.page_size, &STANDARD_PAPER_SIZES)
            });
            changed |= form_row(ui, "Orientation", |ui| {
                enum_combo(ui, "ts_orientation", &mut c.orientation, &orientations)
            });
            changed |= form_row(ui, "Page numbers", |ui| {
                ui.checkbox(&mut c.page_numbers, "").changed()
            });
        });
    });
    if changed {
        state.needs_regeneration = true;
    }
}

fn margins_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    let m = &mut state.config;
    let mut changed = false;
    let mm = |ui: &mut egui::Ui, value: &mut f32| num_field(ui, value, 0.0..=80.0, " mm");
    section(ui, "ts_margins_sec", "Margins", true, |ui| {
        form(ui, "ts_margins", |ui| {
            changed |= form_row(ui, "Top", |ui| mm(ui, &mut m.margin_top_mm));
            changed |= form_row(ui, "Bottom", |ui| mm(ui, &mut m.margin_bottom_mm));
            changed |= form_row_info(ui, "Spine", "Inner margin, at the binding edge", |ui| {
                mm(ui, &mut m.margin_inner_mm)
            });
            changed |= form_row_info(
                ui,
                "Fore-edge",
                "Outer margin, at the front (cut) edge",
                |ui| mm(ui, &mut m.margin_outer_mm),
            );
        });
    });
    if changed {
        state.needs_regeneration = true;
    }
}

fn fonts_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    let mut changed = false;
    section(ui, "ts_body_sec", "Body text", true, |ui| {
        form(ui, "ts_body", |ui| {
            changed |= form_row(ui, "Font", |ui| {
                font_combo(
                    ui,
                    "ts_body_font",
                    &mut state.config.body_font.family,
                    &state.available_fonts,
                )
            });
            changed |= form_row(ui, "Size", |ui| {
                num_field(ui, &mut state.config.body_font.size_pt, 5.0..=32.0, " pt")
            });
            changed |= form_row(ui, "Color", |ui| {
                color_field(ui, &mut state.config.body_color)
            });
            changed |= form_row(ui, "Link color", |ui| {
                color_field(ui, &mut state.config.link_color)
            });
            changed |= form_row(ui, "Code color", |ui| {
                color_field(ui, &mut state.config.code_color)
            });
            changed |= form_row(ui, "Code bg", |ui| {
                optional_color_field(
                    ui,
                    &mut state.config.code_background,
                    Color::new(244, 244, 244),
                )
            });
        });
    });
    if changed {
        state.needs_regeneration = true;
    }
}

/// Per-level heading editor: pick a level, then edit its style. Only the
/// selected level's controls are shown to keep the panel compact.
fn headings_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    // Clamp first — a skipped/zeroed value on restore must not underflow below.
    state.heading_edit_level = state.heading_edit_level.clamp(1, 6);
    let mut changed = false;
    let aligns = [
        (HAlign::Left, "Left"),
        (HAlign::Center, "Center"),
        (HAlign::Right, "Right"),
    ];

    section(ui, "ts_heading_sec", "Headings", false, |ui| {
        ui.horizontal(|ui| {
            for lvl in 1..=6u8 {
                if ui
                    .selectable_label(state.heading_edit_level == lvl, format!("H{lvl}"))
                    .clicked()
                {
                    state.heading_edit_level = lvl;
                }
            }
        });
        ui.add_space(2.0);

        let lvl = state.heading_edit_level;
        let available = &state.available_fonts;
        let st = &mut state.config.heading_styles[usize::from(lvl - 1)];

        form(ui, "ts_heading", |ui| {
            changed |= form_row(ui, "Font", |ui| {
                font_combo(ui, "ts_heading_font", &mut st.family, available)
            });
            changed |= form_row(ui, "Size", |ui| {
                num_field(ui, &mut st.size_pt, 6.0..=72.0, " pt")
            });
            changed |= form_row(ui, "Style", |ui| {
                // One cell (see `optional_color_field`): two cells would add a
                // third grid column and trigger the combo width feedback loop.
                ui.horizontal(|ui| {
                    let mut c = ui.checkbox(&mut st.bold, "Bold").changed();
                    c |= ui.checkbox(&mut st.italic, "Italic").changed();
                    c
                })
                .inner
            });
            changed |= form_row(ui, "Color", |ui| color_field(ui, &mut st.color));
            changed |= form_row(ui, "Align", |ui| {
                enum_combo(ui, "ts_heading_align", &mut st.align, &aligns)
            });
            changed |= form_row(ui, "Space above", |ui| {
                num_field(ui, &mut st.space_above_mm, 0.0..=40.0, " mm")
            });
            changed |= form_row(ui, "Space below", |ui| {
                num_field(ui, &mut st.space_below_mm, 0.0..=40.0, " mm")
            });
            changed |= form_row_info(
                ui,
                "New page",
                "Start this heading on a new page (e.g. chapters)",
                |ui| ui.checkbox(&mut st.start_new_page, "").changed(),
            );
        });
    });

    if changed {
        state.needs_regeneration = true;
    }
}

fn tables_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    let t = &mut state.config.table;
    let mut changed = false;
    let borders = [
        (TableBorder::All, "All"),
        (TableBorder::Horizontal, "Horizontal"),
        (TableBorder::None, "None"),
    ];

    section(ui, "ts_table_sec", "Tables", false, |ui| {
        form(ui, "ts_table", |ui| {
            changed |= form_row(ui, "Bold header", |ui| {
                ui.checkbox(&mut t.header_bold, "").changed()
            });
            changed |= form_row(ui, "Header fill", |ui| {
                optional_color_field(ui, &mut t.header_fill, Color::new(230, 230, 230))
            });
            changed |= form_row_info(ui, "Zebra", "Shade alternating rows", |ui| {
                optional_color_field(ui, &mut t.zebra_fill, Color::new(244, 244, 244))
            });
            changed |= form_row(ui, "Borders", |ui| {
                enum_combo(ui, "ts_table_border", &mut t.border, &borders)
            });
            changed |= form_row(ui, "Border width", |ui| {
                num_field(ui, &mut t.border_width_pt, 0.0..=3.0, " pt")
            });
            changed |= form_row(ui, "Border color", |ui| {
                color_field(ui, &mut t.border_color)
            });
            changed |= form_row(ui, "Cell padding", |ui| {
                num_field(ui, &mut t.cell_padding_mm, 0.0..=8.0, " mm")
            });
        });
    });

    if changed {
        state.needs_regeneration = true;
    }
}

fn document_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    // Smart punctuation is baked in at conversion time; for an imported document
    // that conversion is cached, so the toggle would be inert — hide it there.
    let imported = state.import.is_some();
    let c = &mut state.config;
    let mut changed = false;

    section(ui, "ts_doc_sec", "Document & front matter", false, |ui| {
        form(ui, "ts_doc", |ui| {
            changed |= form_row(ui, "Title", |ui| text_field(ui, &mut c.doc_title, "(none)"));
            changed |= form_row(ui, "Author", |ui| text_field(ui, &mut c.doc_author, ""));
            changed |= form_row(ui, "Keywords", |ui| {
                text_field(ui, &mut c.doc_keywords, "comma, separated")
            });
            changed |= form_row_info(
                ui,
                "Language",
                "BCP-47 code; controls hyphenation and quotes",
                |ui| text_field(ui, &mut c.lang, "en"),
            );
            changed |= form_row(ui, "Contents", |ui| {
                ui.checkbox(&mut c.generate_toc, "").changed()
            });
            if c.generate_toc {
                changed |= form_row(ui, "TOC depth", |ui| {
                    num_field(ui, &mut c.toc_depth, 1..=6, "")
                });
            }
            if !imported {
                changed |= form_row_info(
                    ui,
                    "Smart quotes",
                    "Curly quotes and typographic dashes",
                    |ui| ui.checkbox(&mut c.smart_punctuation, "").changed(),
                );
            }
        });

        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("A title page is added when a title is set.")
                .small()
                .weak(),
        );
    });

    if changed {
        state.needs_regeneration = true;
    }
}

/// Control-only single-line text field filling the row width, for a [`form_row`].
fn text_field(ui: &mut egui::Ui, value: &mut String, hint: &str) -> bool {
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(ui.available_width())
            .hint_text(hint),
    )
    .changed()
}

/// Control-only sRGB color swatch (no label) for a [`form_row`] control.
fn color_field(ui: &mut egui::Ui, color: &mut Color) -> bool {
    let mut rgb = [color.r, color.g, color.b];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        *color = Color::new(rgb[0], rgb[1], rgb[2]);
        true
    } else {
        false
    }
}

/// Control-only optional color: an enable checkbox, plus a swatch when on.
/// `default` seeds the color when first enabled.
fn optional_color_field(ui: &mut egui::Ui, value: &mut Option<Color>, default: Color) -> bool {
    // Keep both widgets in a single horizontal so the enclosing `form` Grid sees
    // one cell — a second cell here would spill into a third grid column and make
    // sibling combos' `available_width()` feed back, growing the panel each frame.
    ui.horizontal(|ui| {
        let mut changed = false;
        let mut enabled = value.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            *value = enabled.then_some(default);
            changed = true;
        }
        if let Some(color) = value.as_mut() {
            changed |= color_field(ui, color);
        }
        changed
    })
    .inner
}

/// Control-only font-family combo (no label), filling the row width, with a
/// leading "(default)" entry that maps to an empty family.
fn font_combo(ui: &mut egui::Ui, id: &str, family: &mut String, available: &[String]) -> bool {
    let mut changed = false;
    let selected = if family.is_empty() {
        "(default)".to_string()
    } else {
        family.clone()
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(family.is_empty(), "(default)")
                .clicked()
                && !family.is_empty()
            {
                family.clear();
                changed = true;
            }
            for fam in available {
                if ui.selectable_label(family == fam, fam).clicked() && family != fam {
                    family.clone_from(fam);
                    changed = true;
                }
            }
        });
    changed
}

fn spacing_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    let c = &mut state.config;
    let mut changed = false;
    section(ui, "ts_spacing_sec", "Spacing", true, |ui| {
        form(ui, "ts_spacing", |ui| {
            changed |= form_row(ui, "Line leading", |ui| {
                num_field(ui, &mut c.line_spacing_em, 0.0..=2.0, " em")
            });
            changed |= form_row(ui, "Paragraph gap", |ui| {
                num_field(ui, &mut c.paragraph_spacing_mm, 0.0..=20.0, " mm")
            });
            changed |= form_row_info(ui, "Indent", "First-line paragraph indent", |ui| {
                num_field(ui, &mut c.paragraph_indent_mm, 0.0..=30.0, " mm")
            });
            changed |= form_row(ui, "Justify", |ui| {
                ui.checkbox(&mut c.justify, "").changed()
            });
            changed |= form_row(ui, "Hyphenate", |ui| {
                ui.checkbox(&mut c.hyphenate, "").changed()
            });
        });
    });
    if changed {
        state.needs_regeneration = true;
    }
}

fn page_breaks_section(ui: &mut egui::Ui, state: &mut TypesettingState) {
    // Page-break rules run over editable source text before conversion; an
    // imported document is converted once, so they don't apply to it.
    if state.import.is_some() {
        return;
    }
    let mut changed = false;
    let positions = [
        (BreakPosition::After, "after"),
        (BreakPosition::Before, "before"),
        (BreakPosition::Replace, "replace"),
    ];

    section(ui, "ts_breaks_sec", "Page breaks", false, |ui| {
        ui.label(
            egui::RichText::new("Insert a page break at lines matching a pattern.")
                .small()
                .weak(),
        );

        let mut remove: Option<usize> = None;
        for (i, rule) in state.config.page_breaks.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut rule.pattern)
                            .desired_width(90.0)
                            .hint_text("e.g. -----"),
                    )
                    .changed();
                changed |= enum_selector(
                    ui,
                    &format!("ts_break_{i}"),
                    "",
                    &mut rule.position,
                    &positions,
                );
                if ui.button("✖").clicked() {
                    remove = Some(i);
                }
            });
        }

        if let Some(i) = remove {
            state.config.page_breaks.remove(i);
            changed = true;
        }
        if ui.button("➕ Add rule").clicked() {
            state
                .config
                .page_breaks
                .push(PageBreakRule::new("", BreakPosition::Replace));
            changed = true;
        }
    });

    if changed {
        state.needs_regeneration = true;
    }
}

fn actions_section(
    ui: &mut egui::Ui,
    state: &mut TypesettingState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    section_heading(ui, "Output");
    // An imported doc compiles from its cached converted artifact; plain text
    // goes through the markup path. Both are gated on having something to output.
    let converted = state
        .import
        .as_ref()
        .and_then(|i| i.converted.as_ref())
        .cloned();
    let overrides = state
        .import
        .as_ref()
        .map(|i| i.overrides.clone())
        .unwrap_or_default();
    let has_output = converted.is_some() || !state.source_text.trim().is_empty();

    ui.horizontal(|ui| {
        if ui
            .add_enabled(has_output, egui::Button::new("💾 Save PDF…"))
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name("typeset.pdf")
                .save_file()
        {
            let _ = command_tx.send(match &converted {
                Some(conv) => PdfCommand::TypesetGenerateImported {
                    body: conv.body.clone(),
                    assets: conv.assets.clone(),
                    outline: conv.outline.clone(),
                    overrides: overrides.clone(),
                    config: state.config.clone(),
                    output_path: path,
                },
                None => PdfCommand::TypesetGenerate {
                    input: state.to_input(),
                    config: state.config.clone(),
                    output_path: path,
                },
            });
        }

        if ui
            .add_enabled(has_output, egui::Button::new("📑 Send to Impose"))
            .on_hover_text("Typeset and load the result into the imposition mode")
            .clicked()
        {
            let _ = command_tx.send(match &converted {
                Some(conv) => PdfCommand::TypesetSendImportedToImpose {
                    body: conv.body.clone(),
                    assets: conv.assets.clone(),
                    outline: conv.outline.clone(),
                    overrides: overrides.clone(),
                    config: state.config.clone(),
                },
                None => PdfCommand::TypesetSendToImpose {
                    input: state.to_input(),
                    config: state.config.clone(),
                },
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_session_round_trips_assets_as_base64() {
        let session = ImportSession {
            source: "arXiv:1706.03762".into(),
            payload: "<p>hi</p>".into(),
            kind: InputFormat::Html,
            raw_assets: vec![("fig.png".into(), vec![0u8, 1, 2, 255])],
            overrides: SectionOverrides::new(),
            asset_report: None,
            // The converted cache must NOT be persisted (holds Arcs / live state).
            converted: Some(ConvertedImport {
                body: Arc::new("body".into()),
                assets: Arc::new(Vec::new()),
                outline: Arc::new(Vec::new()),
                title: Some("T".into()),
                stats: ImportStats::default(),
            }),
            reconvert_requested: true,
        };
        let json = serde_json::to_string(&session).unwrap();
        // Asset bytes are base64 text, not a numeric array; transient fields skipped.
        assert!(json.contains("AAEC/w=="), "assets should be base64: {json}");
        assert!(
            !json.contains("converted"),
            "converted is transient: {json}"
        );

        let back: ImportSession = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, session.source);
        assert_eq!(back.raw_assets, session.raw_assets);
        assert!(back.converted.is_none());
        assert!(!back.reconvert_requested);
    }

    /// A save written before `payload`/`kind` existed (the field was `html`, with
    /// no `kind`) restores as an HTML import.
    #[test]
    fn import_session_reads_legacy_html_field() {
        let legacy = r#"{"source":"u","html":"<p>hi</p>","raw_assets":[]}"#;
        let back: ImportSession = serde_json::from_str(legacy).unwrap();
        assert_eq!(back.payload, "<p>hi</p>");
        assert_eq!(back.kind, InputFormat::Html);
    }
}
