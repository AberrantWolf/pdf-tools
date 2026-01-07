use anyhow::Result;
use clap::{Parser, Subcommand};
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

        /// Cards per page
        #[arg(long, default_value = "4")]
        cards_per_page: usize,
    },

    /// Impose PDF pages
    Impose {
        /// Input PDF file
        #[arg(short, long)]
        input: PathBuf,

        /// Output PDF file
        #[arg(short, long)]
        output: PathBuf,

        /// Layout type: 2up, 4up, booklet
        #[arg(long, default_value = "2up")]
        layout: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Flashcards {
            input,
            output,
            cards_per_page,
        } => {
            let cards = pdf_flashcards::load_from_csv(&input).await?;
            let options = pdf_flashcards::FlashcardOptions {
                cards_per_page,
                ..Default::default()
            };
            pdf_flashcards::generate_pdf(&cards, &options, &output).await?;
            println!("Generated {} flashcards → {}", cards.len(), output.display());
        }

        Commands::Impose {
            input,
            output,
            layout,
        } => {
            let doc = pdf_impose::load_pdf(&input).await?;
            let layout = match layout.as_str() {
                "2up" => pdf_impose::ImpositionLayout::TwoUp,
                "4up" => pdf_impose::ImpositionLayout::FourUp,
                "booklet" => pdf_impose::ImpositionLayout::Booklet,
                _ => anyhow::bail!("Unknown layout: {layout}"),
            };
            let options = pdf_impose::ImpositionOptions {
                layout,
                ..Default::default()
            };
            let imposed = pdf_impose::impose(&doc, &options).await?;
            pdf_impose::save_pdf(imposed, &output).await?;
            println!("Imposed → {}", output.display());
        }
    }

    Ok(())
}
