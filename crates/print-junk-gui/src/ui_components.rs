use eframe::egui;
use pdf_units::PaperSize;
use std::path::PathBuf;

/// A section heading for flat settings panels: a brass, uppercase "eyebrow" with
/// a hairline rule running off to the right — a deliberately different voice from
/// the per-control labels beneath it, so the panel's structure stays scannable.
pub fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let label = ui.label(
            egui::RichText::new(text.to_uppercase())
                .color(crate::theme::BRASS)
                .size(12.0),
        );
        // A hairline rule filling the remaining width, on the label's baseline.
        let rest = ui.available_rect_before_wrap();
        if rest.width() > 12.0 {
            ui.painter().hline(
                (rest.left() + 6.0)..=rest.right(),
                label.rect.center().y,
                egui::Stroke::new(1.0, crate::theme::HAIRLINE),
            );
        }
    });
    ui.add_space(2.0);
}

/// The standard named paper sizes offered in pickers (excludes `Custom`).
pub const STANDARD_PAPER_SIZES: [(PaperSize, &str); 8] = [
    (PaperSize::Letter, "Letter"),
    (PaperSize::Legal, "Legal"),
    (PaperSize::Tabloid, "Tabloid"),
    (PaperSize::A3, "A3"),
    (PaperSize::A4, "A4"),
    (PaperSize::A5, "A5"),
    (PaperSize::B4, "B4"),
    (PaperSize::B5, "B5"),
];

/// A shared paper-size combo box over [`STANDARD_PAPER_SIZES`].
///
/// Used by every mode that picks an output sheet/page size (imposition,
/// flashcards, typesetting) so the option list stays consistent. Returns `true`
/// if the selection changed.
pub fn paper_size_picker(ui: &mut egui::Ui, id: &str, label: &str, value: &mut PaperSize) -> bool {
    enum_selector(ui, id, label, value, &STANDARD_PAPER_SIZES)
}

/// Builder for creating sliders with automatic change tracking
pub struct SliderBuilder<'a, T> {
    value: &'a mut T,
    range: std::ops::RangeInclusive<T>,
    text: String,
    suffix: Option<String>,
}

impl<'a, T> SliderBuilder<'a, T>
where
    T: egui::emath::Numeric,
{
    pub fn new(value: &'a mut T, range: std::ops::RangeInclusive<T>) -> Self {
        Self {
            value,
            range,
            text: String::new(),
            suffix: None,
        }
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    #[allow(dead_code)]
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut slider =
            egui::Slider::new(self.value, self.range).clamping(egui::SliderClamping::Never);

        if !self.text.is_empty() {
            slider = slider.text(self.text);
        }

        if let Some(suffix) = self.suffix {
            slider = slider.suffix(suffix);
        }

        ui.add(slider).changed()
    }
}

/// Builder for creating drag values with automatic formatting
pub struct DragValueBuilder<'a, T> {
    value: &'a mut T,
    range: Option<std::ops::RangeInclusive<T>>,
    suffix: Option<String>,
    speed: Option<f32>,
}

impl<'a, T> DragValueBuilder<'a, T>
where
    T: egui::emath::Numeric,
{
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value,
            range: None,
            suffix: None,
            speed: None,
        }
    }

    pub fn range(mut self, range: std::ops::RangeInclusive<T>) -> Self {
        self.range = Some(range);
        self
    }

    #[allow(dead_code)]
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    #[allow(dead_code)]
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = Some(speed);
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut drag = egui::DragValue::new(self.value);

        if let Some(range) = self.range {
            drag = drag.range(range);
        }

        if let Some(suffix) = self.suffix {
            drag = drag.suffix(suffix);
        }

        if let Some(speed) = self.speed {
            drag = drag.speed(speed);
        }

        ui.add(drag).changed()
    }
}

#[allow(dead_code)]
pub fn labeled_drag<T>(ui: &mut egui::Ui, label: &str, value: &mut T) -> bool
where
    T: egui::emath::Numeric,
{
    ui.horizontal(|ui| {
        ui.label(label);
        DragValueBuilder::new(value).show(ui)
    })
    .inner
}

