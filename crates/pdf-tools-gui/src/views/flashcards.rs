use eframe::egui;
use pdf_async_runtime::PdfCommand;
use pdf_flashcards::{MeasurementSystem, PaperType};
use tokio::sync::mpsc;

use super::ViewerState;

pub struct FlashcardState {
    pub csv_path: String,
    pub paper_type: PaperType,
    pub measurement_system: MeasurementSystem,

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
                // For custom, we'd need additional fields - for now use Letter
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

    // TODO: Add a drop-down to specify EITHER number of rows/cols OR card size, not both
    // The unselected one is grayed out. Otherwise it just gets too cumbersome to track.

    // Calculate rows/columns from card size
    pub fn recalculate_grid_from_card_size(&mut self) {
        let options = self.to_options();

        let available_width =
            options.page_width_mm - options.margin_left_mm - options.margin_right_mm;
        let available_height =
            options.page_height_mm - options.margin_top_mm - options.margin_bottom_mm;

        // Calculate how many cards fit with spacing
        self.columns = ((available_width + options.column_spacing_mm)
            / (options.card_width_mm + options.column_spacing_mm))
            .floor()
            .max(1.0) as usize;
        self.rows = ((available_height + options.row_spacing_mm)
            / (options.card_height_mm + options.row_spacing_mm))
            .floor()
            .max(1.0) as usize;
    }

    // Calculate card size from rows/columns
    pub fn recalculate_card_size_from_grid(&mut self) {
        let options = self.to_options();

        let available_width =
            options.page_width_mm - options.margin_left_mm - options.margin_right_mm;
        let available_height =
            options.page_height_mm - options.margin_top_mm - options.margin_bottom_mm;

        // Calculate card size that fits the grid
        let card_width_mm = if self.columns > 0 {
            (available_width - (self.columns - 1) as f32 * options.column_spacing_mm)
                / self.columns as f32
        } else {
            available_width
        };

        let card_height_mm = if self.rows > 0 {
            (available_height - (self.rows - 1) as f32 * options.row_spacing_mm) / self.rows as f32
        } else {
            available_height
        };

        self.card_width = self.measurement_system.from_mm(card_width_mm);
        self.card_height = self.measurement_system.from_mm(card_height_mm);
    }
}

