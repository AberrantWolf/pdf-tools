use crate::types::*;
use std::path::PathBuf;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Comprehensive imposition configuration
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImpositionOptions {
    // Input
    pub input_files: Vec<PathBuf>,

    // Binding and arrangement
    pub binding_type: BindingType,
    pub page_arrangement: PageArrangement,

    // Output configuration
    pub output_paper_size: PaperSize,
    pub output_format: OutputFormat,
    pub scaling_mode: ScalingMode,

    // Margins
    pub margins: Margins,

    // Printer's marks
    pub marks: PrinterMarks,

    // Page numbering
    pub add_page_numbers: bool,
    pub page_number_start: usize,

    // Flyleaves
    pub front_flyleaves: usize,
    pub back_flyleaves: usize,

    // Output splitting
    pub split_mode: SplitMode,

    // Rotation for source pages
    pub source_rotation: Rotation,
}

impl Default for ImpositionOptions {
    fn default() -> Self {
        Self {
            input_files: Vec::new(),
            binding_type: BindingType::Signature,
            page_arrangement: PageArrangement::Quarto,
            output_paper_size: PaperSize::Letter,
            output_format: OutputFormat::DoubleSided,
            scaling_mode: ScalingMode::Fit,
            margins: Margins::default(),
            marks: PrinterMarks::default(),
            add_page_numbers: false,
            page_number_start: 1,
            front_flyleaves: 0,
            back_flyleaves: 0,
            split_mode: SplitMode::None,
            source_rotation: Rotation::None,
        }
    }
}

impl ImpositionOptions {
    /// Load options from JSON file
    #[cfg(feature = "serde")]
    pub async fn load(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let bytes = tokio::fs::read(path).await?;
        let options = serde_json::from_slice(&bytes)
            .map_err(|e| ImposeError::Config(format!("Failed to parse config: {}", e)))?;
        Ok(options)
    }

    /// Save options to JSON file
    #[cfg(feature = "serde")]
    pub async fn save(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| ImposeError::Config(format!("Failed to serialize config: {}", e)))?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    /// Validate the options
    pub fn validate(&self) -> Result<()> {
        if self.input_files.is_empty() {
            return Err(ImposeError::Config("No input files specified".to_string()));
        }

        let pages_per_sig = self.page_arrangement.pages_per_signature();
        if pages_per_sig == 0 || pages_per_sig % 4 != 0 {
            return Err(ImposeError::Config(
                "Pages per signature must be a multiple of 4".to_string(),
            ));
        }

        // Validate output format compatibility with binding type
        match (self.binding_type, self.output_format) {
            // Signature and case binding work with all output formats
            (BindingType::Signature, _) | (BindingType::CaseBinding, _) => {}

            // Perfect binding, side stitch, and spiral typically use double-sided or single-sided
            // TwoSided (separate front/back PDFs) doesn't make sense for these bindings
            (BindingType::PerfectBinding, OutputFormat::TwoSided)
            | (BindingType::SideStitch, OutputFormat::TwoSided)
            | (BindingType::Spiral, OutputFormat::TwoSided) => {
                return Err(ImposeError::Config(format!(
                    "{:?} binding does not support TwoSided output format. Use DoubleSided or SingleSidedSequence.",
                    self.binding_type
                )));
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::*;
    use serde::{Deserialize, Serialize};

    // Manual implementations for types that don't derive Serialize/Deserialize
    impl Serialize for BindingType {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let s = match self {
                BindingType::Signature => "Signature",
                BindingType::PerfectBinding => "PerfectBinding",
                BindingType::SideStitch => "SideStitch",
                BindingType::Spiral => "Spiral",
                BindingType::CaseBinding => "CaseBinding",
            };
            serializer.serialize_str(s)
        }
    }

    #[cfg(feature = "serde")]
    impl<'de> Deserialize<'de> for BindingType {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "Signature" => Ok(BindingType::Signature),
                "PerfectBinding" => Ok(BindingType::PerfectBinding),
                "SideStitch" => Ok(BindingType::SideStitch),
                "Spiral" => Ok(BindingType::Spiral),
                "CaseBinding" => Ok(BindingType::CaseBinding),
                _ => Err(serde::de::Error::custom("Unknown binding type")),
            }
        }
    }

