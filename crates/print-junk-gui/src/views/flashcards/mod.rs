use std::path::PathBuf;

use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_flashcards::{ColumnRole, CsvTable, Duplex, FlashcardWarning, MeasurementSystem};
use pdf_units::PaperSize;
use tokio::sync::mpsc;

use super::ViewerState;
use crate::ui_components::{
    STANDARD_PAPER_SIZES, button_group, enum_combo, form, form_row, form_row_enabled,
    form_row_info, num_field, section, section_heading,
};

/// Multiple columns assigned to one side are stacked on separate lines.
const COLUMN_SEPARATOR: &str = "\n";

mod flashcard_layout;
use flashcard_layout::{FlashcardLayout, MaxValueType, convert_values, get_max_value};

/// Amber used for non-fatal warnings, matching the imposition mode.
const WARN_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 200, 80);

/// Upper bound for manually-entered grid rows/columns. Generous enough for small
/// cards and label sheets; the summary still warns when the grid overflows the
/// printable area, and Card-size mode computes the grid without this cap.
const MAX_GRID_DIM: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SizingMode {
    Grid,     // Specify rows/columns, card size is calculated
    CardSize, // Specify card size, rows/columns are calculated
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FlashcardState {
    pub csv_path: String,

    // Whether the loaded CSV/paste's first row names the columns (vs. being data).
    pub csv_has_headers: bool,

    // Per-column assignment to the card's front/back (or ignore). Persisted so a
    // reloaded project re-maps the same way; reconciled to the table's column
    // count on load.
    pub column_roles: Vec<ColumnRole>,

    // Pasted card text — a transient input alternative to a CSV file; not persisted.
    #[serde(skip)]
    pub paste_text: String,

    // The parsed source table (re-loaded from `csv_path`/paste; not persisted).
    #[serde(skip)]
    pub csv_table: Option<CsvTable>,

    pub paper_size: PaperSize,
    pub measurement_system: MeasurementSystem,
    pub sizing_mode: SizingMode,

    // Margins in current measurement system
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,

    // Card dimensions in current measurement system
    pub card_width: f32,
    pub card_height: f32,

    // Grid layout
    pub rows: usize,
    pub columns: usize,

    // Spacing in current measurement system
    pub row_spacing: f32,
    pub column_spacing: f32,

    // Base font sizes (pt) for the card front and back. The `font_size_pt` alias
    // loads projects saved before the two were sized independently.
    #[serde(alias = "font_size_pt")]
    pub front_font_size_pt: f32,
    pub back_font_size_pt: f32,

    // Two-sided printing / flip mode.
    pub duplex: Duplex,

    // Loaded flashcards (re-loaded from `csv_path`; not persisted).
    #[serde(skip)]
    pub cards: Vec<pdf_flashcards::Flashcard>,

    // Warnings from the most recent CSV/paste load (transient).
    #[serde(skip)]
    pub load_warnings: Vec<FlashcardWarning>,

    // Preview state (transient).
    #[serde(skip)]
    pub preview_viewer: Option<ViewerState>,

    // Track if we need to regenerate (transient).
    #[serde(skip)]
    pub needs_regeneration: bool,
}

impl Default for FlashcardState {
    fn default() -> Self {
        let measurement_system = MeasurementSystem::Inches;
        Self {
            csv_path: String::new(),
            csv_has_headers: true,
            column_roles: Vec::new(),
            paste_text: String::new(),
            csv_table: None,
            paper_size: PaperSize::Letter,
            measurement_system,
            sizing_mode: SizingMode::Grid,
            margin_top: 0.4,
            margin_bottom: 0.4,
            margin_left: 0.4,
            margin_right: 0.4,
            card_width: 2.5,
            card_height: 3.5,
            rows: 2,
            columns: 3,
            row_spacing: 0.2,
            column_spacing: 0.2,
            front_font_size_pt: 12.0,
            back_font_size_pt: 12.0,
            duplex: Duplex::LongEdge,
            cards: Vec::new(),
            load_warnings: Vec::new(),
            preview_viewer: None,
            needs_regeneration: false,
        }
    }
}

impl FlashcardState {
    /// Adopt a freshly parsed source table: reconcile the column roles to its
    /// column count (keeping the user's mapping when it still fits, e.g. across a
    /// header-toggle re-parse), then project cards.
    pub fn set_table(&mut self, table: CsvTable, warnings: Vec<FlashcardWarning>) {
        if self.column_roles.len() != table.columns.len() {
            self.column_roles = table.default_roles();
        }
        self.csv_table = Some(table);
        self.load_warnings = warnings;
        self.reproject_cards();
    }

    /// Rebuild the cards from the current table and column roles. No-op when no
    /// table is loaded (e.g. a deck supplied cards directly).
    pub fn reproject_cards(&mut self) {
        let Some(table) = &self.csv_table else { return };
        let cards = table.to_cards(&self.column_roles, COLUMN_SEPARATOR);
        self.cards = cards;
        self.needs_regeneration = true;
    }

    pub fn to_options(&self) -> pdf_flashcards::FlashcardOptions {
        let (page_width_mm, page_height_mm) = self.paper_size.dimensions_mm();
        pdf_flashcards::FlashcardOptions {
            page_width_mm,
            page_height_mm,
            margin_top_mm: self.measurement_system.to_mm(self.margin_top),
            margin_bottom_mm: self.measurement_system.to_mm(self.margin_bottom),
            margin_left_mm: self.measurement_system.to_mm(self.margin_left),
            margin_right_mm: self.measurement_system.to_mm(self.margin_right),
            card_width_mm: self.measurement_system.to_mm(self.card_width),
            card_height_mm: self.measurement_system.to_mm(self.card_height),
            rows: self.rows,
            columns: self.columns,
            row_spacing_mm: self.measurement_system.to_mm(self.row_spacing),
            column_spacing_mm: self.measurement_system.to_mm(self.column_spacing),
            front_font_size_pt: self.front_font_size_pt,
            back_font_size_pt: self.back_font_size_pt,
            duplex: self.duplex,
        }
    }

    /// Apply a loaded layout to the UI state. Page dimensions are reverse-mapped
    /// to a paper size, and all measurements are converted into the user's
    /// currently selected unit (the saved file is canonical millimetres).
    pub fn apply_options(&mut self, options: &pdf_flashcards::FlashcardOptions) {
        let sys = self.measurement_system;
        self.paper_size =
            PaperSize::from_dimensions_mm(options.page_width_mm, options.page_height_mm);
        self.margin_top = sys.from_mm(options.margin_top_mm);
        self.margin_bottom = sys.from_mm(options.margin_bottom_mm);
        self.margin_left = sys.from_mm(options.margin_left_mm);
        self.margin_right = sys.from_mm(options.margin_right_mm);
        self.card_width = sys.from_mm(options.card_width_mm);
        self.card_height = sys.from_mm(options.card_height_mm);
        self.rows = options.rows;
        self.columns = options.columns;
        self.row_spacing = sys.from_mm(options.row_spacing_mm);
        self.column_spacing = sys.from_mm(options.column_spacing_mm);
        self.front_font_size_pt = options.front_font_size_pt;
        self.back_font_size_pt = options.back_font_size_pt;
        self.duplex = options.duplex;
    }

    pub fn convert_all_values(&mut self, old_system: MeasurementSystem) {
        convert_values(
            &mut [
                &mut self.margin_top,
                &mut self.margin_bottom,
                &mut self.margin_left,
                &mut self.margin_right,
                &mut self.card_width,
                &mut self.card_height,
                &mut self.row_spacing,
                &mut self.column_spacing,
            ],
            old_system,
            self.measurement_system,
        );
    }

    pub fn recalculate_grid_from_card_size(&mut self) {
        let layout = self.to_layout();
        (self.rows, self.columns) = layout.calculate_grid_from_card_size();
    }

    pub fn recalculate_card_size_from_grid(&mut self) {
        let layout = self.to_layout();
        (self.card_width, self.card_height) = layout.calculate_card_size_from_grid();
    }

    /// Recompute whichever dimension the current sizing mode derives from the
    /// available area: the card size in Grid mode, the rows/columns that fit in
    /// Card-size mode. Call whenever the page, margins, or spacing change.
    pub fn recalculate_for_mode(&mut self) {
        match self.sizing_mode {
            SizingMode::Grid => self.recalculate_card_size_from_grid(),
            SizingMode::CardSize => self.recalculate_grid_from_card_size(),
        }
    }

    fn to_layout(&self) -> FlashcardLayout {
        FlashcardLayout {
            paper_size: self.paper_size,
            measurement_system: self.measurement_system,
            margin_top: self.margin_top,
            margin_bottom: self.margin_bottom,
            margin_left: self.margin_left,
            margin_right: self.margin_right,
            card_width: self.card_width,
            card_height: self.card_height,
            rows: self.rows,
            columns: self.columns,
            row_spacing: self.row_spacing,
            column_spacing: self.column_spacing,
        }
    }
}

pub fn show_flashcards(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::Panel::left("flashcard_controls")
        .min_size(300.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Flashcards");
                ui.add_space(2.0);

                section(ui, "fc_input_sec", "Input", true, |ui| {
                    show_input_section(ui, state, command_tx);
                });
                if state.csv_table.is_some() {
                    section(ui, "fc_columns_sec", "Columns", true, |ui| {
                        show_columns_section(ui, state);
                    });
                }
                section(ui, "fc_page_sec", "Page", true, |ui| {
                    show_paper_section(ui, state);
                });
                section(ui, "fc_margins_sec", "Margins", true, |ui| {
                    show_margins_section(ui, state);
                });
                section(ui, "fc_layout_sec", "Layout", true, |ui| {
                    show_sizing_section(ui, state);
                });
                section(ui, "fc_spacing_sec", "Spacing", false, |ui| {
                    show_spacing_section(ui, state);
                });
                section(ui, "fc_text_sec", "Text", false, |ui| {
                    show_font_section(ui, state);
                });
                section(ui, "fc_duplex_sec", "Two-sided", false, |ui| {
                    show_duplex_section(ui, state);
                });
                section(ui, "fc_summary_sec", "Summary", true, |ui| {
                    show_summary_section(ui, state);
                });
                show_cards_peek(ui, state);

                section_heading(ui, "Output");
                show_actions_section(ui, state, command_tx);
            });
        });

    show_preview_area(ui, state, command_tx);
}

