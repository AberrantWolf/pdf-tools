use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_flashcards::{Duplex, FlashcardWarning, MeasurementSystem};
use pdf_units::PaperSize;
use tokio::sync::mpsc;

use super::ViewerState;
use crate::ui_components::{
    MarginsEditor, SliderBuilder, SpacingEditor, enum_selector, paper_size_picker,
};

mod flashcard_layout;
use flashcard_layout::{FlashcardLayout, MaxValueType, convert_values, get_max_value};

/// Amber used for non-fatal warnings, matching the imposition mode.
const WARN_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 200, 80);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SizingMode {
    Grid,     // Specify rows/columns, card size is calculated
    CardSize, // Specify card size, rows/columns are calculated
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FlashcardState {
    pub csv_path: String,

    // Pasted card text — a transient input alternative to a CSV file; not persisted.
    #[serde(skip)]
    pub paste_text: String,

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

    pub font_size_pt: f32,

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
            paste_text: String::new(),
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
            font_size_pt: 12.0,
            duplex: Duplex::LongEdge,
            cards: Vec::new(),
            load_warnings: Vec::new(),
            preview_viewer: None,
            needs_regeneration: false,
        }
    }
}

impl FlashcardState {
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
            font_size_pt: self.font_size_pt,
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
        self.font_size_pt = options.font_size_pt;
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
                ui.heading("Flashcard Settings");
                ui.separator();

                egui::CollapsingHeader::new("🃏 Input")
                    .default_open(true)
                    .show(ui, |ui| show_input_section(ui, state, command_tx));

                egui::CollapsingHeader::new("📄 Page & Layout")
                    .default_open(true)
                    .show(ui, |ui| {
                        show_paper_section(ui, state);
                        ui.add_space(8.0);
                        ui.separator();
                        show_margins_section(ui, state);
                        ui.add_space(8.0);
                        ui.separator();
                        show_sizing_section(ui, state);
                        ui.add_space(8.0);
                        ui.separator();
                        show_spacing_section(ui, state);
                    });

                egui::CollapsingHeader::new("🔤 Text")
                    .default_open(false)
                    .show(ui, |ui| show_font_section(ui, state));

                egui::CollapsingHeader::new("🔁 Two-sided")
                    .default_open(false)
                    .show(ui, |ui| show_duplex_section(ui, state));

                egui::CollapsingHeader::new("📊 Summary")
                    .default_open(true)
                    .show(ui, |ui| show_summary_section(ui, state));

                show_cards_peek(ui, state);

                ui.add_space(8.0);
                ui.separator();
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
            log::info!("Loading CSV: {}", path.display());
            let _ = command_tx.send(PdfCommand::FlashcardsLoadCsv { input_path: path });
        }
    });

    ui.add_space(8.0);
    ui.label("Or paste cards (one per line, front,back):");
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
        let (cards, warnings) = pdf_flashcards::parse_cards(&state.paste_text);
        log::info!("Loaded {} cards from pasted text", cards.len());
        state.cards = cards;
        state.load_warnings = warnings;
        state.csv_path.clear();
        state.needs_regeneration = true;
    }

    if !state.cards.is_empty() {
        ui.label(format!("Loaded: {} cards", state.cards.len()));
    }
    show_load_warnings(ui, &state.load_warnings);
}

/// Compact, amber summary of the most recent load's warnings.
fn show_load_warnings(ui: &mut egui::Ui, warnings: &[FlashcardWarning]) {
    if warnings
        .iter()
        .any(|w| matches!(w, FlashcardWarning::EmptyCsv))
    {
        ui.colored_label(WARN_COLOR, "⚠ No usable cards (need 2 columns per row)");
        return;
    }
    let skipped = warnings
        .iter()
        .filter(|w| matches!(w, FlashcardWarning::CsvRowSkipped { .. }))
        .count();
    if skipped > 0 {
        ui.colored_label(
            WARN_COLOR,
            format!("⚠ {skipped} row(s) skipped (need 2 columns)"),
        );
    }
}

