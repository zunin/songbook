use crate::latex::render::chorus_signature;
use crate::parser::classify::metadata_provenance;
use crate::parser::song::{LineKind, Song};

/// Builds the audit text describing what the heuristics decided without the
/// user. A pure function over the parsed song and raw input; used by `--yes`,
/// the non-TTY fallback, and batch mode. Callers print it verbatim.
pub fn audit_report(song: &Song, raw: &str) -> String {
    let provenance = metadata_provenance(raw);
    let mut lines: Vec<String> = vec!["Heuristic audit (no interactive review):".to_string()];

    let mut metadata: Vec<String> = Vec::new();
    if let Some(title) = &song.title {
        if provenance.title_labelled {
            metadata.push(format!("title=\"{title}\""));
        } else {
            lines.push(format!(
                "- title \"{title}\" is a guess (inferred from the first unlabelled line)"
            ));
        }
    }
    if let Some(artist) = &song.artist {
        if provenance.artist_labelled {
            metadata.push(format!("artist=\"{artist}\""));
        } else {
            lines.push(format!(
                "- artist \"{artist}\" is a guess (inferred from the next unlabelled line)"
            ));
        }
    }

    let mut ambiguous: Vec<&str> = Vec::new();
    let mut fretboard: Vec<&str> = Vec::new();
    for section in &song.sections {
        for line in &section.lines {
            match line {
                LineKind::Ambiguous(text) => ambiguous.push(text.as_str()),
                LineKind::Fretboard(text) => fretboard.push(text.as_str()),
                _ => {}
            }
        }
    }
    if !ambiguous.is_empty() {
        let quoted: Vec<String> = ambiguous.iter().map(|text| format!("\"{text}\"")).collect();
        lines.push(format!(
            "- {} ambiguous {} kept as lyric: {}",
            ambiguous.len(),
            plural(ambiguous.len(), "line", "lines"),
            quoted.join(", ")
        ));
    }
    if !fretboard.is_empty() {
        let outcome = if crate::tablature::validate(song).is_ok() {
            "retained literally"
        } else {
            "retained for review (export blocked)"
        };
        lines.push(format!(
            "- {} fretboard {} {outcome} (first: \"{}\")",
            fretboard.len(),
            plural(fretboard.len(), "line", "lines"),
            fretboard[0]
        ));
    }
    lines.extend(
        crate::tablature::warnings(song)
            .into_iter()
            .map(|w| format!("- {w}")),
    );
    let collapsed = count_collapsed_choruses(song);
    if collapsed > 0 {
        lines.push(format!(
            "- {} repeated {} collapsed",
            collapsed,
            plural(collapsed, "chorus", "choruses")
        ));
    }

    if let Some(key) = &song.key {
        metadata.push(format!("key={key}"));
    }
    if let Some(capo) = &song.capo {
        metadata.push(format!("capo={capo}"));
    }
    if let Some(tuning) = &song.tuning {
        metadata.push(format!("tuning={tuning}"));
    }
    if !metadata.is_empty() {
        lines.push(format!("- metadata present: {}", metadata.join(", ")));
    }

    if lines.len() == 1 {
        lines.push("- nothing needed a guess; every line was classified unambiguously".to_string());
    }

    let mut report = lines.join("\n");
    report.push('\n');
    report
}

/// Builds the one-line audit summary used in batch mode, where a full report
/// under every file would drown the output. Compact, semicolon-separated;
/// lists only what needed a guess.
pub fn audit_summary_line(song: &Song, raw: &str) -> String {
    let provenance = metadata_provenance(raw);
    let mut parts: Vec<String> = Vec::new();

    if let Some(title) = &song.title {
        if !provenance.title_labelled {
            parts.push(format!("title guessed: \"{title}\""));
        }
    }
    if let Some(artist) = &song.artist {
        if !provenance.artist_labelled {
            parts.push(format!("artist guessed: \"{artist}\""));
        }
    }

    let mut ambiguous = 0usize;
    let mut fretboard = 0usize;
    for section in &song.sections {
        for line in &section.lines {
            match line {
                LineKind::Ambiguous(_) => ambiguous += 1,
                LineKind::Fretboard(_) => fretboard += 1,
                _ => {}
            }
        }
    }
    if ambiguous > 0 {
        parts.push(format!("{ambiguous} ambiguous kept as lyric"));
    }
    if fretboard > 0 {
        let outcome = if crate::tablature::validate(song).is_ok() {
            "retained literally"
        } else {
            "retained for review (export blocked)"
        };
        parts.push(format!("{fretboard} fretboard {outcome}"));
    }
    parts.extend(crate::tablature::warnings(song));
    let collapsed = count_collapsed_choruses(song);
    if collapsed > 0 {
        parts.push(format!("{collapsed} chorus repeats collapsed"));
    }

    if parts.is_empty() {
        "no guesses needed".to_string()
    } else {
        parts.join("; ")
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural_form: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural_form
    }
}

