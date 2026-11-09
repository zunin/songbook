use anyhow::{bail, Context, Result};
use songbook_core::audit::{apply_noninteractive_defaults, audit_summary_line};
use songbook_core::book::{render_book, BookManifest};
use songbook_core::parser::classify::parse;
use songbook_core::parser::song::Song;
use songbook_core::transpose::transpose_song;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct BookSummary {
    pub included: usize,
    pub skipped: usize,
}

/// Compiles the book described by `manifest` into one standalone LaTeX
/// document. Per-song errors are reported to stderr and skipped, never fatal;
/// manifest errors (unknown header, bad extension, missing song file) are
/// fatal, since they describe the book itself. The title index is always
/// included; `chord_index` opts into the chord index. Book mode is inherently
/// non-interactive: each included song gets a one-line audit summary, like
/// batch mode. `semitones` transposes every included song (negative = down;
/// 0 leaves songs untouched).
pub fn run(
    manifest_path: &Path,
    out: Option<&Path>,
    chord_index: bool,
    semitones: i32,
) -> Result<BookSummary> {
    let manifest = BookManifest::load(manifest_path)?;
    let out_path = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_book_out_path(manifest_path));

    let mut songs = Vec::new();
    let mut included = 0;
    let mut skipped = 0;
    for entry in &manifest.songs {
        match convert_song(&entry.path, semitones) {
            Ok((song, summary)) => {
                println!("Included {} — {summary}", entry.path.display());
                songs.push(song);
                included += 1;
            }
            Err(error) => {
                eprintln!("Skipped {}: {error:#}", entry.path.display());
                skipped += 1;
            }
        }
    }
    if songs.is_empty() {
        bail!("all {skipped} manifest songs were skipped; no book written");
    }

    let tex = render_book(&manifest, &songs, chord_index).context("rendering book")?;
    std::fs::write(&out_path, tex)
        .with_context(|| format!("writing output {}", out_path.display()))?;
    println!("Wrote {}", out_path.display());

    Ok(BookSummary { included, skipped })
}

/// Reads, parses, and audits one song, applying the non-interactive defaults
/// so the book matches `--yes`/batch output, then transposing it.
fn convert_song(path: &Path, semitones: i32) -> Result<(Song, String)> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    if raw.trim().is_empty() {
        bail!("file is empty");
    }
    let song = parse(&raw);
    if song.sections.is_empty() {
        bail!("no song sections found (input may be only noise or metadata)");
    }
    let mut audited_song = song.clone();
    transpose_song(&mut audited_song, semitones);
    let summary = audit_summary_line(&audited_song, &raw);
    let mut song = song;
    apply_noninteractive_defaults(&mut song);
    transpose_song(&mut song, semitones);
    songbook_core::tablature::validate(&song)
        .with_context(|| format!("validating notation in {}", path.display()))?;
    Ok((song, summary))
}