fn show_input_section(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.label("CSV File:");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.csv_path);
        if ui.button("Browse...").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
        {
            state.csv_path = path.display().to_string();
            // A file is assumed to have a header row; the toggle below overrides.
            state.csv_has_headers = true;
            log::info!("Loading CSV: {}", path.display());
            let _ = command_tx.send(PdfCommand::FlashcardsLoadCsv {
                input_path: path,
                has_headers: true,
            });
        }
    });

    // The header toggle lives with the input controls so it can be set before
    // (or after) loading; changing it re-parses whatever source is loaded.
    if ui
        .checkbox(&mut state.csv_has_headers, "First row is a header")
        .on_hover_text("Treat the first row as column names rather than card data")
        .changed()
        && state.csv_table.is_some()
    {
        reload_source(state, command_tx);
    }

    ui.add_space(8.0);
    ui.label("Or paste cards (one per line, e.g. front,back):");
    ui.add(
        egui::TextEdit::multiline(&mut state.paste_text)
            .desired_rows(4)
            .desired_width(f32::INFINITY)
            .hint_text("apple,a fruit\nchien,dog"),
    );
    if ui
        .add_enabled(
            !state.paste_text.trim().is_empty(),
            egui::Button::new("Load from text"),
        )
        .clicked()
    {
        // Pasted rows are assumed to be data (no header); the toggle overrides.
        state.csv_has_headers = false;
        state.csv_path.clear();
        let (table, warnings) = pdf_flashcards::parse_pasted_table(&state.paste_text, false);
        log::info!("Loaded {} row(s) from pasted text", table.rows.len());
        state.set_table(table, warnings);
    }

    if !state.cards.is_empty() {
        ui.label(format!("Loaded: {} cards", state.cards.len()));
    }
    show_load_warnings(ui, &state.load_warnings);
}

