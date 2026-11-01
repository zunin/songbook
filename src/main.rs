mod batch;
mod tui;

use anyhow::{Context, Result};
use clap::Parser as ClapParser;
use songbook_core::{audit, latex, parser};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

#[derive(ClapParser, Debug)]
#[command(
    name = "songbook",
    version,
    about = "Convert ASCII tabs (chord-over-lyrics) to LaTeX songs-package format"
)]
struct Cli {
    /// Input .txt/.crd file, or a directory whose .txt/.crd files are all converted
    input: PathBuf,

    /// Output .tex path (file input) or output directory (directory input)
    #[arg(short, long)]
    out: Option<PathBuf>,

    /// With a directory input, also convert files in subdirectories
    #[arg(long)]
    recursive: bool,

    /// Emit a standalone compilable LaTeX document
    #[arg(long)]
    standalone: bool,

    /// Skip the TUI review, trust the heuristics, and print an audit of every guess
    #[arg(short = 'y', long)]
    yes: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.input.is_dir() {
        let summary = batch::run(
            &cli.input,
            cli.out.as_deref(),
            cli.recursive,
            cli.standalone,
        )?;
        println!(
            "Converted {}, skipped {}.",
            summary.converted, summary.skipped
        );
        if summary.skipped > 0 {
            std::process::exit(1);
        }
        return Ok(());
    }

    run_single(&cli)
}

fn run_single(cli: &Cli) -> Result<()> {
    let raw = std::fs::read_to_string(&cli.input)
        .with_context(|| format!("reading input {}", cli.input.display()))?;
    if raw.trim().is_empty() {
        anyhow::bail!("input file {} is empty", cli.input.display());
    }
    let out_path = cli
        .out
        .clone()
        .unwrap_or_else(|| default_out_path(&cli.input));

    let song = parser::classify::parse(&raw);
    if song.sections.is_empty() {
        anyhow::bail!(
            "no song sections found in {}; the input may be only noise or metadata",
            cli.input.display()
        );
    }

    let triage_ran = !cli.yes && std::io::stdin().is_terminal();
    let song = if cli.yes {
        Some(song)
    } else {
        match tui::run(song, &raw, &out_path).context("interactive triage failed")? {
            tui::Outcome::Saved(song) => Some(song),
            tui::Outcome::Quit => None,
        }
    };
    let Some(mut song) = song else {
        println!("Triage cancelled; no output written.");
        return Ok(());
    };

    if !triage_ran {
        let report = audit::audit_report(&song, &raw);
        audit::apply_noninteractive_defaults(&mut song);
        print!("{report}");
    }

    let tex = latex::render::render(&song, cli.standalone)
        .with_context(|| format!("rendering {}", cli.input.display()))?;
    std::fs::write(&out_path, tex)
        .with_context(|| format!("writing output {}", out_path.display()))?;
    println!("Wrote {}", out_path.display());

    Ok(())
}

fn default_out_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "song".to_string());
    PathBuf::from(format!("{stem}.tex"))
}