/// The manifest's stem plus `.tex`, beside the manifest.
fn default_book_out_path(manifest: &Path) -> PathBuf {
    let stem = manifest
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "book".to_string());
    manifest
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{stem}.tex"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const GOOD: &str = "Song: Good Song\nArtist: Someone\n\n[Verse]\n[Am]Hello there\n";

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("songbook-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_manifest(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("album.book");
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn book_tabs_warn_at_active_offset_and_unsupported_songs_are_skipped() {
        let dir = tempdir("booktabs");
        fs::write(dir.join("good.txt"), "e|--0--12--|\n".repeat(6)).unwrap();
        fs::write(dir.join("bad.txt"), "e|--0--|\n".repeat(4)).unwrap();
        let (_, summary) = convert_song(&dir.join("good.txt"), 2).unwrap();
        assert!(summary.contains(songbook_core::tablature::UNCHANGED_WARNING));
        let manifest = write_manifest(&dir, "good.txt\nbad.txt\n");
        let summary = run(&manifest, None, false, 0).unwrap();
        assert_eq!((summary.included, summary.skipped), (1, 1));
        let tex = fs::read_to_string(dir.join("album.tex")).unwrap();
        assert!(tex.contains("\\songcolumns{1}"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn book_writes_all_songs_into_one_standalone_tex() {
        let dir = tempdir("book");
        fs::write(dir.join("one.txt"), GOOD).unwrap();
        fs::write(dir.join("two.crd"), "Song: Second\n[Chorus]\n[F]Hey\n").unwrap();
        let manifest = write_manifest(&dir, "one.txt\ntwo.crd\n");

        let summary = run(&manifest, None, false, 0).unwrap();
        assert_eq!(summary.included, 2);
        assert_eq!(summary.skipped, 0);

        // Default output: the manifest's stem + .tex, beside the manifest.
        let tex = fs::read_to_string(dir.join("album.tex")).unwrap();
        assert!(tex.starts_with("\\documentclass"), "{tex}");
        assert_eq!(tex.matches("\\beginsong{").count(), 2, "{tex}");
        assert!(tex.contains("\\beginsong{Good Song}"));
        assert!(tex.contains("\\beginsong{Second}"));
        assert!(tex.contains("\\showindex{Index of Songs}{titleidx}"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn book_skips_invalid_songs_but_writes_survivors() {
        let dir = tempdir("bookskip");
        fs::write(dir.join("good.txt"), GOOD).unwrap();
        fs::write(dir.join("empty.txt"), "").unwrap();
        fs::write(dir.join("noise.txt"), "Tabbed by: x\n").unwrap();
        let manifest = write_manifest(&dir, "good.txt\nempty.txt\nnoise.txt\n");

        let summary = run(&manifest, None, false, 0).unwrap();
        assert_eq!(summary.included, 1);
        assert_eq!(summary.skipped, 2);

        let tex = fs::read_to_string(dir.join("album.tex")).unwrap();
        assert_eq!(tex.matches("\\beginsong{").count(), 1, "{tex}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn book_errors_when_every_song_is_skipped() {
        let dir = tempdir("bookempty");
        fs::write(dir.join("empty.txt"), "").unwrap();
        let manifest = write_manifest(&dir, "empty.txt\n");

        let error = run(&manifest, None, false, 0).unwrap_err();
        assert!(format!("{error:#}").contains("all 1 manifest songs were skipped"));

        assert!(!dir.join("album.tex").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn book_out_flag_names_the_output() {
        let dir = tempdir("bookout");
        fs::write(dir.join("one.txt"), GOOD).unwrap();
        let manifest = write_manifest(&dir, "one.txt\n");
        let out = dir.join("elsewhere.tex");

        run(&manifest, Some(&out), false, 0).unwrap();
        assert!(out.exists());
        assert!(!dir.join("album.tex").exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn book_chord_index_opt_in_reaches_the_tex() {
        let dir = tempdir("bookchords");
        fs::write(dir.join("one.txt"), "[Verse]\n[Am]Hello there\n").unwrap();
        let manifest = write_manifest(&dir, "one.txt\n");

        run(&manifest, None, true, 0).unwrap();
        let tex = fs::read_to_string(dir.join("album.tex")).unwrap();
        assert!(tex.contains("\\newindex{chordidx}{chordidx}"), "{tex}");
        assert!(tex.contains("\\indexentry[chordidx]{Am}%"), "{tex}");
        assert!(tex.contains("\\showindex{Index of Chords}{chordidx}"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn book_transposes_every_included_song_and_its_chord_index() {
        let dir = tempdir("booktranspose");
        fs::write(dir.join("one.txt"), "[Verse]\n[Am]Hello there\n").unwrap();
        fs::write(dir.join("two.crd"), "Song: Second\n[Chorus]\n[F]Hey\n").unwrap();
        let manifest = write_manifest(&dir, "one.txt\ntwo.crd\n");

        run(&manifest, None, true, 2).unwrap();
        let tex = fs::read_to_string(dir.join("album.tex")).unwrap();
        assert!(tex.contains("\\[Bm]Hello there"), "{tex}");
        assert!(tex.contains("\\[G]Hey"), "{tex}");
        assert!(!tex.contains("\\[Am]"), "{tex}");
        // The chord index is derived from the transposed song.
        assert!(tex.contains("\\indexentry[chordidx]{Bm}%"), "{tex}");
        assert!(tex.contains("\\indexentry[chordidx]{G}%"), "{tex}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn default_book_out_path_sits_beside_the_manifest() {
        assert_eq!(
            default_book_out_path(Path::new("songs/album.book")),
            PathBuf::from("songs/album.tex")
        );
        assert_eq!(
            default_book_out_path(Path::new("album.book")),
            PathBuf::from("album.tex")
        );
    }
}