/// Re-parse the currently loaded source with the current header setting — used
/// when the "First row is a header" toggle changes.
fn reload_source(state: &mut FlashcardState, command_tx: &mpsc::UnboundedSender<PdfCommand>) {
    if !state.csv_path.is_empty() {
        let _ = command_tx.send(PdfCommand::FlashcardsLoadCsv {
            input_path: PathBuf::from(state.csv_path.clone()),
            has_headers: state.csv_has_headers,
        });
    } else if !state.paste_text.trim().is_empty() {
        let (table, warnings) =
            pdf_flashcards::parse_pasted_table(&state.paste_text, state.csv_has_headers);
        state.set_table(table, warnings);
    }
}

/// Map each detected column to the card's front, back, or neither.
fn show_columns_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let role_opts = [
        (ColumnRole::Front, "Front"),
        (ColumnRole::Back, "Back"),
        (ColumnRole::Ignore, "Ignore"),
    ];

    // Column names are cloned so the roles can be edited alongside (a few short
    // strings; avoids holding a borrow on `csv_table` across the role edits).
    let columns = state
        .csv_table
        .as_ref()
        .map(|t| t.columns.clone())
        .unwrap_or_default();

    let mut roles_changed = false;
    form(ui, "fc_columns", |ui| {
        for (i, name) in columns.iter().enumerate() {
            roles_changed |= form_row(ui, name, |ui| {
                button_group(ui, &mut state.column_roles[i], &role_opts)
            });
        }
    });

    if roles_changed {
        state.reproject_cards();
    }

    if !state.column_roles.contains(&ColumnRole::Front) {
        ui.colored_label(WARN_COLOR, "⚠ No column assigned to the front");
    }
    if state.duplex != Duplex::OneSided && !state.column_roles.contains(&ColumnRole::Back) {
        ui.colored_label(WARN_COLOR, "⚠ No column assigned to the back");
    }
}