fn show_paper_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    if paper_size_picker(ui, "paper_size", "Paper Type:", &mut state.paper_size) {
        state.needs_regeneration = true;
    }

    ui.add_space(10.0);

    let measurement_systems = [
        (MeasurementSystem::Inches, "Inches (in)"),
        (MeasurementSystem::Millimeters, "Millimeters (mm)"),
        (MeasurementSystem::Points, "Points (pt)"),
    ];

    let old_system = state.measurement_system;
    enum_selector(
        ui,
        "measurement_system",
        "Measurement System:",
        &mut state.measurement_system,
        &measurement_systems,
    );

    if old_system != state.measurement_system {
        state.convert_all_values(old_system);
    }
}

fn show_margins_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Page Margins:");
    let max = get_max_value(MaxValueType::Margin, state.measurement_system);
    let unit = state.measurement_system.name();

    if MarginsEditor::new(
        &mut state.margin_top,
        &mut state.margin_bottom,
        &mut state.margin_left,
        &mut state.margin_right,
        max,
        unit,
    )
    .show(ui)
    {
        state.needs_regeneration = true;
    }
}

fn show_sizing_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Sizing Mode:");
    egui::ComboBox::from_id_salt("sizing_mode")
        .selected_text(match state.sizing_mode {
            SizingMode::Grid => "Specify Grid (rows/columns)",
            SizingMode::CardSize => "Specify Card Size",
        })
        .show_ui(ui, |ui| {
            if ui
                .selectable_value(
                    &mut state.sizing_mode,
                    SizingMode::Grid,
                    "Specify Grid (rows/columns)",
                )
                .on_hover_text("Set rows and columns; card size is computed to fit.")
                .changed()
            {
                state.recalculate_card_size_from_grid();
                state.needs_regeneration = true;
            }
            if ui
                .selectable_value(
                    &mut state.sizing_mode,
                    SizingMode::CardSize,
                    "Specify Card Size",
                )
                .on_hover_text("Set the card size; the grid is computed to fit the page.")
                .changed()
            {
                state.recalculate_grid_from_card_size();
                state.needs_regeneration = true;
            }
        });

    ui.add_space(10.0);
    ui.separator();

    // Grid Layout
    ui.label("Grid Layout:");
    ui.add_enabled_ui(state.sizing_mode == SizingMode::Grid, |ui| {
        let mut changed = false;
        changed |= SliderBuilder::new(&mut state.rows, 1..=10)
            .text("Rows")
            .show(ui);
        changed |= SliderBuilder::new(&mut state.columns, 1..=10)
            .text("Columns")
            .show(ui);

        if changed {
            state.recalculate_card_size_from_grid();
            state.needs_regeneration = true;
        }
    });

    ui.add_space(10.0);
    ui.separator();

    // Card Size
    ui.label("Card Size:")
        .on_hover_text("Standard index cards are 2.5 × 3.5 in (poker size).");
    ui.add_enabled_ui(state.sizing_mode == SizingMode::CardSize, |ui| {
        let max = get_max_value(MaxValueType::CardSize, state.measurement_system);
        let unit = state.measurement_system.name();
        let mut changed = false;

        changed |= SliderBuilder::new(&mut state.card_width, 0.0..=max)
            .text(format!("Width ({unit})"))
            .show(ui);

        changed |= SliderBuilder::new(&mut state.card_height, 0.0..=max)
            .text(format!("Height ({unit})"))
            .show(ui);

        if changed {
            state.recalculate_grid_from_card_size();
            state.needs_regeneration = true;
        }
    });
}

fn show_spacing_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Spacing:");
    let max = get_max_value(MaxValueType::Spacing, state.measurement_system);
    let unit = state.measurement_system.name();

    if SpacingEditor::new(
        &mut state.column_spacing,
        &mut state.row_spacing,
        "Column Spacing",
        "Row Spacing",
        max,
        unit,
    )
    .show(ui)
    {
        state.needs_regeneration = true;
    }
}

fn show_font_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Font Size:");
    if SliderBuilder::new(&mut state.font_size_pt, 6.0..=36.0)
        .text("Size (pt)")
        .show(ui)
    {
        state.needs_regeneration = true;
    }
}

fn show_duplex_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    ui.label("Two-sided printing:").on_hover_text(
        "Match your printer's two-sided setting so card backs line up behind their fronts.",
    );

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
        Duplex::LongEdge => "Backs mirror left ↔ right.",
        Duplex::ShortEdge => "Backs mirror top ↔ bottom.",
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
    egui::CollapsingHeader::new(format!("Cards ({})", state.cards.len()))
        .default_open(false)
        .show(ui, |ui| {
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
