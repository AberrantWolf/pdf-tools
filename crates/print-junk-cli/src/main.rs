use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pdft", about = "Print Junk CLI", version)]
struct Cli {
    /// Enable verbose diagnostic output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate flashcard PDF from CSV
    Flashcards {
        /// Input CSV file (columns: front, back)
        #[arg(short, long)]
        input: PathBuf,

        /// Output PDF file
        #[arg(short, long)]
        output: PathBuf,

        /// Rows per page
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u64).range(1..))]
        rows: u64,

        /// Columns per page
        #[arg(long, default_value = "3", value_parser = clap::value_parser!(u64).range(1..))]
        columns: u64,

        /// Card width in inches
        #[arg(long, default_value = "2.5", value_parser = positive_f32)]
        card_width_in: f32,

        /// Card height in inches
        #[arg(long, default_value = "3.5", value_parser = positive_f32)]
        card_height_in: f32,
    },

    /// Import an HTML / arXiv document and re-typeset it to PDF
    Import {
        /// Source: a URL, an arXiv id/abs URL (e.g. 2310.12345), or a local HTML file
        source: String,

        /// Output PDF file
        #[arg(short, long)]
        output: PathBuf,

        /// Output paper size
        #[arg(long, default_value = "a4", value_enum)]
        paper: PaperArg,

        /// Uniform page margin in millimeters (overrides the template default)
        #[arg(long, value_parser = positive_f32)]
        margin_mm: Option<f32>,
    },

    /// Impose PDF pages for bookbinding
    Impose {
        /// Input PDF file(s) - can specify multiple
        #[arg(short, long, required = true, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Output PDF file
        #[arg(short, long)]
        output: PathBuf,

        /// Binding type
        #[arg(long, default_value = "signature", value_enum)]
        binding: BindingArg,

        /// Page arrangement (fold type)
        #[arg(long, default_value = "folio", value_enum)]
        arrangement: ArrangementArg,

        /// Number of sheets nested per signature
        #[arg(long, default_value = "1")]
        sheets_per_signature: usize,

        /// Output paper size
        #[arg(long, default_value = "letter", value_enum)]
        paper: PaperArg,

        /// Output orientation
        #[arg(long, default_value = "landscape", value_enum)]
        orientation: OrientationArg,

        /// Output format
        #[arg(long, default_value = "double-sided", value_enum)]
        format: FormatArg,

        /// Scaling mode
        #[arg(long, default_value = "fit", value_enum)]
        scaling: ScalingArg,

        /// Number of blank pages at front
        #[arg(long, default_value = "0")]
        front_flyleaves: usize,

        /// Number of blank pages at back
        #[arg(long, default_value = "0")]
        back_flyleaves: usize,

        /// Split output into one PDF per N signatures (signature/case binding only).
        /// Files are named `{output-stem}-signature-{N}.{ext}`.
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
        split_by_signatures: Option<u64>,

        /// Add fold lines (including spine fold)
        #[arg(long)]
        fold_lines: bool,

        /// Add trim marks (guillotine guides at inter-spread fold edges)
        #[arg(long)]
        trim_marks: bool,

        /// Add crop marks (at sheet edges)
        #[arg(long)]
        crop_marks: bool,

        /// Add registration marks
        #[arg(long)]
        registration_marks: bool,

        /// Add sewing station marks along spine fold
        #[arg(long)]
        sewing_marks: bool,

        /// Add collation marks (back marks) on spine edge
        #[arg(long)]
        collation_marks: bool,

        /// Gray value for interior marks (fold, trim, sewing): 0.0 = black, 1.0 = white
        #[arg(long, default_value = "0.0", value_parser = non_negative_f32)]
        interior_marks_gray: f32,

        /// Line width scale for interior marks (fold, trim, sewing): 1.0 = default
        #[arg(long, default_value = "1.0", value_parser = non_negative_f32)]
        interior_marks_line_scale: f32,

        /// Gray value for exterior marks (crop, registration, collation): 0.0 = black, 1.0 = white
        #[arg(long, default_value = "0.0", value_parser = non_negative_f32)]
        exterior_marks_gray: f32,

        /// Line width scale for exterior marks (crop, registration, collation): 1.0 = default
        #[arg(long, default_value = "1.0", value_parser = non_negative_f32)]
        exterior_marks_line_scale: f32,

        /// Number of sewing stations between kettle stitches
        #[arg(long, default_value = "3")]
        sewing_stations: usize,

        /// Distance from head/tail to kettle stitch holes in mm
        #[arg(long, default_value = "12.0", value_parser = non_negative_f32)]
        kettle_offset: f32,

        /// Sheet margin in mm (uniform on all sides)
        #[arg(long, default_value = "5.0", value_parser = non_negative_f32)]
        sheet_margin: f32,

        /// Leaf spine/gutter margin in mm (inner edge near binding)
        #[arg(long, default_value = "0.0", value_parser = non_negative_f32)]
        leaf_spine_margin: f32,

        /// Leaf fore-edge margin in mm (outer edge)
        #[arg(long, default_value = "0.0", value_parser = non_negative_f32)]
        leaf_fore_edge_margin: f32,

        /// Leaf top margin in mm
        #[arg(long, default_value = "0.0", value_parser = non_negative_f32)]
        leaf_top_margin: f32,

        /// Leaf bottom margin in mm
        #[arg(long, default_value = "0.0", value_parser = non_negative_f32)]
        leaf_bottom_margin: f32,

        /// Trim allowance in mm (extra material around fold edges, trimmed after binding)
        #[arg(long, default_value = "3.0", value_parser = non_negative_f32)]
        trim_allowance: f32,

        /// Creep compensation mode
        #[arg(long, default_value = "none", value_enum)]
        creep_mode: CreepModeArg,

        /// Creep per nested leaf layer in mm (used with --creep-mode per-layer)
        #[arg(long, default_value = "0.1", value_parser = non_negative_f32)]
        creep_per_layer: f32,

        /// Paper caliper (thickness) in mm (used with --creep-mode from-caliper)
        #[arg(long, default_value = "0.1", value_parser = non_negative_f32)]
        paper_thickness: f32,

        /// Show statistics only, don't generate PDF
        #[arg(long)]
        stats_only: bool,

        /// Cascade: number of columns in the grid
        #[arg(long, default_value = "1")]
        cascade_cols: usize,

        /// Cascade: number of rows in the grid
        #[arg(long, default_value = "1")]
        cascade_rows: usize,

        /// Cascade: margin between cells in mm
        #[arg(long, default_value = "5.0", value_parser = non_negative_f32)]
        cascade_margin: f32,

        /// Cascade: add cut lines between cells
        #[arg(long)]
        cascade_cut_lines: bool,

        /// Cascade: duplex flip axis
        #[arg(long, default_value = "long-edge", value_enum)]
        cascade_flip: FlipArg,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CreepModeArg {
    None,
    PerLayer,
    FromCaliper,
}

#[derive(Clone, Copy, ValueEnum)]
enum FlipArg {
    LongEdge,
    ShortEdge,
}

impl From<FlipArg> for pdf_impose::FlipAxis {
    fn from(arg: FlipArg) -> Self {
        match arg {
            FlipArg::LongEdge => Self::LongEdge,
            FlipArg::ShortEdge => Self::ShortEdge,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum BindingArg {
    Signature,
    Perfect,
    SideStitch,
    Spiral,
    Case,
}

#[derive(Clone, Copy, ValueEnum)]
enum ArrangementArg {
    Folio,
    Quarto,
    Octavo,
}

#[derive(Clone, Copy, ValueEnum)]
enum PaperArg {
    A3,
    A4,
    A5,
    B4,
    B5,
    Letter,
    Legal,
    Tabloid,
}

#[derive(Clone, Copy, ValueEnum)]
enum OrientationArg {
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, ValueEnum)]
enum FormatArg {
    DoubleSided,
    TwoSided,
    SingleSided,
}

#[derive(Clone, Copy, ValueEnum)]
enum ScalingArg {
    Fit,
    Fill,
    None,
    Stretch,
}

impl From<BindingArg> for pdf_impose::BindingType {
    fn from(arg: BindingArg) -> Self {
        match arg {
            BindingArg::Signature => Self::Signature,
            BindingArg::Perfect => Self::PerfectBinding,
            BindingArg::SideStitch => Self::SideStitch,
            BindingArg::Spiral => Self::Spiral,
            BindingArg::Case => Self::CaseBinding,
        }
    }
}

impl From<ArrangementArg> for pdf_impose::PageArrangement {
    fn from(arg: ArrangementArg) -> Self {
        match arg {
            ArrangementArg::Folio => Self::Folio,
            ArrangementArg::Quarto => Self::Quarto,
            ArrangementArg::Octavo => Self::Octavo,
        }
    }
}

impl From<PaperArg> for pdf_impose::PaperSize {
    fn from(arg: PaperArg) -> Self {
        match arg {
            PaperArg::A3 => Self::A3,
            PaperArg::A4 => Self::A4,
            PaperArg::A5 => Self::A5,
            PaperArg::B4 => Self::B4,
            PaperArg::B5 => Self::B5,
            PaperArg::Letter => Self::Letter,
            PaperArg::Legal => Self::Legal,
            PaperArg::Tabloid => Self::Tabloid,
        }
    }
}

impl From<OrientationArg> for pdf_impose::Orientation {
    fn from(arg: OrientationArg) -> Self {
        match arg {
            OrientationArg::Portrait => Self::Portrait,
            OrientationArg::Landscape => Self::Landscape,
        }
    }
}

impl From<FormatArg> for pdf_impose::OutputFormat {
    fn from(arg: FormatArg) -> Self {
        match arg {
            FormatArg::DoubleSided => Self::DoubleSided,
            FormatArg::TwoSided => Self::TwoSided,
            FormatArg::SingleSided => Self::SingleSidedSequence,
        }
    }
}

impl From<ScalingArg> for pdf_impose::ScalingMode {
    fn from(arg: ScalingArg) -> Self {
        match arg {
            ScalingArg::Fit => Self::Fit,
            ScalingArg::Fill => Self::Fill,
            ScalingArg::None => Self::None,
            ScalingArg::Stretch => Self::Stretch,
        }
    }
}

fn non_negative_f32(s: &str) -> std::result::Result<f32, String> {
    let v: f32 = s.parse().map_err(|e| format!("{e}"))?;
    if v < 0.0 {
        Err("value must not be negative".into())
    } else {
        Ok(v)
    }
}

fn positive_f32(s: &str) -> std::result::Result<f32, String> {
    let v: f32 = s.parse().map_err(|e| format!("{e}"))?;
    if v <= 0.0 {
        Err("value must be positive".into())
    } else {
        Ok(v)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(if cli.verbose {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Warn
        })
        .format_timestamp(None)
        .format_target(cli.verbose)
        .init();

    match cli.command {
        Commands::Flashcards {
            input,
            output,
            rows,
            columns,
            card_width_in,
            card_height_in,
        } => {
            // The CLI uses the default column mapping (first column → front,
            // second → back, rest ignored) on a header-bearing CSV; the GUI
            // exposes per-column control.
            let (table, csv_warnings) = pdf_flashcards::load_table_from_csv(&input, true).await?;
            for w in &csv_warnings {
                eprintln!("Warning: {w}");
            }
            let cards = table.to_cards(&table.default_roles(), "\n");

            if cards.is_empty() {
                eprintln!("No flashcards to generate");
                return Ok(());
            }

            let options = pdf_flashcards::FlashcardOptions {
                rows: rows as usize,
                columns: columns as usize,
                card_width_mm: card_width_in * 25.4,
                card_height_mm: card_height_in * 25.4,
                ..Default::default()
            };
            options.validate()?;
            pdf_flashcards::generate_pdf(&cards, &options, &output).await?;
            println!(
                "Generated {} flashcards → {}",
                cards.len(),
                output.display()
            );
        }

        Commands::Import {
            source,
            output,
            paper,
            margin_mm,
        } => {
            let imported = pdf_import::fetch(&source)?;
            let mut config = pdf_typeset::TypesetConfig {
                page_size: paper.into(),
                ..Default::default()
            };
            if let Some(m) = margin_mm {
                config.margin_top_mm = m;
                config.margin_bottom_mm = m;
                config.margin_inner_mm = m;
                config.margin_outer_mm = m;
            }
            let doc = match imported.kind {
                pdf_typeset::InputFormat::Markdown => pdf_typeset::import_markdown(
                    &imported.payload,
                    &imported,
                    config.smart_punctuation,
                ),
                _ => pdf_typeset::import_html(&imported.payload, &imported),
            };
            // The template owns all front matter: the extracted title becomes the
            // title page, and the table of contents replaces the source's own.
            config.doc_title = doc.title.clone().unwrap_or_default();
            config.generate_toc = true;
            let pdf = pdf_typeset::compile_imported(&doc, &config)?;
            std::fs::write(&output, &pdf)?;
            println!(
                "Imported {} → {} ({} KB)",
                imported.source_url.as_deref().unwrap_or(source.as_str()),
                output.display(),
                pdf.len() / 1024
            );
            match imported.asset_report() {
                pdf_import::AssetReport {
                    archive: pdf_import::ArchiveStatus::Fetched { .. },
                    figures_upgraded,
                } => println!("{figures_upgraded} figure(s) upgraded to print resolution"),
                pdf_import::AssetReport {
                    archive: pdf_import::ArchiveStatus::Failed(e),
                    ..
                } => eprintln!("note: figures kept at web resolution (LaTeX source: {e})"),
                pdf_import::AssetReport {
                    archive: pdf_import::ArchiveStatus::Disabled,
                    ..
                } => eprintln!(
                    "note: build with --features hires-import for print-resolution figures"
                ),
                _ => {}
            }
        }

        Commands::Impose {
            input,
            output,
            binding,
            arrangement,
            sheets_per_signature,
            paper,
            orientation,
            format,
            scaling,
            front_flyleaves,
            back_flyleaves,
            split_by_signatures,
            fold_lines,
            trim_marks,
            crop_marks,
            registration_marks,
            sewing_marks,
            collation_marks,
            interior_marks_gray,
            interior_marks_line_scale,
            exterior_marks_gray,
            exterior_marks_line_scale,
            sewing_stations,
            kettle_offset,
            sheet_margin,
            leaf_spine_margin,
            leaf_fore_edge_margin,
            leaf_top_margin,
            leaf_bottom_margin,
            trim_allowance,
            creep_mode,
            creep_per_layer,
            paper_thickness,
            stats_only,
            cascade_cols,
            cascade_rows,
            cascade_margin,
            cascade_cut_lines,
            cascade_flip,
        } => {
            let creep = match creep_mode {
                CreepModeArg::None => pdf_impose::CreepConfig::None,
                CreepModeArg::PerLayer => pdf_impose::CreepConfig::PerLayer {
                    creep_per_layer_mm: creep_per_layer,
                },
                CreepModeArg::FromCaliper => pdf_impose::CreepConfig::FromCaliper {
                    paper_thickness_mm: paper_thickness,
                },
            };

            let split_mode = match split_by_signatures {
                Some(n) => pdf_impose::SplitMode::BySignatures(n as usize),
                None => pdf_impose::SplitMode::None,
            };

            let cascade = if cascade_cols > 1 || cascade_rows > 1 {
                Some(pdf_impose::CascadeConfig {
                    cols: cascade_cols,
                    rows: cascade_rows,
                    margin_mm: cascade_margin,
                    cut_lines: cascade_cut_lines,
                    flip_axis: cascade_flip.into(),
                })
            } else {
                None
            };

            let options = pdf_impose::ImpositionOptions {
                input_files: input.clone(),
                binding_type: binding.into(),
                page_arrangement: arrangement.into(),
                sheets_per_signature,
                output_paper_size: paper.into(),
                output_orientation: orientation.into(),
                output_format: format.into(),
                scaling_mode: scaling.into(),
                front_flyleaves,
                back_flyleaves,
                margins: pdf_impose::Margins {
                    sheet: pdf_impose::SheetMargins::uniform(sheet_margin),
                    leaf: pdf_impose::LeafMargins {
                        top_mm: leaf_top_margin,
                        bottom_mm: leaf_bottom_margin,
                        fore_edge_mm: leaf_fore_edge_margin,
                        spine_mm: leaf_spine_margin,
                        trim_allowance_mm: trim_allowance,
                    },
                },
                marks: pdf_impose::PrinterMarks {
                    fold_lines,
                    trim_marks,
                    crop_marks,
                    registration_marks,
                    sewing_marks,
                    collation_marks,
                },
                interior_marks_appearance: pdf_impose::MarksAppearance {
                    gray: interior_marks_gray,
                    line_width_scale: interior_marks_line_scale,
                },
                exterior_marks_appearance: pdf_impose::MarksAppearance {
                    gray: exterior_marks_gray,
                    line_width_scale: exterior_marks_line_scale,
                },
                sewing_config: pdf_impose::SewingConfig {
                    station_count: sewing_stations,
                    kettle_offset_mm: kettle_offset,
                },
                cascade,
                creep,
                split_mode,
                ..Default::default()
            };

            // Load all input PDFs
            let documents = pdf_impose::load_multiple_pdfs(&input).await?;

            // Calculate and show statistics
            let stats = pdf_impose::calculate_statistics(&documents, &options)?;
            println!("Imposition Statistics:");
            println!("  Source pages: {}", stats.source_pages);
            println!("  Output sheets: {}", stats.output_sheets);
            println!("  Output pages: {}", stats.output_pages);
            if stats.blank_pages_added > 0 {
                println!("  Blank pages added: {}", stats.blank_pages_added);
            }
            if let Some(sigs) = stats.signatures {
                println!("  Signatures: {sigs}");
            }
            if let Some(cells) = stats.cascade_cells_per_sheet {
                println!("  Cascade cells per sheet: {cells}");
            }
            for warning in &stats.warnings {
                eprintln!("  Warning: {warning}");
            }

            if stats_only {
                return Ok(());
            }

            // Perform imposition (handles split_mode internally)
            let saved_paths = pdf_impose::impose_and_save(documents, &options, &output).await?;
            match saved_paths.len() {
                0 => unreachable!("impose_and_save always returns at least one path"),
                1 => println!("Imposed → {}", saved_paths[0].display()),
                n => {
                    println!("Imposed → {n} files:");
                    for p in &saved_paths {
                        println!("  {}", p.display());
                    }
                }
            }
        }
    }

    Ok(())
}
