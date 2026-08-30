//! `kb` -- the knowledge base tool.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kb", version, about = "The knowledge base tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Work with the schema itself.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
}

#[derive(Subcommand)]
enum SchemaCommand {
    /// Render the schema as markdown.
    Render {
        /// The schema data file.
        #[arg(long, default_value = "schemas/note.toml")]
        schema: PathBuf,
        /// Where to write. Defaults to stdout.
        #[arg(long, short)]
        out: Option<PathBuf>,
        /// Exit non-zero if `--out` is already up to date -- for CI, so a schema
        /// edit that forgets to re-render is caught rather than merged.
        #[arg(long)]
        check: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Schema { command } => match command {
            SchemaCommand::Render { schema, out, check } => render(&schema, out, check),
        },
    }
}

fn render(schema_path: &std::path::Path, out: Option<PathBuf>, check: bool) -> Result<()> {
    let schema = lab_schema::Schema::load(schema_path)?;
    let markdown = lab_schema::render_markdown(&schema);

    let Some(out) = out else {
        print!("{markdown}");
        return Ok(());
    };

    if check {
        let current = std::fs::read_to_string(&out).unwrap_or_default();
        if current == markdown {
            return Ok(());
        }
        anyhow::bail!(
            "{} is stale -- rerun without --check to regenerate",
            out.display()
        );
    }

    std::fs::write(&out, &markdown).with_context(|| format!("writing {}", out.display()))?;
    Ok(())
}