    impl Serialize for PageArrangement {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::ser::SerializeStruct;
            match self {
                PageArrangement::Folio => serializer.serialize_str("Folio"),
                PageArrangement::Quarto => serializer.serialize_str("Quarto"),
                PageArrangement::Octavo => serializer.serialize_str("Octavo"),
                PageArrangement::Custom {
                    pages_per_signature,
                } => {
                    let mut s = serializer.serialize_struct("Custom", 1)?;
                    s.serialize_field("pages_per_signature", pages_per_signature)?;
                    s.end()
                }
            }
        }
    }

    impl<'de> Deserialize<'de> for PageArrangement {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::{self, MapAccess, Visitor};
            use std::fmt;

            struct PageArrangementVisitor;

            impl<'de> Visitor<'de> for PageArrangementVisitor {
                type Value = PageArrangement;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a page arrangement type")
                }

                fn visit_str<E>(self, value: &str) -> std::result::Result<PageArrangement, E>
                where
                    E: de::Error,
                {
                    match value {
                        "Folio" => Ok(PageArrangement::Folio),
                        "Quarto" => Ok(PageArrangement::Quarto),
                        "Octavo" => Ok(PageArrangement::Octavo),
                        _ => Err(de::Error::unknown_variant(
                            value,
                            &["Folio", "Quarto", "Octavo", "Custom"],
                        )),
                    }
                }

                fn visit_map<M>(self, mut map: M) -> std::result::Result<PageArrangement, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut pages_per_signature = None;
                    while let Some(key) = map.next_key::<String>()? {
                        match key.as_str() {
                            "pages_per_signature" => {
                                pages_per_signature = Some(map.next_value()?);
                            }
                            _ => {
                                let _: serde::de::IgnoredAny = map.next_value()?;
                            }
                        }
                    }

                    if let Some(pps) = pages_per_signature {
                        Ok(PageArrangement::Custom {
                            pages_per_signature: pps,
                        })
                    } else {
                        Err(de::Error::missing_field("pages_per_signature"))
                    }
                }
            }

            deserializer.deserialize_any(PageArrangementVisitor)
        }
    }

    impl Serialize for PaperSize {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::ser::SerializeStruct;
            match self {
                PaperSize::A3 => serializer.serialize_str("A3"),
                PaperSize::A4 => serializer.serialize_str("A4"),
                PaperSize::A5 => serializer.serialize_str("A5"),
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

    impl<'de> Deserialize<'de> for PaperSize {
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
                        "Letter" => Ok(PaperSize::Letter),
                        "Legal" => Ok(PaperSize::Legal),
                        "Tabloid" => Ok(PaperSize::Tabloid),
                        _ => Err(de::Error::unknown_variant(
                            value,
                            &["A3", "A4", "A5", "Letter", "Legal", "Tabloid", "Custom"],
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

    // Simple derive-based implementations for remaining types
    impl Serialize for OutputFormat {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(match self {
                OutputFormat::DoubleSided => "DoubleSided",
                OutputFormat::TwoSided => "TwoSided",
                OutputFormat::SingleSidedSequence => "SingleSidedSequence",
            })
        }
    }

    impl<'de> Deserialize<'de> for OutputFormat {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "DoubleSided" => Ok(OutputFormat::DoubleSided),
                "TwoSided" => Ok(OutputFormat::TwoSided),
                "SingleSidedSequence" => Ok(OutputFormat::SingleSidedSequence),
                _ => Err(serde::de::Error::custom("Unknown output format")),
            }
        }
    }

    impl Serialize for ScalingMode {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(match self {
                ScalingMode::Fit => "Fit",
                ScalingMode::Fill => "Fill",
                ScalingMode::None => "None",
                ScalingMode::Stretch => "Stretch",
            })
        }
    }

    impl<'de> Deserialize<'de> for ScalingMode {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "Fit" => Ok(ScalingMode::Fit),
                "Fill" => Ok(ScalingMode::Fill),
                "None" => Ok(ScalingMode::None),
                "Stretch" => Ok(ScalingMode::Stretch),
                _ => Err(serde::de::Error::custom("Unknown scaling mode")),
            }
        }
    }

    impl Serialize for Rotation {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_str(match self {
                Rotation::None => "None",
                Rotation::Clockwise90 => "Clockwise90",
                Rotation::Clockwise180 => "Clockwise180",
                Rotation::Clockwise270 => "Clockwise270",
            })
        }
    }

    impl<'de> Deserialize<'de> for Rotation {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            match s.as_str() {
                "None" => Ok(Rotation::None),
                "Clockwise90" => Ok(Rotation::Clockwise90),
                "Clockwise180" => Ok(Rotation::Clockwise180),
                "Clockwise270" => Ok(Rotation::Clockwise270),
                _ => Err(serde::de::Error::custom("Unknown rotation")),
            }
        }
    }

    impl Serialize for SplitMode {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::ser::SerializeStruct;
            match self {
                SplitMode::None => serializer.serialize_str("None"),
                SplitMode::ByPages(n) => {
                    let mut s = serializer.serialize_struct("ByPages", 1)?;
                    s.serialize_field("pages", n)?;
                    s.end()
                }
                SplitMode::BySheets(n) => {
                    let mut s = serializer.serialize_struct("BySheets", 1)?;
                    s.serialize_field("sheets", n)?;
                    s.end()
                }
                SplitMode::BySignatures(n) => {
                    let mut s = serializer.serialize_struct("BySignatures", 1)?;
                    s.serialize_field("signatures", n)?;
                    s.end()
                }
            }
        }
    }

    impl<'de> Deserialize<'de> for SplitMode {
        fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::de::{self, MapAccess, Visitor};
            use std::fmt;

            struct SplitModeVisitor;

            impl<'de> Visitor<'de> for SplitModeVisitor {
                type Value = SplitMode;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a split mode")
                }

                fn visit_str<E>(self, value: &str) -> std::result::Result<SplitMode, E>
                where
                    E: de::Error,
                {
                    match value {
                        "None" => Ok(SplitMode::None),
                        _ => Err(de::Error::custom("Unknown split mode")),
                    }
                }

                fn visit_map<M>(self, mut map: M) -> std::result::Result<SplitMode, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut pages = None;
                    let mut sheets = None;
                    let mut signatures = None;

                    while let Some(key) = map.next_key::<String>()? {
                        match key.as_str() {
                            "pages" => pages = Some(map.next_value()?),
                            "sheets" => sheets = Some(map.next_value()?),
                            "signatures" => signatures = Some(map.next_value()?),
                            _ => {
                                let _: serde::de::IgnoredAny = map.next_value()?;
                            }
                        }
                    }

                    if let Some(p) = pages {
                        Ok(SplitMode::ByPages(p))
                    } else if let Some(s) = sheets {
                        Ok(SplitMode::BySheets(s))
                    } else if let Some(sig) = signatures {
                        Ok(SplitMode::BySignatures(sig))
                    } else {
                        Err(de::Error::missing_field("pages, sheets, or signatures"))
                    }
                }
            }

            deserializer.deserialize_any(SplitModeVisitor)
        }
    }
} // end of serde_impls module
