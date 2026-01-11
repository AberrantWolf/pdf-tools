use eframe::egui;
use pdf_async_runtime::{DocumentId, PdfCommand};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ViewerState {
    pub current_doc_id: Option<DocumentId>,
    pub current_page: usize,
    pub total_pages: usize,
    pub page_texture: Option<egui::TextureHandle>,
}

impl ViewerState {
    pub fn new(doc_id: DocumentId, page_count: usize) -> Self {
        Self {
            current_doc_id: Some(doc_id),
            current_page: 0,
            total_pages: page_count,
            page_texture: None,
        }
    }
}

pub fn show_viewer(
    ui: &mut egui::Ui,
    viewer_state: &mut Option<ViewerState>,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
    status: &mut String,
) {
    if let Some(state) = viewer_state {
        // Show navigation bar
        ui.horizontal(|ui| {
            let can_go_back = state.current_page > 0;
            let can_go_forward = state.current_page < state.total_pages.saturating_sub(1);

            if ui
                .add_enabled(can_go_back, egui::Button::new("◀ Previous"))
                .clicked()
            {
                state.current_page -= 1;
                if let Some(doc_id) = state.current_doc_id {
                    let _ = command_tx.send(PdfCommand::ViewerRenderPage {
                        doc_id,
                        page_index: state.current_page,
                    });
                    *status = format!("Rendering page {}...", state.current_page + 1);
                }
            }

            ui.label(format!(
                "Page {} of {}",
                state.current_page + 1,
                state.total_pages
            ));

            if ui
                .add_enabled(can_go_forward, egui::Button::new("Next ▶"))
                .clicked()
            {
                state.current_page += 1;
                if let Some(doc_id) = state.current_doc_id {
                    let _ = command_tx.send(PdfCommand::ViewerRenderPage {
                        doc_id,
                        page_index: state.current_page,
                    });
                    *status = format!("Rendering page {}...", state.current_page + 1);
                }
            }

            ui.separator();

            if ui.button("Close PDF").clicked() {
                if let Some(doc_id) = state.current_doc_id {
                    let _ = command_tx.send(PdfCommand::ViewerClose { doc_id });
                }
            }
        });

        ui.separator();

        // Display page texture if available
        if let Some(texture) = &state.page_texture {
            // Center the image
            egui::ScrollArea::both().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.image((texture.id(), texture.size_vec2()));
                });
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("Rendering page...");
            });
        }

        // TODO: Add zoom controls
        // TODO: Add jump to page input
        // TODO: Add thumbnail sidebar
    } else {
        // No PDF loaded - show file loading UI
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.heading("PDF Viewer");
            ui.add_space(20.0);

            #[cfg(feature = "pdf-viewer")]
            {
                ui.label("Drop a PDF file here or click to open");
                ui.add_space(10.0);

                if ui.button("Open PDF...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .pick_file()
                    {
                        let _ = command_tx.send(PdfCommand::ViewerLoad { path });
                        *status = "Loading PDF...".to_string();
                    }
                }
            }

            #[cfg(not(feature = "pdf-viewer"))]
            {
                ui.label("PDF viewing not available in WASM build");
            }
        });
    }
}
