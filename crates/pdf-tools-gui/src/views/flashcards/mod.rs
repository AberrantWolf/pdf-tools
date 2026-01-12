use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_flashcards::{MeasurementSystem, PaperType};
use tokio::sync::mpsc;

use super::ViewerState;
use crate::ui_components::{MarginsEditor, SliderBuilder, SpacingEditor, enum_selector};

mod flashcard_layout;
use flashcard_layout::{FlashcardLayout, MaxValueType, convert_values, get_max_value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizingMode {
    Grid,     // Specify rows/columns, card size is calculated
    CardSize, // Specify card size, rows/columns are calculated
}

pub struct FlashcardState {
    pub csv_path: String,
    pub paper_type: PaperType,
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

    // Loaded flashcards
    pub cards: Vec<pdf_flashcards::Flashcard>,

    // Preview state
    pub preview_viewer: Option<ViewerState>,

    // Track if we need to regenerate
    pub needs_regeneration: bool,
}

impl Default for FlashcardState {
    fn default() -> Self {
        let measurement_system = MeasurementSystem::Inches;
        Self {
            csv_path: String::new(),
            paper_type: PaperType::Letter,
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
        pdf_flashcards::FlashcardOptions {
            page_width_mm: if self.paper_type == PaperType::Custom {
                215.9
            } else {
                self.paper_type.dimensions_mm().0
            },
            page_height_mm: if self.paper_type == PaperType::Custom {
                279.4
            } else {
                self.paper_type.dimensions_mm().1
            },
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
            paper_type: self.paper_type,
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
    egui::SidePanel::left("flashcard_controls")
        .min_width(300.0)
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
        if ui.button("Browse...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                state.csv_path = path.display().to_string();
                log::info!("Loading CSV: {}", path.display());
                let _ = command_tx.send(PdfCommand::FlashcardsLoadCsv { input_path: path });
            }
        }
    });

    if !state.cards.is_empty() {
        ui.label(format!("Loaded: {} cards", state.cards.len()));
    }
}

fn show_paper_section(ui: &mut egui::Ui, state: &mut FlashcardState) {
    let paper_types = [
        (PaperType::Letter, "Letter"),
        (PaperType::Legal, "Legal"),
        (PaperType::A4, "A4"),
        (PaperType::A5, "A5"),
    ];

    if enum_selector(
        ui,
        "paper_type",
        "Paper Type:",
        &mut state.paper_type,
        &paper_types,
    ) {
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
            .text(format!("Width ({})", unit))
            .show(ui);

        changed |= SliderBuilder::new(&mut state.card_height, 0.0..=max)
            .text(format!("Height ({})", unit))
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
    if ui.button("📄 Generate Preview").clicked() && !state.cards.is_empty() {
        state.needs_regeneration = false;
        let options = state.to_options();
        log::info!("Generating flashcard preview");
        let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
            cards: state.cards.clone(),
            options,
            output_path: std::env::temp_dir().join("flashcards_preview.pdf"),
        });
    }

    if ui.button("💾 Save PDF...").clicked() && !state.cards.is_empty() {
        if let Some(path) = rfd::FileDialog::new()
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
    }

    if state.needs_regeneration && !state.cards.is_empty() {
        let options = state.to_options();
        log::info!("Regenerating preview due to settings change");
        let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
            cards: state.cards.clone(),
            options,
            output_path: std::env::temp_dir().join("flashcards_preview.pdf"),
        });
        state.needs_regeneration = false;
    }
}

fn show_preview_area(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        if state.preview_viewer.is_some() {
            super::show_viewer(ui, &mut state.preview_viewer, command_tx);
        } else if state.cards.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("No CSV Loaded");
                    ui.label("Select a CSV file to begin");
                });
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Ready to Generate");
                    ui.label(format!("{} flashcards loaded", state.cards.len()));
                    ui.label("Click 'Generate Preview' to see the result");
                });
            });
        }
    });
}
