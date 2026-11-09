//! Book compilation (ADR0018): one LaTeX document composing many songs.
//!
//! A book is a manifest (plain text, version 1) plus the songs it lists. The
//! document reuses the single-song renderer's `\beginsong…\endsong` bodies
//! unchanged and only adds document-level structure: package setup, an
//! optional title block, the `songs` environment, and the package's indexes.
//!
//! Composition is also exposed over pre-rendered parts (`compose_book`,
//! ADR0019): the web triages songs one at a time in wasm `Session`s and hands
//! the rendered bodies plus their chord lists back to core, so book assembly
//! has exactly one implementation and never drifts into JavaScript.
//!
//! Index structure follows the pinned `songs` package documentation (v3.1,
//! `web/dist/pdf-assets/sources/songs.zip`), which requires three coordinated
//! pieces for an index to be non-empty:
//!
//! 1. `\newindex{id}{filename}` in the preamble (it is `\@onlypreamble`),
//! 2. the index id listed in the `songs` environment argument
//!    (`\begin{songs}{titleidx}`) — this is what registers every song in the
//!    index; an empty argument silently produces an empty index,
//! 3. `\showindex[columns]{title}{id}` outside the `songs` environment, where
//!    the index should be typeset (indexes are their own pages by default).
//!
//! The package has no built-in chord index (only title, author, and scripture
//! indexes), so the opt-in chord index is built from the documented
//! `\indexentry[<indexes>]{<text>}` macro: one entry per distinct chord per
//! song, targeted at the chord index only.

use crate::latex::render::{escape, render};
use crate::parser::chord::{chord_tokens, inline_chords, is_chord};
use crate::parser::song::{LineKind, Song};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// One song path listed in a book manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookEntry {
    /// The path as written in the manifest, resolved against the manifest's
    /// directory (absolute paths are kept as-is).
    pub path: PathBuf,
    /// One-based manifest line the path was read from, for error messages.
    pub line: usize,
}

/// A parsed book manifest (v1 plain-text format).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookManifest {
    /// Book title from a `Title:` header, if present.
    pub title: Option<String>,
    /// Book author from an `Author:` header, if present.
    pub author: Option<String>,
    /// Song paths in manifest order; no discovery, no sorting.
    pub songs: Vec<BookEntry>,
}

impl BookManifest {
    /// Parses manifest text. Paths resolve relative to `base`, which should be
    /// the manifest's directory.
    ///
    /// Format: `#` comments and blank lines are ignored; before the first
    /// path, `Title: …` and `Author: …` headers (case-insensitive keys) are
    /// accepted and any other `Key: value` line is an error; every remaining
    /// non-comment, non-blank line is one `.txt`/`.crd` path.
    pub fn parse(text: &str, base: &Path) -> Result<BookManifest> {
        let mut manifest = BookManifest::default();
        for (index, raw_line) in text.lines().enumerate() {
            let line = index + 1;
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if manifest.songs.is_empty() {
                if let Some((key, value)) = trimmed.split_once(':') {
                    let key = key.trim().to_ascii_lowercase();
                    let value = value.trim().to_string();
                    match key.as_str() {
                        "title" => set_header(&mut manifest.title, "Title", &value, line)?,
                        "author" => set_header(&mut manifest.author, "Author", &value, line)?,
                        _ => bail!(
                            "manifest line {line}: unknown header \"{key}:\" (expected \
                             Title:, Author:, or the first song path)"
                        ),
                    }
                    continue;
                }
            }
            if !is_tab_path(trimmed) {
                bail!("manifest line {line}: not a .txt/.crd file: {trimmed}");
            }
            manifest.songs.push(BookEntry {
                path: base.join(trimmed),
                line,
            });
        }
        if manifest.songs.is_empty() {
            bail!("manifest lists no songs (expected at least one .txt/.crd path)");
        }
        Ok(manifest)
    }

