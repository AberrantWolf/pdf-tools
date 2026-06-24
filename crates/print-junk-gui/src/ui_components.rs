use eframe::egui;
use pdf_units::PaperSize;
use std::path::PathBuf;

/// Paint the brass "eyebrow": an uppercase brass label with a hairline rule
/// running off to the right — a deliberately different voice from the per-control
/// labels beneath it, so the panel's structure stays scannable. Shared by the
/// flat [`section_heading`] and the collapsible [`section`] header.
fn eyebrow(ui: &mut egui::Ui, text: &str) {
    let label = ui.label(
        egui::RichText::new(text.to_uppercase())
            .color(crate::theme::BRASS)
            .size(12.0),
    );
    // A hairline rule filling the remaining width, on the label's center line.
    let rest = ui.available_rect_before_wrap();
    if rest.width() > 12.0 {
        ui.painter().hline(
            (rest.left() + 6.0)..=rest.right(),
            label.rect.center().y,
            egui::Stroke::new(1.0, crate::theme::HAIRLINE),
        );
    }
}

/// A flat (non-collapsible) brass eyebrow section heading.
pub fn section_heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| eyebrow(ui, text));
    ui.add_space(2.0);
}

/// A collapsible settings section whose header is the brass eyebrow — the
/// Pressroom section voice, made foldable so dense panels stay scannable. `id`
/// must be unique per section; `default_open` sets the initial fold state.
pub fn section(
    ui: &mut egui::Ui,
    id: &str,
    title: &str,
    default_open: bool,
    contents: impl FnOnce(&mut egui::Ui),
) {
    ui.add_space(4.0);
    let header_id = ui.make_persistent_id(id);
    egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        header_id,
        default_open,
    )
    .show_header(ui, |ui| eyebrow(ui, title))
    .body(|ui| contents(ui));
}

/// A two-column "label : control" settings form. The label column auto-sizes to
/// the widest label (no magic widths) and inputs align in the second column.
/// Call [`form_row`] for each row inside `contents`.
pub fn form(ui: &mut egui::Ui, id: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([10.0, ui.spacing().item_spacing.y.max(6.0)])
        .show(ui, |ui| contents(ui));
}

/// One row of a [`form`]: a left-aligned label, then `add_control`. Keep labels
/// short so the gutter stays narrow. Returns whatever `add_control` returns
/// (e.g. a `changed` bool).
pub fn form_row<R>(
    ui: &mut egui::Ui,
    label: &str,
    add_control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(label);
    });
    let result = add_control(ui);
    ui.end_row();
    result
}

/// Like [`form_row`], but the label carries a muted info icon that reveals
/// `info` on hover — for short labels (e.g. bindery terms) whose full meaning is
/// worth a tooltip without lengthening the gutter.
pub fn form_row_info<R>(
    ui: &mut egui::Ui,
    label: &str,
    info: &str,
    add_control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(label).on_hover_text(info);
        info_icon(ui, info);
    });
    let result = add_control(ui);
    ui.end_row();
    result
}

/// Like [`form_row`], but dims the whole row (label + control) when `enabled` is
/// false — for inputs computed from another mode that shouldn't be edited there.
pub fn form_row_enabled<R>(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    add_control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.add_enabled(enabled, egui::Label::new(label));
    });
    let result = ui.add_enabled_ui(enabled, add_control).inner;
    ui.end_row();
    result
}

/// A small "i in a circle" info marker, painted (not a glyph, so it renders in
/// any font) and revealing `tooltip` on hover.
fn info_icon(ui: &mut egui::Ui, tooltip: &str) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(14.0, ui.spacing().interact_size.y),
        egui::Sense::hover(),
    );
    let c = rect.center();
    let color = crate::theme::NEWSPRINT;
    let painter = ui.painter();
    painter.circle_stroke(c, 5.5, egui::Stroke::new(1.0, color));
    painter.circle_filled(egui::pos2(c.x, c.y - 2.3), 1.0, color); // dot of the "i"
    painter.line_segment(
        [egui::pos2(c.x, c.y - 0.3), egui::pos2(c.x, c.y + 2.8)],
        egui::Stroke::new(1.2, color),
    ); // stem of the "i"
    response.on_hover_text(tooltip);
}

/// A fixed-width numeric field for form rows, so values with different digit
/// counts keep the same box size. Returns whether the value changed.
pub fn num_field<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    suffix: &str,
) -> bool {
    const WIDTH: f32 = 66.0;
    ui.add_sized(
        [WIDTH, ui.spacing().interact_size.y],
        egui::DragValue::new(value).range(range).suffix(suffix),
    )
    .changed()
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

/// Control-only enum dropdown (no label), filling the available width — for use
/// as the control inside a [`form_row`].
pub fn enum_combo<T>(ui: &mut egui::Ui, id: &str, value: &mut T, options: &[(T, &str)]) -> bool
where
    T: PartialEq + Clone,
{
    let mut changed = false;
    let current = options
        .iter()
        .find(|(v, _)| v == value)
        .map_or("Unknown", |(_, t)| *t);
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (option_value, option_text) in options {
                changed |= ui
                    .selectable_value(value, option_value.clone(), *option_text)
                    .changed();
            }
        });
    changed
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
