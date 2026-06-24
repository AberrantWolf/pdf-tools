use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_flashcards::MeasurementSystem;
use pdf_units::PaperSize;
use tokio::sync::mpsc;

use super::ViewerState;
use crate::ui_components::{
    MarginsEditor, SliderBuilder, SpacingEditor, enum_selector, paper_size_picker,
};

mod flashcard_layout;
use flashcard_layout::{FlashcardLayout, MaxValueType, convert_values, get_max_value};

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

    // Loaded flashcards (re-loaded from `csv_path`; not persisted).
    #[serde(skip)]
    pub cards: Vec<pdf_flashcards::Flashcard>,

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
            cards: Vec::new(),
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

                show_csv_section(ui, state, command_tx);
                ui.add_space(10.0);
                ui.separator();

                show_paper_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_margins_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_sizing_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_spacing_section(ui, state);
                ui.add_space(10.0);
                ui.separator();

                show_font_section(ui, state);
                ui.add_space(20.0);
                ui.separator();

                show_actions_section(ui, state, command_tx);
            });
        });

    show_preview_area(ui, state, command_tx);
}

fn show_csv_section(
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
        for w in &warnings {
            log::warn!("Pasted cards: {w}");
        }
        log::info!("Loaded {} cards from pasted text", cards.len());
        state.cards = cards;
        state.csv_path.clear();
        state.needs_regeneration = true;
    }

    if !state.cards.is_empty() {
        ui.label(format!("Loaded: {} cards", state.cards.len()));
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
    ui.label("Card Size:");
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
    show_layout_buttons(ui, state, command_tx);

    // Auto-regenerate preview when settings change
    if state.needs_regeneration && can_generate {
        generate_preview(state, command_tx);
    }
}

/// Save/load buttons for layouts (templates) and projects (layout + cards).
#[cfg(not(target_arch = "wasm32"))]
fn show_layout_buttons(
    ui: &mut egui::Ui,
    state: &FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.add_space(6.0);
    ui.label("Layout & Project:");

    // Saving a project requires cards; a layout template is always available.
    let has_cards = !state.cards.is_empty();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(has_cards, egui::Button::new("💾 Save Project..."))
            .on_hover_text("Save the page layout together with the loaded cards")
            .clicked()
        {
            save_project(state);
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
        .button("📂 Load Layout / Project...")
        .on_hover_text("Load a saved layout template or full project")
        .clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
    {
        let _ = command_tx.send(PdfCommand::FlashcardsLoadConfig { path });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_project(state: &FlashcardState) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name("flashcards_project.json")
        .save_file()
    {
        let project = pdf_flashcards::FlashcardProject {
            options: state.to_options(),
            cards: state.cards.clone(),
        };
        tokio::spawn(async move {
            match project.save(&path).await {
                Ok(()) => log::info!("Project saved to {}", path.display()),
                Err(e) => log::error!("Failed to save project: {e}"),
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
    let card_count = state.cards.len();
    super::preview::show_preview_pane(ui, &mut state.preview_viewer, command_tx, None, |ui| {
        if card_count == 0 {
            ui.heading("No CSV Loaded");
            ui.label("Select a CSV file to begin");
        } else {
            ui.heading("Ready to Generate");
            ui.label(format!("{card_count} flashcards loaded"));
            ui.label("Click 'Generate Preview' to see the result");
        }
    });
}