    /// Reads and parses the manifest at `path`, then verifies that every
    /// listed song file exists (missing files are manifest errors, reported
    /// with the manifest line that listed them).
    pub fn load(path: &Path) -> Result<BookManifest> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading manifest {}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new(""));
        let manifest =
            Self::parse(&text, base).with_context(|| format!("in manifest {}", path.display()))?;
        for entry in &manifest.songs {
            if !entry.path.exists() {
                bail!(
                    "manifest line {}: song file not found: {}",
                    entry.line,
                    entry.path.display()
                );
            }
            if !entry.path.is_file() {
                bail!(
                    "manifest line {}: not a regular file: {}",
                    entry.line,
                    entry.path.display()
                );
            }
        }
        Ok(manifest)
    }
}

fn set_header(slot: &mut Option<String>, key: &str, value: &str, line: usize) -> Result<()> {
    if slot.is_some() {
        bail!("manifest line {line}: duplicate {key}: header");
    }
    *slot = Some(value.to_string());
    Ok(())
}

/// Mirrors the CLI's tab-file extension check: `.txt`/`.crd`,
/// case-insensitive.
fn is_tab_path(path: &str) -> bool {
    matches!(
        Path::new(path).extension().and_then(|extension| extension.to_str()),
        Some(extension)
            if extension.eq_ignore_ascii_case("txt") || extension.eq_ignore_ascii_case("crd")
    )
}

/// Renders a complete standalone LaTeX document for the book: the package
/// preamble with the index declarations, an optional title block, the `songs`
/// environment holding each song's usual body in manifest order, and the
/// index displays.
///
/// `songs` are the manifest's songs that converted successfully, in manifest
/// order; songs the caller skipped simply do not appear in the book. The
/// title index is always included; `chord_index` adds the chord index.
pub fn render_book(manifest: &BookManifest, songs: &[Song], chord_index: bool) -> Result<String> {
    let mut parts = Vec::with_capacity(songs.len());
    for song in songs {
        parts.push(BookPart {
            body: render(song, false)?,
            chords: distinct_chords(song),
        });
    }
    compose_book(
        manifest.title.as_deref(),
        manifest.author.as_deref(),
        &parts,
        chord_index,
    )
}

/// One triaged song's contribution to a book (ADR0019): the non-standalone
/// `\beginsong…\endsong` body plus the chord list the chord index needs. The
/// web produces one of these per song in its wasm `Session` and hands the
/// slice back to [`compose_book`], so book assembly stays in core and no
/// LaTeX is ever assembled in JavaScript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookPart {
    /// The song's body exactly as the single-song renderer produces it
    /// (`render(song, false)` — the same string the wasm `export_tex(false)`
    /// returns).
    pub body: String,
    /// The song's distinct chords in first-appearance order, as
    /// [`distinct_chords`] derives them.
    pub chords: Vec<String>,
}

