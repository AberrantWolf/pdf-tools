//! Shared print units, paper sizes, orientation, and margin types.
//!
//! This crate is the single source of truth for physical-unit handling across
//! the print-junk tools (imposition, flashcards, typesetting). Keeping it free
//! of PDF/I-O dependencies lets every feature crate — and the WASM build —
//! reuse the same dimensions and conversions.

// =============================================================================
// Unit Conversion
// =============================================================================

/// Points per millimeter (1 inch = 72 points, 1 inch = 25.4mm)
pub const POINTS_PER_MM: f32 = 72.0 / 25.4; // ≈ 2.83465

/// Convert millimeters to points
#[inline]
pub fn mm_to_pt(mm: f32) -> f32 {
    mm * POINTS_PER_MM
}

/// Convert points to millimeters
#[inline]
pub fn pt_to_mm(pt: f32) -> f32 {
    pt / POINTS_PER_MM
}

// =============================================================================
// Orientation
// =============================================================================

/// Paper orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    /// Portrait: height > width
    Portrait,
    /// Landscape: width > height (default for imposition — pages are arranged side by side)
    #[default]
    Landscape,
}

impl Orientation {
    /// Returns true if landscape orientation
    pub fn is_landscape(self) -> bool {
        matches!(self, Orientation::Landscape)
    }

    /// Returns the opposite orientation
    pub fn flip(self) -> Self {
        match self {
            Orientation::Portrait => Orientation::Landscape,
            Orientation::Landscape => Orientation::Portrait,
        }
    }
}

// =============================================================================
// Paper Sizes
// =============================================================================

/// Standard paper sizes
///
/// All dimensions are stored in portrait orientation (width < height).
/// Use `dimensions_with_orientation` to get landscape dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PaperSize {
    /// ISO A3 (297mm × 420mm)
    A3,
    /// ISO A4 (210mm × 297mm)
    A4,
    /// ISO A5 (148mm × 210mm)
    A5,
    /// ISO B4 (250mm × 353mm)
    B4,
    /// ISO B5 (176mm × 250mm)
    B5,
    /// US Letter (8.5" × 11")
    #[default]
    Letter,
    /// US Legal (8.5" × 14")
    Legal,
    /// US Tabloid (11" × 17")
    Tabloid,
    /// Custom dimensions in millimeters
    Custom { width_mm: f32, height_mm: f32 },
}

impl PaperSize {
    /// Get base dimensions in millimeters (always portrait: width < height for standard sizes)
    pub fn dimensions_mm(self) -> (f32, f32) {
        match self {
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::B4 => (250.0, 353.0),
            PaperSize::B5 => (176.0, 250.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::Tabloid => (279.4, 431.8),
            PaperSize::Custom {
                width_mm,
                height_mm,
            } => (width_mm, height_mm),
        }
    }

    /// Get dimensions with orientation applied
    pub fn dimensions_with_orientation(self, orientation: Orientation) -> (f32, f32) {
        let (w, h) = self.dimensions_mm();
        match orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }

    /// Get dimensions in points (1/72 inch)
    pub fn dimensions_pt(self) -> (f32, f32) {
        let (w, h) = self.dimensions_mm();
        (mm_to_pt(w), mm_to_pt(h))
    }

    /// Get dimensions in points with orientation applied
    pub fn dimensions_pt_with_orientation(self, orientation: Orientation) -> (f32, f32) {
        let (w, h) = self.dimensions_with_orientation(orientation);
        (mm_to_pt(w), mm_to_pt(h))
    }

    /// Reverse-map portrait dimensions (mm) to a standard size, falling back to
    /// [`PaperSize::Custom`] when they don't match a known size within tolerance.
    /// Used when loading a saved layout to restore the paper-size selection.
    pub fn from_dimensions_mm(width_mm: f32, height_mm: f32) -> Self {
        const TOLERANCE_MM: f32 = 0.5;
        for paper in [
            Self::A3,
            Self::A4,
            Self::A5,
            Self::B4,
            Self::B5,
            Self::Letter,
            Self::Legal,
            Self::Tabloid,
        ] {
            let (w, h) = paper.dimensions_mm();
            if (w - width_mm).abs() < TOLERANCE_MM && (h - height_mm).abs() < TOLERANCE_MM {
                return paper;
            }
        }
        Self::Custom {
            width_mm,
            height_mm,
        }
    }
}

// A hand-written serde representation so saved configs stay human-readable:
// standard sizes serialize as a plain string ("A5"), and `Custom` as a bare
// `{ "width_mm": …, "height_mm": … }` object.
#[cfg(feature = "serde")]
impl serde::Serialize for PaperSize {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        match self {
            PaperSize::A3 => serializer.serialize_str("A3"),
            PaperSize::A4 => serializer.serialize_str("A4"),
            PaperSize::A5 => serializer.serialize_str("A5"),
            PaperSize::B4 => serializer.serialize_str("B4"),
            PaperSize::B5 => serializer.serialize_str("B5"),
            PaperSize::Letter => serializer.serialize_str("Letter"),
            PaperSize::Legal => serializer.serialize_str("Legal"),
            PaperSize::Tabloid => serializer.serialize_str("Tabloid"),
            PaperSize::Custom {
                width_mm,
                height_mm,
            } => {
                let mut s = serializer.serialize_struct("Custom", 2)?;
                s.serialize_field("width_mm", width_mm)?;
                s.serialize_field("height_mm", height_mm)?;
                s.end()
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PaperSize {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct PaperSizeVisitor;

        impl<'de> Visitor<'de> for PaperSizeVisitor {
            type Value = PaperSize;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a paper size")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<PaperSize, E>
            where
                E: de::Error,
            {
                match value {
                    "A3" => Ok(PaperSize::A3),
                    "A4" => Ok(PaperSize::A4),
                    "A5" => Ok(PaperSize::A5),
                    "B4" => Ok(PaperSize::B4),
                    "B5" => Ok(PaperSize::B5),
                    "Letter" => Ok(PaperSize::Letter),
                    "Legal" => Ok(PaperSize::Legal),
                    "Tabloid" => Ok(PaperSize::Tabloid),
                    _ => Err(de::Error::unknown_variant(
                        value,
                        &[
                            "A3", "A4", "A5", "B4", "B5", "Letter", "Legal", "Tabloid", "Custom",
                        ],
                    )),
                }
            }

            fn visit_map<M>(self, mut map: M) -> std::result::Result<PaperSize, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut width_mm = None;
                let mut height_mm = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "width_mm" => width_mm = Some(map.next_value()?),
                        "height_mm" => height_mm = Some(map.next_value()?),
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                match (width_mm, height_mm) {
                    (Some(w), Some(h)) => Ok(PaperSize::Custom {
                        width_mm: w,
                        height_mm: h,
                    }),
                    _ => Err(de::Error::missing_field("width_mm or height_mm")),
                }
            }
        }

        deserializer.deserialize_any(PaperSizeVisitor)
    }
}

// =============================================================================
// Margins
// =============================================================================

/// Sheet margins - printer-safe area around the entire output sheet.
///
/// These margins ensure content stays within the printer's printable area.
/// 10mm default ensures printer's marks (crop marks, registration marks) remain visible
/// even on consumer printers that can't print to the edge.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SheetMargins {
    pub top_mm: f32,
    pub bottom_mm: f32,
    pub left_mm: f32,
    pub right_mm: f32,
}

impl Default for SheetMargins {
    fn default() -> Self {
        Self::uniform(10.0)
    }
}

impl SheetMargins {
    /// Create uniform margins on all sides
    pub fn uniform(margin_mm: f32) -> Self {
        Self {
            top_mm: margin_mm,
            bottom_mm: margin_mm,
            left_mm: margin_mm,
            right_mm: margin_mm,
        }
    }

