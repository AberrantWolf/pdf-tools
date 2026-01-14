use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pdft", about = "PDF tools CLI", version)]
struct Cli {
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
        #[arg(long, default_value = "2")]
        rows: usize,

        /// Columns per page
        #[arg(long, default_value = "3")]
        columns: usize,

        /// Card width in inches
        #[arg(long, default_value = "2.5")]
        card_width_in: f32,

        /// Card height in inches
        #[arg(long, default_value = "3.5")]
        card_height_in: f32,
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

        /// Page arrangement (pages per signature)
        #[arg(long, default_value = "folio", value_enum)]
        arrangement: ArrangementArg,

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

        /// Add fold lines
        #[arg(long)]
        fold_lines: bool,

        /// Add cut lines
        #[arg(long)]
        cut_lines: bool,

        /// Add crop marks (at sheet edges)
        #[arg(long)]
        crop_marks: bool,

        /// Add trim marks (at each leaf boundary)
        #[arg(long)]
        trim_marks: bool,

        /// Add registration marks
        #[arg(long)]
        registration_marks: bool,

        /// Sheet margin in mm (uniform on all sides)
        #[arg(long, default_value = "5.0")]
        sheet_margin: f32,

        /// Show statistics only, don't generate PDF
        #[arg(long)]
        stats_only: bool,
    },
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Flashcards {
            input,
            output,
            rows,
            columns,
            card_width_in,
            card_height_in,
        } => {
            let cards = pdf_flashcards::load_from_csv(&input).await?;
            let options = pdf_flashcards::FlashcardOptions {
                rows,
                columns,
                card_width_mm: card_width_in * 25.4,
                card_height_mm: card_height_in * 25.4,
                ..Default::default()
            };
            pdf_flashcards::generate_pdf(&cards, &options, &output).await?;
            println!(
                "Generated {} flashcards → {}",
                cards.len(),
                output.display()
            );
        }

        Commands::Impose {
            input,
            output,
            binding,
            arrangement,
            paper,
            orientation,
            format,
            scaling,
            front_flyleaves,
            back_flyleaves,
            fold_lines,
            cut_lines,
            crop_marks,
            trim_marks,
            registration_marks,
            sheet_margin,
            stats_only,
        } => {
            let options = pdf_impose::ImpositionOptions {
                input_files: input.clone(),
                binding_type: binding.into(),
                page_arrangement: arrangement.into(),
                output_paper_size: paper.into(),
                output_orientation: orientation.into(),
                output_format: format.into(),
                scaling_mode: scaling.into(),
                front_flyleaves,
                back_flyleaves,
                margins: pdf_impose::Margins {
                    sheet: pdf_impose::SheetMargins::uniform(sheet_margin),
                    ..Default::default()
                },
                marks: pdf_impose::PrinterMarks {
                    fold_lines,
                    cut_lines,
                    crop_marks,
                    trim_marks,
                    registration_marks,
                },
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
            println!("  Blank pages added: {}", stats.blank_pages_added);
            if let Some(sigs) = stats.signatures {
                println!("  Signatures: {}", sigs);
            }

            if stats_only {
                return Ok(());
            }

            // Perform imposition
            let imposed = pdf_impose::impose(&documents, &options).await?;
            pdf_impose::save_pdf(imposed, &output).await?;
            println!("Imposed → {}", output.display());
        }
    }

    Ok(())
}
