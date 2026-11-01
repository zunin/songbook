mod batch;
mod book;
mod tui;

use anyhow::{bail, Context, Result};
use clap::Parser as ClapParser;
use songbook_core::{audit, latex, parser, transpose};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

#[derive(ClapParser, Debug)]
#[command(
    name = "songbook",
    version,
    about = "Convert ASCII tabs (chord-over-lyrics) to LaTeX songs-package format, one song or a whole book"
)]
struct Cli {
    /// Input .txt/.crd file, or a directory whose .txt/.crd files are all
    /// converted (required unless --book is given)
    input: Option<PathBuf>,

    /// Build one LaTeX document from a book manifest: a plain-text list of
    /// .txt/.crd paths (one per line, # comments allowed), optionally
    /// preceded by Title:/Author: headers
    #[arg(long)]
    book: Option<PathBuf>,

    /// Output .tex path (file input), output directory (directory input), or
    /// output .tex for --book (default: the manifest's stem + .tex beside it)
    #[arg(short, long)]
    out: Option<PathBuf>,

    /// With --book, also emit the chord index (the title index is always
    /// included)
    #[arg(long)]
    chord_index: bool,

    /// With a directory input, also convert files in subdirectories
    #[arg(long)]
    recursive: bool,

    /// Emit a standalone compilable LaTeX document (implied for --book)
    #[arg(long)]
    standalone: bool,

    /// Skip the TUI review, trust the heuristics, and print an audit of every
    /// guess (no-op for --book, which is always non-interactive)
    #[arg(short = 'y', long)]
    yes: bool,

    /// Shift every chord by this many semitones (negative = down, e.g.
    /// --transpose=-2); with a `Key:` header, names are respelled in that key
    /// (F+5=Bb under Key: C) and the header itself follows (Key: C +2 becomes
    /// Key: D); without one, a fixed sharp ladder applies upward and a flat
    /// ladder downward (C+1=C#, Bb-1=A)
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    transpose: i32,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(manifest) = &cli.book {
        if cli.input.is_some() {
            bail!("--book cannot be combined with a positional input file");
        }
        if cli.recursive {
            bail!("--recursive has no effect with --book (a book lists its songs explicitly)");
        }
        let summary = book::run(manifest, cli.out.as_deref(), cli.chord_index, cli.transpose)?;
        println!(
            "Included {}, skipped {}.",
            summary.included, summary.skipped
        );
        if summary.skipped > 0 {
            std::process::exit(1);
        }
        return Ok(());
    }

    let Some(input) = cli.input.as_deref() else {
        bail!("expected an input .txt/.crd file or directory, or --book <manifest>");
    };

    if input.is_dir() {
        let summary = batch::run(
            input,
            cli.out.as_deref(),
            cli.recursive,
            cli.standalone,
            cli.transpose,
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

    run_single(input, &cli)
}

fn run_single(input: &Path, cli: &Cli) -> Result<()> {
    let raw = std::fs::read_to_string(input)
        .with_context(|| format!("reading input {}", input.display()))?;
    if raw.trim().is_empty() {
        anyhow::bail!("input file {} is empty", input.display());
    }
    let out_path = cli.out.clone().unwrap_or_else(|| default_out_path(input));

    let song = parser::classify::parse(&raw);
    if song.sections.is_empty() {
        anyhow::bail!(
            "no song sections found in {}; the input may be only noise or metadata",
            input.display()
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

    // The audit above describes the parse guesses, which transposition does
    // not touch; the transposed song is what gets rendered below.
    transpose::transpose_song(&mut song, cli.transpose);

    let tex = latex::render::render(&song, cli.standalone)
        .with_context(|| format!("rendering {}", input.display()))?;
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