/// Compact, amber summary of the most recent load's warnings.
fn show_load_warnings(ui: &mut egui::Ui, warnings: &[FlashcardWarning]) {
    if warnings
        .iter()
        .any(|w| matches!(w, FlashcardWarning::EmptyCsv))
    {
        ui.colored_label(WARN_COLOR, "⚠ No data rows found");
    }
}

fn show_paper_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let measurement_systems = [
        (MeasurementSystem::Inches, "Inches (in)"),
        (MeasurementSystem::Millimeters, "Millimeters (mm)"),
        (MeasurementSystem::Points, "Points (pt)"),
    ];
    let old_system = state.measurement_system;
    let mut changed = false;
    form(ui, "fc_page", |ui| {
        changed |= form_row(ui, "Paper", |ui| {
            enum_combo(ui, "fc_paper", &mut state.paper_size, &STANDARD_PAPER_SIZES)
        });
        form_row(ui, "Units", |ui| {
            enum_combo(
                ui,
                "fc_units",
                &mut state.measurement_system,
                &measurement_systems,
            )
        });
    });
    // Switching units only re-expresses the same physical sizes — no regenerate.
    if old_system != state.measurement_system {
        state.convert_all_values(old_system);
    }
    // A new paper size changes the printable area, so refit the derived dimension.
    if changed {
        state.recalculate_for_mode();
        state.needs_regeneration = true;
    }
}

fn show_margins_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let max = get_max_value(MaxValueType::Margin, state.measurement_system);
    let unit = format!(" {}", state.measurement_system.name());
    let mut changed = false;
    form(ui, "fc_margins", |ui| {
        changed |= form_row(ui, "Top", |ui| {
            num_field(ui, &mut state.margin_top, 0.0..=max, &unit)
        });
        changed |= form_row(ui, "Bottom", |ui| {
            num_field(ui, &mut state.margin_bottom, 0.0..=max, &unit)
        });
        changed |= form_row(ui, "Left", |ui| {
            num_field(ui, &mut state.margin_left, 0.0..=max, &unit)
        });
        changed |= form_row(ui, "Right", |ui| {
            num_field(ui, &mut state.margin_right, 0.0..=max, &unit)
        });
    });
    // Margins change the printable area, so refit the derived dimension.
    if changed {
        state.recalculate_for_mode();
        state.needs_regeneration = true;
    }
}

fn show_sizing_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let sizing_modes = [
        (SizingMode::Grid, "Grid (rows × columns)"),
        (SizingMode::CardSize, "Card size"),
    ];
    let max = get_max_value(MaxValueType::CardSize, state.measurement_system);
    let unit = format!(" {}", state.measurement_system.name());
    let grid_mode = state.sizing_mode == SizingMode::Grid;

    let mut mode_changed = false;
    let mut grid_changed = false;
    let mut card_changed = false;
    form(ui, "fc_layout", |ui| {
        mode_changed |= form_row_info(
            ui,
            "Sizing",
            "Pick which to fix; the other is computed to fill the page. \
             Standard index cards are 2.5 × 3.5 in (poker size).",
            |ui| enum_combo(ui, "fc_sizing", &mut state.sizing_mode, &sizing_modes),
        );
        grid_changed |= form_row_enabled(ui, "Rows", grid_mode, |ui| {
            num_field(ui, &mut state.rows, 1..=MAX_GRID_DIM, "")
        });
        grid_changed |= form_row_enabled(ui, "Columns", grid_mode, |ui| {
            num_field(ui, &mut state.columns, 1..=MAX_GRID_DIM, "")
        });
        card_changed |= form_row_enabled(ui, "Width", !grid_mode, |ui| {
            num_field(ui, &mut state.card_width, 0.0..=max, &unit)
        });
        card_changed |= form_row_enabled(ui, "Height", !grid_mode, |ui| {
            num_field(ui, &mut state.card_height, 0.0..=max, &unit)
        });
    });

    // The fixed dimension drives the computed one; recompute on any change.
    if mode_changed {
        match state.sizing_mode {
            SizingMode::Grid => state.recalculate_card_size_from_grid(),
            SizingMode::CardSize => state.recalculate_grid_from_card_size(),
        }
        state.needs_regeneration = true;
    }
    if grid_changed {
        state.recalculate_card_size_from_grid();
        state.needs_regeneration = true;
    }
    if card_changed {
        state.recalculate_grid_from_card_size();
        state.needs_regeneration = true;
    }
}