/// Counts consecutive chorus sections the renderer will collapse into the
/// first chorus, mirroring the render-time equivalence check.
fn count_collapsed_choruses(song: &Song) -> usize {
    let mut previous: Option<String> = None;
    let mut collapsed = 0;
    for section_index in 0..song.sections.len() {
        match chorus_signature(song, section_index) {
            Some(signature) if previous.as_deref() == Some(signature.as_str()) => collapsed += 1,
            Some(signature) => previous = Some(signature),
            None => previous = None,
        }
    }
    collapsed
}

/// Applies the decisions the TUI would have defaulted to when no interactive
/// review runs: ambiguous lines are kept as lyrics (the triage default, so
/// `--yes` and batch output match the TUI's auto-accept), while fretboard
/// lines are retained literally with shared notation warnings.
pub fn apply_noninteractive_defaults(song: &mut Song) {
    for section in &mut song.sections {
        for line in &mut section.lines {
            if let LineKind::Ambiguous(text) = line {
                *line = LineKind::Lyric(text.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::classify::parse;

    #[test]
    fn reports_inferred_title_and_artist_as_guesses() {
        let raw = "\nLet It Be\nThe Beatles\n[Verse]\n[Am]Hello";
        let report = audit_report(&parse(raw), raw);
        assert!(
            report.contains("title \"Let It Be\" is a guess"),
            "{report}"
        );
        assert!(
            report.contains("artist \"The Beatles\" is a guess"),
            "{report}"
        );
        assert!(!report.contains("metadata present"), "{report}");
    }

    #[test]
    fn reports_labelled_metadata_as_present() {
        let raw = "Song: Let It Be\nArtist: The Beatles\nKey: C\nCapo: 2\n[Verse]\n[Am]Hello";
        let report = audit_report(&parse(raw), raw);
        assert!(
            report.contains(
                "metadata present: title=\"Let It Be\", artist=\"The Beatles\", key=C, capo=2"
            ),
            "{report}"
        );
        assert!(!report.contains("is a guess"), "{report}");
    }

    #[test]
    fn lists_ambiguous_lines_kept_as_lyrics() {
        let raw = "Song: T\n[Verse]\n[Am]Hello\nmystery one\nmystery two";
        let report = audit_report(&parse(raw), raw);
        assert!(
            report.contains("2 ambiguous lines kept as lyric: \"mystery one\", \"mystery two\""),
            "{report}"
        );
    }

    #[test]
    fn singular_ambiguous_line_reads_naturally() {
        let raw = "Song: T\n[Verse]\nmystery line";
        let report = audit_report(&parse(raw), raw);
        assert!(
            report.contains("1 ambiguous line kept as lyric: \"mystery line\""),
            "{report}"
        );
    }

    #[test]
    fn reports_blocked_fretboard_and_differing_choruses() {
        let raw = "Song: T\n[Chorus]\n[F]Same line\n\n[Chorus]\n[F]Same line\n\ne|---3---|\n";
        let report = audit_report(&parse(raw), raw);
        assert!(
            report.contains("1 fretboard line retained for review (export blocked)"),
            "{report}"
        );
        assert!(!report.contains("1 repeated chorus collapsed"), "{report}");
    }

    #[test]
    fn reports_clean_song_as_no_guesses() {
        let raw = "[Verse]\n[Am]Hello there";
        let report = audit_report(&parse(raw), raw);
        assert!(report.contains("nothing needed a guess"), "{report}");
        assert_eq!(report.lines().count(), 2);
        assert!(report.ends_with('\n'));
    }

    #[test]
    fn noninteractive_defaults_keep_ambiguous_as_lyric() {
        let raw = "Song: T\n[Verse]\n[Am]Hello\nmystery line";
        let mut song = parse(raw);
        apply_noninteractive_defaults(&mut song);
        assert!(song.sections[0]
            .lines
            .contains(&LineKind::Lyric("mystery line".into())));
        assert!(!song.sections[0]
            .lines
            .iter()
            .any(|line| matches!(line, LineKind::Ambiguous(_))));
    }

    #[test]
    fn summary_line_lists_guesses_compactly() {
        let raw = "\nLet It Be\nThe Beatles\n[Verse]\n[Am]Hello\nmystery\ne|---3---|";
        let summary = audit_summary_line(&parse(raw), raw);
        assert!(summary.starts_with(
            "title guessed: \"Let It Be\"; artist guessed: \"The Beatles\"; \
             1 ambiguous kept as lyric; 1 fretboard retained for review (export blocked)"
        ));
    }

    #[test]
    fn summary_line_reports_clean_song() {
        let raw = "Song: T\nArtist: A\n[Verse]\n[Am]Hello";
        assert_eq!(audit_summary_line(&parse(raw), raw), "no guesses needed");
    }
}