/// Composes pre-rendered song parts into the complete standalone LaTeX book
/// document: the package preamble with the index declarations, an optional
/// title block, the `songs` environment holding each part's body in order,
/// and the index displays.
///
/// This is the one composition implementation (P1): [`render_book`] derives
/// parts from a manifest's songs and delegates here, and the wasm ferry hands
/// browser-triaged parts to the same code. Titles and authors are escaped like
/// song metadata, and empty ones count as absent. The title index is always
/// included; `chord_index` adds the chord index, injecting one `\indexentry`
/// per part chord right after the part's `\beginsong` line.
pub fn compose_book(
    title: Option<&str>,
    author: Option<&str>,
    parts: &[BookPart],
    chord_index: bool,
) -> Result<String> {
    let mut doc = String::new();
    doc.push_str("\\documentclass[12pt]{article}\n");
    doc.push_str("\\usepackage[chorded]{songs}\n\n");

    doc.push_str("\\newindex{titleidx}{titleidx}\n");
    if chord_index {
        doc.push_str("\\newindex{chordidx}{chordidx}\n");
    }
    // The parts carry plain text; escaping happens here at render time,
    // with the same helper the song renderer uses for titles and artists.
    let title = title.filter(|title| !title.is_empty());
    let author = author.filter(|author| !author.is_empty());
    if let Some(title) = title {
        doc.push_str(&format!("\\title{{{}}}\n", escape(title)));
    }
    if let Some(author) = author {
        doc.push_str(&format!("\\author{{{}}}\n", escape(author)));
    }
    if title.is_some() || author.is_some() {
        // \maketitle would print \today without \date{}; an empty date keeps
        // the output deterministic.
        doc.push_str("\\date{}\n");
    }

    doc.push_str("\n\\begin{document}\n");
    if title.is_some() || author.is_some() {
        doc.push_str("\\maketitle\n\\newpage\n");
    }
    doc.push_str("\\showindex{Index of Songs}{titleidx}\n");
    if parts
        .iter()
        .any(|part| part.body.contains(crate::tablature::LAYOUT_MARKER))
    {
        doc.push_str("\\songcolumns{1}\n");
    }
    doc.push_str("\\begin{songs}{titleidx}\n");
    for part in parts {
        if chord_index {
            push_chord_entries(&mut doc, part);
        } else {
            doc.push_str(&part.body);
        }
    }
    doc.push_str("\\end{songs}\n");
    if chord_index {
        doc.push_str("\\showindex{Index of Chords}{chordidx}\n");
    }
    doc.push_str("\\end{document}\n");
    Ok(doc)
}

/// Appends a pre-rendered body to the document, adding the book's chord-index
/// entries right after the `\beginsong` line. `\indexentry` is only effective
/// between `\beginsong` and `\endsong`, and writes only if the song is
/// shipped, which injection at composition time satisfies.
fn push_chord_entries(doc: &mut String, part: &BookPart) {
    if part.chords.is_empty() {
        doc.push_str(&part.body);
        return;
    }
    let header_end = part
        .body
        .find('\n')
        .map(|end| end + 1)
        .unwrap_or(part.body.len());
    doc.push_str(&part.body[..header_end]);
    for chord in &part.chords {
        doc.push_str(&format!("\\indexentry[chordidx]{{{}}}%\n", escape(chord)));
    }
    doc.push_str(&part.body[header_end..]);
}