#[allow(dead_code)]
pub fn labeled_drag_with_suffix<T>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    suffix: &str,
) -> bool
where
    T: egui::emath::Numeric,
{
    ui.horizontal(|ui| {
        ui.label(label);
        DragValueBuilder::new(value).suffix(suffix).show(ui)
    })
    .inner
}

/// Helper for creating labeled horizontal drag values with range and suffix
pub fn labeled_drag_clamped<T>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    suffix: &str,
) -> bool
where
    T: egui::emath::Numeric,
{
    ui.horizontal(|ui| {
        ui.label(label);
        DragValueBuilder::new(value)
            .range(range)
            .suffix(suffix)
            .show(ui)
    })
    .inner
}

/// Helper for labeled drag values with range, suffix, and tooltip
pub fn labeled_drag_clamped_with_tooltip<T>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    suffix: &str,
    tooltip: &str,
) -> bool
where
    T: egui::emath::Numeric,
{
    ui.horizontal(|ui| {
        ui.label(label).on_hover_text(tooltip);
        DragValueBuilder::new(value)
            .range(range)
            .suffix(suffix)
            .show(ui)
    })
    .inner
}

/// Enum selector using `ComboBox`
pub fn enum_selector<T>(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool
where
    T: PartialEq + Clone,
{
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);

        let current_text = options
            .iter()
            .find(|(v, _)| v == value)
            .map_or("Unknown", |(_, text)| *text);

        egui::ComboBox::from_id_salt(id)
            .selected_text(current_text)
            .show_ui(ui, |ui| {
                for (option_value, option_text) in options {
                    if ui
                        .selectable_value(value, option_value.clone(), *option_text)
                        .changed()
                    {
                        changed = true;
                    }
                }
            });
    });
    changed
}

/// Horizontal button group for enum selection
pub fn button_group<T>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> bool
where
    T: PartialEq + Clone,
{
    let mut changed = false;
    ui.horizontal(|ui| {
        for (option_value, option_text) in options {
            if ui
                .selectable_value(value, option_value.clone(), *option_text)
                .changed()
            {
                changed = true;
            }
        }
    });
    changed
}

/// File list editor with reordering and removal
pub struct FileListEditor<'a> {
    files: &'a mut Vec<PathBuf>,
    changed: bool,
}

impl<'a> FileListEditor<'a> {
    pub fn new(files: &'a mut Vec<PathBuf>) -> Self {
        Self {
            files,
            changed: false,
        }
    }

    pub fn show(mut self, ui: &mut egui::Ui) -> bool {
        if self.files.is_empty() {
            ui.label("No files selected");
            return false;
        }

        let mut to_remove = None;
        let mut to_move_up = None;
        let mut to_move_down = None;

        for (idx, path) in self.files.iter().enumerate() {
            ui.horizontal(|ui| {
                // Reorder buttons
                if idx > 0 && ui.small_button("▲").clicked() {
                    to_move_up = Some(idx);
                }
                if idx < self.files.len() - 1 && ui.small_button("▼").clicked() {
                    to_move_down = Some(idx);
                }

                let file_name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().to_string(),
                );
                ui.label(format!("{}. {}", idx + 1, file_name))
                    .on_hover_text(path.display().to_string());

                if ui.small_button("✖").clicked() {
                    to_remove = Some(idx);
                }
            });
        }

        // Apply changes
        if let Some(idx) = to_move_up {
            self.files.swap(idx, idx - 1);
            self.changed = true;
        }
        if let Some(idx) = to_move_down {
            self.files.swap(idx, idx + 1);
            self.changed = true;
        }
        if let Some(idx) = to_remove {
            self.files.remove(idx);
            self.changed = true;
        }

        self.changed
    }
}

/// Margin editor component (4-sided margins)
pub struct MarginsEditor<'a> {
    top: &'a mut f32,
    bottom: &'a mut f32,
    left: &'a mut f32,
    right: &'a mut f32,
    max: f32,
    unit: &'a str,
}

