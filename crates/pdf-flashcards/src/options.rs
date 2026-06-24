#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
            MeasurementSystem::Points => value * 0.352_778,
        }
    }

    pub fn from_mm(&self, value: f32) -> f32 {
        match self {
            MeasurementSystem::Inches => value / 25.4,
            MeasurementSystem::Millimeters => value,
            MeasurementSystem::Points => value / 0.352_778,
        }
    }
}

use crate::types::{FlashcardError, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

impl FlashcardOptions {
    pub fn validate(&self) -> Result<()> {
        if self.rows == 0 {
            return Err(FlashcardError::Pdf("rows must be at least 1".into()));
        }
        if self.columns == 0 {
            return Err(FlashcardError::Pdf("columns must be at least 1".into()));
        }
        if self.card_width_mm <= 0.0 {
            return Err(FlashcardError::Pdf("card width must be positive".into()));
        }
        if self.card_height_mm <= 0.0 {
            return Err(FlashcardError::Pdf("card height must be positive".into()));
        }
        if self.font_size_pt <= 0.0 {
            return Err(FlashcardError::Pdf("font size must be positive".into()));
        }
        if self.page_width_mm <= 0.0 || self.page_height_mm <= 0.0 {
            return Err(FlashcardError::Pdf(
                "page dimensions must be positive".into(),
            ));
        }
        Ok(())
    }
}