fn show_spacing_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let max = get_max_value(MaxValueType::Spacing, state.measurement_system);
    let unit = format!(" {}", state.measurement_system.name());
    let mut changed = false;
    form(ui, "fc_spacing", |ui| {
        changed |= form_row(ui, "Column gap", |ui| {
            num_field(ui, &mut state.column_spacing, 0.0..=max, &unit)
        });
        changed |= form_row(ui, "Row gap", |ui| {
            num_field(ui, &mut state.row_spacing, 0.0..=max, &unit)
        });
    });
    // Gaps change how many cards fit (Card-size mode) or how big they are
    // (Grid mode), so refit the derived dimension.
    if changed {
        state.recalculate_for_mode();
        state.needs_regeneration = true;
    }
}

fn show_font_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let mut changed = false;
    let two_sided = state.duplex != Duplex::OneSided;
    form(ui, "fc_font", |ui| {
        changed |= form_row(ui, "Front size", |ui| {
            num_field(ui, &mut state.front_font_size_pt, 6.0..=36.0, " pt")
        });
        // Backs aren't printed when one-sided, so dim their size there.
        changed |= form_row_enabled(ui, "Back size", two_sided, |ui| {
            num_field(ui, &mut state.back_font_size_pt, 6.0..=36.0, " pt")
        });
    });
    if changed {
        state.needs_regeneration = true;
    }
}

fn show_duplex_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui
            .selectable_value(&mut state.duplex, Duplex::LongEdge, "Long edge")
            .on_hover_text("Flip on the long edge — backs mirror left ↔ right (most common).")
            .changed();
        changed |= ui
            .selectable_value(&mut state.duplex, Duplex::ShortEdge, "Short edge")
            .on_hover_text("Flip on the short edge — backs mirror top ↔ bottom.")
            .changed();
        changed |= ui
            .selectable_value(&mut state.duplex, Duplex::OneSided, "One-sided")
            .on_hover_text("Print fronts only.")
            .changed();
    });

    let hint = match state.duplex {
        Duplex::LongEdge => "Backs mirror left ↔ right — match your printer's long-edge binding.",
        Duplex::ShortEdge => "Backs mirror top ↔ bottom — match your printer's short-edge binding.",
        Duplex::OneSided => "Only the fronts are printed.",
    };
    ui.label(egui::RichText::new(hint).small().weak());

    if changed {
        state.needs_regeneration = true;
    }
}

fn show_summary_section(ui: &mut egui::Ui, state: &FlashcardState) {
    if state.cards.is_empty() {
        ui.label("Load cards to see a production summary.");
        return;
    }
    let summary = state.to_options().summarize(state.cards.len());
    ui.label(format!("Cards: {}", summary.card_count));
    ui.label(format!("Cards per sheet: {}", summary.cards_per_sheet));
    ui.label(format!("Sheets of paper: {}", summary.sheet_count));
    ui.label(format!("PDF pages: {}", summary.pdf_page_count));
    ui.label(format!(
        "Sides: {}",
        if summary.double_sided {
            "double-sided"
        } else {
            "single-sided"
        }
    ));
    if summary.leftover_slots > 0 {
        ui.label(format!(
            "Empty slots on last sheet: {}",
            summary.leftover_slots
        ));
    }
    for warning in &summary.warnings {
        ui.colored_label(WARN_COLOR, format!("⚠ {warning}"));
    }
}