/// The song's distinct chords in first-appearance order, using the same chord
/// helpers the renderer emits from: inline `[Chord]` markers for lyric lines
/// and bare tokens for chord-only lines. Public because the wasm ferry must
/// reuse this one chord derivation for the parts it composes.
pub fn distinct_chords(song: &Song) -> Vec<String> {
    let mut chords: Vec<String> = Vec::new();
    for section in &song.sections {
        for line in &section.lines {
            let text = match line {
                LineKind::Lyric(text) | LineKind::ChordOnly(text) => text,
                _ => continue,
            };
            let tokens = match line {
                LineKind::ChordOnly(_) => chord_tokens(text)
                    .into_iter()
                    .map(|token| token.text)
                    .collect::<Vec<_>>(),
                _ => inline_chords(text),
            };
            for token in tokens {
                if is_chord(&token) && !chords.iter().any(|chord| chord == &token) {
                    chords.push(token);
                }
            }
        }
    }
    chords
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::apply_noninteractive_defaults;
    use crate::parser::classify::parse;
    use crate::triage::{TriageState, Verdict};
    use std::fs;

    const GOOD: &str = "Song: Good Song\nArtist: Someone\n\n[Verse]\n[Am]Hello there\n";

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("songbook-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_headers_and_paths_in_order() {
        let dir = tempdir("parse");
        let text = "\
# comment line

Title: My Book
author: Someone

b-song.txt
a-song.crd
";
        let manifest = BookManifest::parse(text, &dir).unwrap();
        assert_eq!(manifest.title.as_deref(), Some("My Book"));
        assert_eq!(manifest.author.as_deref(), Some("Someone"));
        assert_eq!(
            manifest.songs,
            vec![
                BookEntry {
                    path: dir.join("b-song.txt"),
                    line: 6
                },
                BookEntry {
                    path: dir.join("a-song.crd"),
                    line: 7
                },
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_header_key_names_the_line() {
        let error =
            BookManifest::parse("Title: X\nTuning: EADGBE\nsong.txt", Path::new(".")).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("line 2"), "{message}");
        assert!(message.contains("unknown header"), "{message}");
    }

    #[test]
    fn duplicate_header_names_the_line() {
        let error =
            BookManifest::parse("Title: A\nTitle: B\nsong.txt", Path::new(".")).unwrap_err();
        assert!(format!("{error:#}").contains("line 2"));
    }

    #[test]
    fn bad_extension_names_the_line() {
        let error = BookManifest::parse("song.md", Path::new(".")).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("line 1"), "{message}");
        assert!(message.contains("not a .txt/.crd file"), "{message}");
    }

    #[test]
    fn key_value_line_after_first_path_is_a_path() {
        let manifest =
            BookManifest::parse("song.txt\nTitle: not-a-header.txt", Path::new(".")).unwrap();
        assert_eq!(manifest.title, None);
        assert_eq!(manifest.songs.len(), 2);
        assert_eq!(
            manifest.songs[1].path,
            Path::new(".").join("Title: not-a-header.txt")
        );
    }

    #[test]
    fn absolute_paths_are_kept() {
        let manifest =
            BookManifest::parse("song.txt\n/abs/songs/other.crd", Path::new("/base")).unwrap();
        assert_eq!(manifest.songs[0].path, Path::new("/base").join("song.txt"));
        assert_eq!(manifest.songs[1].path, Path::new("/abs/songs/other.crd"));
    }

    #[test]
    fn empty_manifest_is_an_error() {
        let error = BookManifest::parse("# only comments\n\n", Path::new(".")).unwrap_err();
        assert!(format!("{error:#}").contains("lists no songs"));
    }

    #[test]
    fn load_reports_missing_file_with_manifest_line() {
        let dir = tempdir("missing");
        fs::write(dir.join("real.txt"), GOOD).unwrap();
        let manifest_path = dir.join("book.book");
        fs::write(&manifest_path, "real.txt\ngone.txt\n").unwrap();

        let error = BookManifest::load(&manifest_path).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("line 2"), "{message}");
        assert!(message.contains("song file not found"), "{message}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_resolves_paths_relative_to_the_manifest() {
        let dir = tempdir("resolve");
        let subdir = dir.join("songs");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("one.txt"), GOOD).unwrap();
        let manifest_path = dir.join("nested").join("book.book");
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(&manifest_path, "../songs/one.txt\n").unwrap();

        let manifest = BookManifest::load(&manifest_path).unwrap();
        // `Path::join` keeps `..` lexical; the OS resolves it on open, so the
        // existence check in `load` still passes.
        assert_eq!(
            manifest.songs[0].path,
            manifest_path.parent().unwrap().join("../songs/one.txt")
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn renders_book_with_title_block_and_song_bodies() {
        let manifest = BookManifest::parse(
            "Title: My Book\nAuthor: A. Writer\nsong.txt",
            Path::new("."),
        )
        .unwrap();
        let song = parse(GOOD);
        let tex = render_book(&manifest, &[song], false).unwrap();

        assert!(tex.starts_with("\\documentclass[12pt]{article}\n"), "{tex}");
        assert!(tex.contains("\\usepackage[chorded]{songs}"));
        assert!(tex.contains("\\newindex{titleidx}{titleidx}"));
        assert!(tex.contains("\\title{My Book}"));
        assert!(tex.contains("\\author{A. Writer}"));
        assert!(tex.contains("\\date{}"));
        assert!(tex.contains("\\maketitle"));
        assert!(tex.contains("\\showindex{Index of Songs}{titleidx}"));
        // The index id must be listed on the songs environment, or no song
        // would be registered in the index.
        assert!(tex.contains("\\begin{songs}{titleidx}"));
        assert!(tex.contains("\\beginsong{Good Song}[by={Someone}]"));
        assert!(tex.contains("\\end{songs}\n\\end{document}\n"));
        assert!(!tex.contains("\\newindex{chordidx}"));
        assert!(!tex.contains("Index of Chords"));
        assert!(!tex.contains("\\indexentry"));
    }

    #[test]
    fn chord_index_opt_in_declares_and_targets_chordidx() {
        let manifest = BookManifest::parse("song.txt", Path::new(".")).unwrap();
        let song = parse("[Verse]\n[Am]Hello [Am]there\nD  G  2x\n");
        let tex = render_book(&manifest, &[song], true).unwrap();

        assert!(tex.contains("\\newindex{chordidx}{chordidx}"), "{tex}");
        assert!(tex.contains("\\begin{songs}{titleidx}"));
        assert!(!tex.contains("songs}{titleidx,chordidx}"), "{tex}");
        assert!(tex.contains("\\showindex{Index of Chords}{chordidx}"));
        // One entry per distinct chord, after the \beginsong line.
        assert!(tex.contains(
            "\\beginsong{Untitled}\n\\indexentry[chordidx]{Am}%\n\\indexentry[chordidx]{D}%\n\\indexentry[chordidx]{G}%\n\\beginverse"
        ), "{tex}");
    }

    #[test]
    fn chord_entries_escape_latex_specials() {
        let manifest = BookManifest::parse("song.txt", Path::new(".")).unwrap();
        let song = parse("[Verse]\n[D/F#]Hello\n");
        let tex = render_book(&manifest, &[song], true).unwrap();
        assert!(tex.contains("\\indexentry[chordidx]{D/F\\#}%"), "{tex}");
    }

    #[test]
    fn escapes_manifest_metadata_like_song_metadata() {
        let manifest = BookManifest::parse("Title: 100% R&D\nsong.txt", Path::new(".")).unwrap();
        let tex = render_book(&manifest, &[], false).unwrap();
        assert!(tex.contains("\\title{100\\% R\\&D}"), "{tex}");
    }

    #[test]
    fn empty_title_header_is_treated_as_absent() {
        let manifest = BookManifest::parse("Title:\nsong.txt", Path::new(".")).unwrap();
        assert_eq!(manifest.title.as_deref(), Some(""));
        let tex = render_book(&manifest, &[], false).unwrap();
        assert!(!tex.contains("\\title"), "{tex}");
        assert!(!tex.contains("\\maketitle"));
    }

    #[test]
    fn no_title_block_skips_maketitle() {
        let manifest = BookManifest::parse("song.txt", Path::new(".")).unwrap();
        let tex = render_book(&manifest, &[], false).unwrap();
        assert!(!tex.contains("\\title"), "{tex}");
        assert!(!tex.contains("\\author"), "{tex}");
        assert!(!tex.contains("\\date"), "{tex}");
        assert!(!tex.contains("\\maketitle"));
        assert!(tex.contains("\\begin{document}\n\\showindex"));
    }

    /// Golden book test (ADR0008): explicitly discard tabs to preserve the
    /// historical non-tab document byte for byte. Synthetic tablature tests
    /// separately cover the new literal-retention default.
    #[test]
    fn golden_book_matches_fixture() {
        let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let manifest = BookManifest::load(&examples.join("demo-book.book")).unwrap();
        let mut songs = Vec::new();
        for entry in &manifest.songs {
            let raw = fs::read_to_string(&entry.path).unwrap();
            let mut song = parse(&raw);
            assert!(
                !song.sections.is_empty(),
                "{} produced no sections",
                entry.path.display()
            );
            apply_noninteractive_defaults(&mut song);
            let mut review = TriageState::new(song, &raw);
            for line in &mut review.lines {
                if matches!(line.kind, LineKind::Fretboard(_)) {
                    line.verdict = Verdict::Discard;
                }
            }
            songs.push(review.finish());
        }
        let tex = render_book(&manifest, &songs, false).unwrap();
        let golden = fs::read_to_string(examples.join("demo-book.tex")).unwrap();
        assert_eq!(tex, golden);
    }

    // --- parts-based composition (ADR0019) ---

    /// Derives parts the way the wasm ferry does: one `render(song, false)`
    /// body plus core's `distinct_chords` per song.
    fn parts_for(songs: &[Song]) -> Vec<BookPart> {
        songs
            .iter()
            .map(|song| BookPart {
                body: render(song, false).unwrap(),
                chords: distinct_chords(song),
            })
            .collect()
    }

    #[test]
    fn compose_book_equals_render_book_for_the_same_songs() {
        let manifest = BookManifest::parse(
            "Title: My Book\nAuthor: A. Writer\nsong.txt\nsong.crd",
            Path::new("."),
        )
        .unwrap();
        let songs = [
            parse(GOOD),
            parse("[Verse]\n[Am]Hello [Am]there\nD  G  2x\n"),
            // A chordless part must pass through untouched either way.
            parse("[Chorus]\nLa la la\n"),
        ];
        let parts = parts_for(&songs);
        for chord_index in [false, true] {
            assert_eq!(
                compose_book(
                    manifest.title.as_deref(),
                    manifest.author.as_deref(),
                    &parts,
                    chord_index
                )
                .unwrap(),
                render_book(&manifest, &songs, chord_index).unwrap(),
                "chord_index={chord_index}"
            );
        }
    }

    #[test]
    fn compose_book_with_no_parts_is_a_valid_empty_book() {
        let manifest = BookManifest::parse("song.txt", Path::new(".")).unwrap();
        for chord_index in [false, true] {
            assert_eq!(
                compose_book(None, None, &[], chord_index).unwrap(),
                render_book(&manifest, &[], chord_index).unwrap(),
                "chord_index={chord_index}"
            );
        }
        let tex = compose_book(None, None, &[], false).unwrap();
        assert!(
            tex.contains("\\begin{songs}{titleidx}\n\\end{songs}"),
            "{tex}"
        );
        assert!(tex.contains("\\begin{document}\n\\showindex"), "{tex}");
    }

    #[test]
    fn compose_book_treats_empty_metadata_as_absent() {
        let tex = compose_book(Some(""), Some("Someone"), &[], false).unwrap();
        assert!(!tex.contains("\\title"), "{tex}");
        assert!(tex.contains("\\author{Someone}"), "{tex}");
        assert!(tex.contains("\\maketitle"));
    }

    #[test]
    fn compose_book_injects_chord_entries_between_the_part_boundaries() {
        let song = parse("[Verse]\n[Am]Hello [Am]there\nD  G  2x\n");
        let part = BookPart {
            body: render(&song, false).unwrap(),
            chords: distinct_chords(&song),
        };
        assert_eq!(part.chords, ["Am", "D", "G"]);
        let tex = compose_book(None, None, &[part], true).unwrap();
        assert!(
            tex.contains(
                "\\beginsong{Untitled}\n\\indexentry[chordidx]{Am}%\n\\indexentry[chordidx]{D}%\n\
             \\indexentry[chordidx]{G}%\n\\beginverse"
            ),
            "{tex}"
        );

        // A part with no chords is appended verbatim, no injection.
        let plain = BookPart {
            body: "\\beginsong{Plain}\n\\beginverse\nla\n\\endverse\n\\endsong\n".to_string(),
            chords: Vec::new(),
        };
        let tex = compose_book(None, None, &[plain], true).unwrap();
        assert!(tex.contains("\\beginsong{Plain}\n\\beginverse"), "{tex}");
    }
}
