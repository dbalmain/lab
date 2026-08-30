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
    /// Check every note against the schema.
    Lint {
        /// The source to check.
        #[arg(default_value = ".")]
        root: PathBuf,
        /// The schema data file.
        #[arg(long, default_value = "schemas/note.toml")]
        schema: PathBuf,
    },
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
        Command::Lint { root, schema } => lint(&schema, &root),
        Command::Schema { command } => match command {
            SchemaCommand::Render { schema, out, check } => render(&schema, out, check),
        },
    }
}

fn lint(schema_path: &std::path::Path, root: &std::path::Path) -> Result<()> {
    let schema = lab_schema::Schema::load(schema_path)?;
    let today = jiff::Zoned::now().date();
    let report = lab_lint::check(&schema, root, today)?;

    for diagnostic in &report.diagnostics {
        println!("{}", diagnostic.render(root));
    }

    // A rule that did not run is reported every time. A check nobody ran looks
    // exactly like a check that passed, and the difference matters most on the
    // day someone relies on it.
    if !report.not_run.is_empty() {
        println!(
            "\nnot run (needs the kb-priv link index): {}",
            report.not_run.join(", ")
        );
    }

    println!(
        "\n{} notes checked, {} errors, {} warnings",
        report.notes_checked,
        report.errors(),
        report.warnings()
    );

    if report.errors() > 0 {
        std::process::exit(1);
    }
    Ok(())
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