    /// Create with no margins (borderless)
    pub fn none() -> Self {
        Self::uniform(0.0)
    }

    /// Total horizontal margin (left + right)
    pub fn horizontal_mm(&self) -> f32 {
        self.left_mm + self.right_mm
    }

    /// Total vertical margin (top + bottom)
    pub fn vertical_mm(&self) -> f32 {
        self.top_mm + self.bottom_mm
    }
}

/// Leaf margins - applied to each logical page within the imposed sheet.
///
/// These provide:
/// - Trim space for cutting after folding
/// - Spine gutter for readability when bound
/// - Consistent page margins in the final book
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct LeafMargins {
    /// Top margin (head) of each leaf
    pub top_mm: f32,
    /// Bottom margin (tail) of each leaf
    pub bottom_mm: f32,
    /// Outer margin (fore edge) - the edge opposite the spine
    pub fore_edge_mm: f32,
    /// Inner margin (spine/gutter) - extra space near the binding
    pub spine_mm: f32,
    /// Trim allowance - extra material around fold edges, trimmed away after binding (3mm standard)
    pub trim_allowance_mm: f32,
}

impl Default for LeafMargins {
    fn default() -> Self {
        Self {
            top_mm: 0.0,
            bottom_mm: 0.0,
            fore_edge_mm: 0.0,
            spine_mm: 0.0,
            trim_allowance_mm: 3.0,
        }
    }
}

impl LeafMargins {
    /// Create uniform margins (except spine and trim allowance)
    pub fn uniform(margin_mm: f32) -> Self {
        Self {
            top_mm: margin_mm,
            bottom_mm: margin_mm,
            fore_edge_mm: margin_mm,
            spine_mm: margin_mm,
            trim_allowance_mm: 3.0,
        }
    }
}

/// Combined margins for imposition
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Margins {
    /// Printer-safe margins around the entire output sheet
    pub sheet: SheetMargins,
    /// Margins for each logical page/leaf
    pub leaf: LeafMargins,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_round_trips() {
        assert!((POINTS_PER_MM - 72.0 / 25.4).abs() < f32::EPSILON);
        assert!((pt_to_mm(mm_to_pt(123.4)) - 123.4).abs() < 1e-3);
        // A4 height in points (297mm) ≈ 841.89pt
        assert!((mm_to_pt(297.0) - 841.89).abs() < 0.1);
    }

    #[test]
    fn paper_dimensions_are_portrait() {
        assert_eq!(PaperSize::A5.dimensions_mm(), (148.0, 210.0));
        assert_eq!(PaperSize::A4.dimensions_mm(), (210.0, 297.0));
        // Landscape swaps the axes.
        assert_eq!(
            PaperSize::A5.dimensions_with_orientation(Orientation::Landscape),
            (210.0, 148.0)
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn paper_size_serde_wire_format() {
        // Standard sizes serialize as a bare string…
        assert_eq!(serde_json::to_string(&PaperSize::A5).unwrap(), "\"A5\"");
        assert_eq!(
            serde_json::from_str::<PaperSize>("\"Letter\"").unwrap(),
            PaperSize::Letter
        );
        // …and Custom as a flat object, round-tripping cleanly.
        let custom = PaperSize::Custom {
            width_mm: 156.0,
            height_mm: 234.0,
        };
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(json, r#"{"width_mm":156.0,"height_mm":234.0}"#);
        assert_eq!(serde_json::from_str::<PaperSize>(&json).unwrap(), custom);
    }
}