impl<'a> MarginsEditor<'a> {
    pub fn new(
        top: &'a mut f32,
        bottom: &'a mut f32,
        left: &'a mut f32,
        right: &'a mut f32,
        max: f32,
        unit: &'a str,
    ) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
            max,
            unit,
        }
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        changed |= SliderBuilder::new(self.top, 0.0..=self.max)
            .text(format!("Top ({})", self.unit))
            .show(ui);

        changed |= SliderBuilder::new(self.bottom, 0.0..=self.max)
            .text(format!("Bottom ({})", self.unit))
            .show(ui);

        changed |= SliderBuilder::new(self.left, 0.0..=self.max)
            .text(format!("Left ({})", self.unit))
            .show(ui);

        changed |= SliderBuilder::new(self.right, 0.0..=self.max)
            .text(format!("Right ({})", self.unit))
            .show(ui);

        changed
    }
}

/// Sheet margins editor (printer-safe area - uniform sides)
pub struct SheetMarginsEditor<'a> {
    top: &'a mut f32,
    bottom: &'a mut f32,
    left: &'a mut f32,
    right: &'a mut f32,
    max: f32,
}

impl<'a> SheetMarginsEditor<'a> {
    pub fn new(
        top: &'a mut f32,
        bottom: &'a mut f32,
        left: &'a mut f32,
        right: &'a mut f32,
        max: f32,
    ) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
            max,
        }
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        changed |= labeled_drag_clamped(ui, "Top:", self.top, 0.0..=self.max, " mm");
        changed |= labeled_drag_clamped(ui, "Bottom:", self.bottom, 0.0..=self.max, " mm");
        changed |= labeled_drag_clamped(ui, "Left:", self.left, 0.0..=self.max, " mm");
        changed |= labeled_drag_clamped(ui, "Right:", self.right, 0.0..=self.max, " mm");

        changed
    }
}

/// Leaf margins editor (trim and gutter - bookbinding terminology)
pub struct LeafMarginsEditor<'a> {
    top: &'a mut f32,
    bottom: &'a mut f32,
    fore_edge: &'a mut f32,
    spine: &'a mut f32,
    cut: &'a mut f32,
    max: f32,
}

impl<'a> LeafMarginsEditor<'a> {
    pub fn new(
        top: &'a mut f32,
        bottom: &'a mut f32,
        fore_edge: &'a mut f32,
        spine: &'a mut f32,
        cut: &'a mut f32,
        max: f32,
    ) -> Self {
        Self {
            top,
            bottom,
            fore_edge,
            spine,
            cut,
            max,
        }
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        changed |= labeled_drag_clamped_with_tooltip(
            ui,
            "Top (head):",
            self.top,
            0.0..=self.max,
            " mm",
            "Margin at the top (head) of the page",
        );
        changed |= labeled_drag_clamped_with_tooltip(
            ui,
            "Bottom (tail):",
            self.bottom,
            0.0..=self.max,
            " mm",
            "Margin at the bottom (tail/foot) of the page",
        );
        changed |= labeled_drag_clamped_with_tooltip(
            ui,
            "Fore edge:",
            self.fore_edge,
            0.0..=self.max,
            " mm",
            "Margin on the side opposite the spine",
        );
        changed |= labeled_drag_clamped_with_tooltip(
            ui,
            "Spine (gutter):",
            self.spine,
            0.0..=self.max,
            " mm",
            "Margin at the spine where pages are bound together",
        );
        changed |= labeled_drag_clamped_with_tooltip(
            ui,
            "Trim allowance:",
            self.cut,
            0.0..=self.max,
            " mm",
            "Extra material around fold edges, trimmed away after binding (3mm standard)",
        );

        changed
    }
}

/// Two-dimensional spacing editor
pub struct SpacingEditor<'a> {
    horizontal: &'a mut f32,
    vertical: &'a mut f32,
    h_label: &'a str,
    v_label: &'a str,
    max: f32,
    unit: &'a str,
}

impl<'a> SpacingEditor<'a> {
    pub fn new(
        horizontal: &'a mut f32,
        vertical: &'a mut f32,
        h_label: &'a str,
        v_label: &'a str,
        max: f32,
        unit: &'a str,
    ) -> Self {
        Self {
            horizontal,
            vertical,
            h_label,
            v_label,
            max,
            unit,
        }
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        changed |= SliderBuilder::new(self.vertical, 0.0..=self.max)
            .text(format!("{} ({})", self.v_label, self.unit))
            .show(ui);

        changed |= SliderBuilder::new(self.horizontal, 0.0..=self.max)
            .text(format!("{} ({})", self.h_label, self.unit))
            .show(ui);

        changed
    }
}