pub fn show_flashcards(
    ui: &mut egui::Ui,
    state: &mut FlashcardState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
    status: &mut String,
) {
    egui::SidePanel::left("flashcard_controls")
        .min_width(300.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Flashcard Settings");
                ui.separator();

                // CSV File Selection
                ui.label("CSV File:");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut state.csv_path);
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .pick_file()
                        {
                            state.csv_path = path.display().to_string();
                            // Load CSV
                            let _ =
                                command_tx.send(PdfCommand::FlashcardsLoadCsv { input_path: path });
                            *status = "Loading CSV...".to_string();
                        }
                    }
                });

                if !state.cards.is_empty() {
                    ui.label(format!("Loaded: {} cards", state.cards.len()));
                }

                ui.add_space(10.0);
                ui.separator();

                // Paper Type
                ui.label("Paper Type:");
                let mut changed = false;
                egui::ComboBox::from_id_salt("paper_type")
                    .selected_text(state.paper_type.name())
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(&mut state.paper_type, PaperType::Letter, "Letter")
                            .changed();
                        changed |= ui
                            .selectable_value(&mut state.paper_type, PaperType::Legal, "Legal")
                            .changed();
                        changed |= ui
                            .selectable_value(&mut state.paper_type, PaperType::A4, "A4")
                            .changed();
                        changed |= ui
                            .selectable_value(&mut state.paper_type, PaperType::A5, "A5")
                            .changed();
                    });
                if changed {
                    state.needs_regeneration = true;
                }

                ui.add_space(10.0);

                // Measurement System
                ui.label("Measurement System:");
                let old_system = state.measurement_system;
                egui::ComboBox::from_id_salt("measurement_system")
                    .selected_text(state.measurement_system.name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.measurement_system,
                            MeasurementSystem::Inches,
                            "Inches (in)",
                        );
                        ui.selectable_value(
                            &mut state.measurement_system,
                            MeasurementSystem::Millimeters,
                            "Millimeters (mm)",
                        );
                        ui.selectable_value(
                            &mut state.measurement_system,
                            MeasurementSystem::Points,
                            "Points (pt)",
                        );
                    });

                // Convert values if system changed
                if old_system != state.measurement_system {
                    let to_mm = |v| old_system.to_mm(v);
                    let from_mm = |v| state.measurement_system.from_mm(v);

                    state.margin_top = from_mm(to_mm(state.margin_top));
                    state.margin_bottom = from_mm(to_mm(state.margin_bottom));
                    state.margin_left = from_mm(to_mm(state.margin_left));
                    state.margin_right = from_mm(to_mm(state.margin_right));
                    state.card_width = from_mm(to_mm(state.card_width));
                    state.card_height = from_mm(to_mm(state.card_height));
                    state.row_spacing = from_mm(to_mm(state.row_spacing));
                    state.column_spacing = from_mm(to_mm(state.column_spacing));
                }

                ui.add_space(10.0);
                ui.separator();

                // Margins
                ui.label("Page Margins:");
                let unit = state.measurement_system.name();
                let margin_max = match state.measurement_system {
                    MeasurementSystem::Inches => 2.0,
                    MeasurementSystem::Millimeters => 50.0,
                    MeasurementSystem::Points => 144.0,
                };
                changed = false;
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.margin_top, 0.0..=margin_max)
                            .text(format!("Top ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.margin_bottom, 0.0..=margin_max)
                            .text(format!("Bottom ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.margin_left, 0.0..=margin_max)
                            .text(format!("Left ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.margin_right, 0.0..=margin_max)
                            .text(format!("Right ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                if changed {
                    state.needs_regeneration = true;
                }

                ui.add_space(10.0);
                ui.separator();

                // Grid Layout
                ui.label("Grid Layout:");
                changed = false;
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.rows, 1..=10)
                            .text("Rows")
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.columns, 1..=10)
                            .text("Columns")
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                if changed {
                    state.needs_regeneration = true;
                }

                ui.add_space(10.0);
                ui.separator();

                // Card Size
                ui.label("Card Size:");
                let card_max = match state.measurement_system {
                    MeasurementSystem::Inches => 10.0,
                    MeasurementSystem::Millimeters => 250.0,
                    MeasurementSystem::Points => 720.0,
                };
                changed = false;
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.card_width, 0.0..=card_max)
                            .text(format!("Width ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.card_height, 0.0..=card_max)
                            .text(format!("Height ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                if changed {
                    state.needs_regeneration = true;
                }

                ui.add_space(10.0);
                ui.separator();

                // Spacing
                ui.label("Spacing:");
                let spacing_max = match state.measurement_system {
                    MeasurementSystem::Inches => 1.0,
                    MeasurementSystem::Millimeters => 25.0,
                    MeasurementSystem::Points => 72.0,
                };
                changed = false;
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.row_spacing, 0.0..=spacing_max)
                            .text(format!("Row Spacing ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut state.column_spacing, 0.0..=spacing_max)
                            .text(format!("Column Spacing ({})", unit))
                            .clamping(egui::SliderClamping::Never),
                    )
                    .changed();
                if changed {
                    state.needs_regeneration = true;
                }

                ui.add_space(10.0);
                ui.separator();

                // Font Size
                ui.label("Font Size:");
                if ui
                    .add(egui::Slider::new(&mut state.font_size_pt, 6.0..=36.0).text("Size (pt)"))
                    .changed()
                {
                    state.needs_regeneration = true;
                }

                ui.add_space(20.0);
                ui.separator();

                // Actions
                if ui.button("📄 Generate Preview").clicked() && !state.cards.is_empty() {
                    state.needs_regeneration = false;
                    let options = state.to_options();
                    let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
                        cards: state.cards.clone(),
                        options,
                        output_path: std::env::temp_dir().join("flashcards_preview.pdf"),
                    });
                    *status = "Generating preview...".to_string();
                }

                if ui.button("💾 Save PDF...").clicked() && !state.cards.is_empty() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("flashcards.pdf")
                        .save_file()
                    {
                        let options = state.to_options();
                        let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
                            cards: state.cards.clone(),
                            options,
                            output_path: path,
                        });
                        *status = "Saving PDF...".to_string();
                    }
                }

                if state.needs_regeneration && !state.cards.is_empty() {
                    state.recalculate_card_size_from_grid();
                    let options = state.to_options();
                    let _ = command_tx.send(PdfCommand::FlashcardsGenerate {
                        cards: state.cards.clone(),
                        options,
                        output_path: std::env::temp_dir().join("flashcards_preview.pdf"),
                    });
                    state.needs_regeneration = false;
                    *status = "Generating preview...".to_string();
                }
            });
        });

    // Preview area
    egui::CentralPanel::default().show_inside(ui, |ui| {
        if state.preview_viewer.is_some() {
            super::show_viewer(ui, &mut state.preview_viewer, command_tx, status);
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