fn show_cards_peek(ui: &mut egui::Ui, state: &FlashcardState) {
    if state.cards.is_empty() {
        return;
    }
    let title = format!("Cards ({})", state.cards.len());
    section(ui, "fc_cards_peek", &title, false, |ui| {
        for card in state.cards.iter().take(10) {
            ui.label(format!("{}  →  {}", card.front, card.back));
        }
        if state.cards.len() > 10 {
            ui.label(
                egui::RichText::new(format!("… and {} more", state.cards.len() - 10))
                    .small()
                    .weak(),
            );
        }
    });
}

fn show_actions_section(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let can_generate = !state.cards.is_empty();

    #[cfg(not(target_arch = "wasm32"))]
    if ui
        .add_enabled(can_generate, egui::Button::new("💾 Save PDF..."))
        .clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name("flashcards.pdf")
            .save_file()
    {
        log::info!("Saving flashcards to: {}", path.display());
        let options = state.to_options();
        let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
            cards: state.cards.clone(),
            options,
            output_path: path,
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    show_deck_buttons(ui, state, command_tx);

    // Auto-regenerate preview when settings change
    if state.needs_regeneration && can_generate {
        generate_preview(state, command_tx);
    }
}

/// Save/load buttons for layouts (templates) and decks (layout + cards).
#[cfg(not(target_arch = "wasm32"))]
fn show_deck_buttons(
    ui: &mut egui::Ui,
    state: &FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.add_space(6.0);
    ui.label("Layout & Deck:");

    // Saving a deck requires cards; a layout template is always available.
    let has_cards = !state.cards.is_empty();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(has_cards, egui::Button::new("💾 Save Deck..."))
            .on_hover_text("Save the page layout together with the loaded cards")
            .clicked()
        {
            save_deck(state);
        }
        if ui
            .button("Save Layout...")
            .on_hover_text("Save just the page layout as a reusable template")
            .clicked()
        {
            save_layout(state);
        }
    });

    if ui
        .button("📂 Load Deck / Layout...")
        .on_hover_text("Load a saved deck (layout + cards) or a layout template")
        .clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
    {
        let _ = command_tx.send(PdfCommand::FlashcardsLoadConfig { path });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_deck(state: &FlashcardState) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name("flashcards_deck.json")
        .save_file()
    {
        let deck = pdf_flashcards::FlashcardDeck {
            options: state.to_options(),
            cards: state.cards.clone(),
        };
        tokio::spawn(async move {
            match deck.save(&path).await {
                Ok(()) => log::info!("Deck saved to {}", path.display()),
                Err(e) => log::error!("Failed to save deck: {e}"),
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_layout(state: &FlashcardState) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name("flashcard_layout.json")
        .save_file()
    {
        let options = state.to_options();
        tokio::spawn(async move {
            match options.save(&path).await {
                Ok(()) => log::info!("Layout saved to {}", path.display()),
                Err(e) => log::error!("Failed to save layout: {e}"),
            }
        });
    }
}

fn generate_preview(state: &mut FlashcardState, command_tx: &mpsc::UnboundedSender<PdfCommand>) {
    state.needs_regeneration = false;
    log::info!("Generating flashcard preview");
    let options = state.to_options();
    let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
        cards: state.cards.clone(),
        options,
        output_path: std::env::temp_dir().join("flashcards_preview.pdf"),
    });
}

fn show_preview_area(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let overlay = (!state.cards.is_empty()).then(|| {
        let summary = state.to_options().summarize(state.cards.len());
        let sides = match state.duplex {
            Duplex::LongEdge => "double-sided, long edge",
            Duplex::ShortEdge => "double-sided, short edge",
            Duplex::OneSided => "single-sided",
        };
        let sheet_word = if summary.sheet_count == 1 {
            "sheet"
        } else {
            "sheets"
        };
        format!(
            "{} cards · {}/sheet · {} {sheet_word} · {sides}",
            summary.card_count, summary.cards_per_sheet, summary.sheet_count
        )
    });

    let card_count = state.cards.len();
    super::preview::show_preview_pane(ui, &mut state.preview_viewer, command_tx, overlay, |ui| {
        if card_count == 0 {
            ui.heading("No cards yet");
            ui.label("Load a CSV, paste rows, or open a deck");
        } else {
            ui.heading("Ready to Generate");
            ui.label(format!("{card_count} flashcards loaded"));
            ui.label("Click 'Generate Preview' to see the result");
        }
    });
}
