#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperType {
    Letter,
    Legal,
    A4,
    A5,
    Custom,
}

impl PaperType {
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperType::Letter => (215.9, 279.4),
            PaperType::Legal => (215.9, 355.6),
            PaperType::A4 => (210.0, 297.0),
            PaperType::A5 => (148.0, 210.0),
            PaperType::Custom => (215.9, 279.4),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PaperType::Letter => "Letter",
            PaperType::Legal => "Legal",
            PaperType::A4 => "A4",
            PaperType::A5 => "A5",
            PaperType::Custom => "Custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeasurementSystem {
    Inches,
    Millimeters,
    Points,
}

impl MeasurementSystem {
    pub fn name(&self) -> &'static str {
        match self {
            MeasurementSystem::Inches => "in",
            MeasurementSystem::Millimeters => "mm",
            MeasurementSystem::Points => "pt",
        }
    }

    pub fn to_mm(&self, value: f32) -> f32 {
        match self {
            MeasurementSystem::Inches => value * 25.4,
            MeasurementSystem::Millimeters => value,
            MeasurementSystem::Points => value * 0.352778,
        }
    }

    pub fn from_mm(&self, value: f32) -> f32 {
        match self {
            MeasurementSystem::Inches => value / 25.4,
            MeasurementSystem::Millimeters => value,
            MeasurementSystem::Points => value / 0.352778,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlashcardOptions {
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub margin_top_mm: f32,
    pub margin_bottom_mm: f32,
    pub margin_left_mm: f32,
    pub margin_right_mm: f32,
    pub card_width_mm: f32,
    pub card_height_mm: f32,
    pub rows: usize,
    pub columns: usize,
    pub row_spacing_mm: f32,
    pub column_spacing_mm: f32,
    pub font_size_pt: f32,
}

impl Default for FlashcardOptions {
    fn default() -> Self {
        Self {
            page_width_mm: 215.9,
            page_height_mm: 279.4,
            margin_top_mm: 10.0,
            margin_bottom_mm: 10.0,
            margin_left_mm: 10.0,
            margin_right_mm: 10.0,
            card_width_mm: 63.5,
            card_height_mm: 88.9,
            rows: 2,
            columns: 3,
            row_spacing_mm: 5.0,
            column_spacing_mm: 5.0,
            font_size_pt: 12.0,
        }
    }
}
