use pdf_flashcards::{FlashcardOptions, MeasurementSystem, PaperType};

/// Layout calculator for flashcard grid sizing
pub struct FlashcardLayout {
    pub paper_type: PaperType,
    pub measurement_system: MeasurementSystem,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub card_width: f32,
    pub card_height: f32,
    pub rows: usize,
    pub columns: usize,
    pub row_spacing: f32,
    pub column_spacing: f32,
}

impl FlashcardLayout {
    /// Calculate rows/columns from card size
    pub fn calculate_grid_from_card_size(&self) -> (usize, usize) {
        let options = self.to_options_mm();

        let available_width =
            options.page_width_mm - options.margin_left_mm - options.margin_right_mm;
        let available_height =
            options.page_height_mm - options.margin_top_mm - options.margin_bottom_mm;

        let columns = ((available_width + options.column_spacing_mm)
            / (options.card_width_mm + options.column_spacing_mm))
            .floor()
            .max(1.0) as usize;

        let rows = ((available_height + options.row_spacing_mm)
            / (options.card_height_mm + options.row_spacing_mm))
            .floor()
            .max(1.0) as usize;

        (rows, columns)
    }

    /// Calculate card size from rows/columns
    pub fn calculate_card_size_from_grid(&self) -> (f32, f32) {
        let options = self.to_options_mm();

        let available_width =
            options.page_width_mm - options.margin_left_mm - options.margin_right_mm;
        let available_height =
            options.page_height_mm - options.margin_top_mm - options.margin_bottom_mm;

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

        (
            self.measurement_system.from_mm(card_width_mm),
            self.measurement_system.from_mm(card_height_mm),
        )
    }

    /// Convert to FlashcardOptions (all values in mm)
    fn to_options_mm(&self) -> FlashcardOptions {
        FlashcardOptions {
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
            font_size_pt: 12.0, // Default, will be overridden
        }
    }
}

/// Convert a single value between measurement systems
pub fn convert_value(
    value: f32,
    from_system: MeasurementSystem,
    to_system: MeasurementSystem,
) -> f32 {
    to_system.from_mm(from_system.to_mm(value))
}

/// Convert multiple values between measurement systems
pub fn convert_values(
    values: &mut [&mut f32],
    from_system: MeasurementSystem,
    to_system: MeasurementSystem,
) {
    for value in values {
        **value = convert_value(**value, from_system, to_system);
    }
}

/// Get maximum value for sliders based on value type and measurement system
#[derive(Debug, Clone, Copy)]
pub enum MaxValueType {
    Margin,
    CardSize,
    Spacing,
}

pub fn get_max_value(value_type: MaxValueType, system: MeasurementSystem) -> f32 {
    match value_type {
        MaxValueType::Margin => match system {
            MeasurementSystem::Inches => 2.0,
            MeasurementSystem::Millimeters => 50.0,
            MeasurementSystem::Points => 144.0,
        },
        MaxValueType::CardSize => match system {
            MeasurementSystem::Inches => 10.0,
            MeasurementSystem::Millimeters => 250.0,
            MeasurementSystem::Points => 720.0,
        },
        MaxValueType::Spacing => match system {
            MeasurementSystem::Inches => 1.0,
            MeasurementSystem::Millimeters => 25.0,
            MeasurementSystem::Points => 72.0,
        },
    }
}
