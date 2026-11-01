use super::chord::{chord_tokens, inline_chords, is_chord, merge_chord_line};
use super::song::{LineKind, Section, SectionKind, Song};
use regex::Regex;
use std::sync::OnceLock;

static SECTION_HEADER_RE: OnceLock<Regex> = OnceLock::new();
static FRETBOARD_RE: OnceLock<Regex> = OnceLock::new();
static META_RE: OnceLock<Regex> = OnceLock::new();
static URL_RE: OnceLock<Regex> = OnceLock::new();
static STAR_RE: OnceLock<Regex> = OnceLock::new();
static NOTE_RE: OnceLock<Regex> = OnceLock::new();

fn section_header_re() -> &'static Regex {
    SECTION_HEADER_RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)^\s*[\[\(\*#]*(?:guitar\s+)?",
            r"(intro|verse|pre.?chorus|chorus|bridge|solo|interlude|instrumental|",
            r"outro|hook|refrain|riff|breakdown|tag|coda|ending)\b",
            r".*?[\]\)\:]*\s*$",
        ))
        .unwrap()
    })
}

fn fretboard_re() -> &'static Regex {
    FRETBOARD_RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*[A-G]#?\|(?:[-]|[-0-9hbpsl/\\~^xv\(\)\|\s])+\|?\s*$").unwrap()
    })
}

fn meta_re() -> &'static Regex {
    META_RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)^\s*(song(?:\s+name)?|title|artist(?:\s+name)?|band|",
            r"key(?:\s+signature)?|capo|tuning|difficulty|author|tabbed\s+by)\b",
            r"\s*(?::|[-–—])?\s*(.*)$",
        ))
        .unwrap()
    })
}

fn url_re() -> &'static Regex {
    URL_RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)(https?://|www\.|",
            r"\b[a-z0-9][a-z0-9.-]*\.(com|net|org|io|co|dk|de|gov|edu)\b)",
        ))
        .unwrap()
    })
}

fn star_re() -> &'static Regex {
    STAR_RE.get_or_init(|| {
        Regex::new(concat!(
            r"(?i)([★☆⭐]|",
            r"\b\d+(?:\.\d+)?\s*(?:/\s*5|out of\s*5|stars?)\b|",
            r"\b(?:rating|rated)\b|\b\d+\s+votes?\b|\b\d+k?\s+views?\b)",
        ))
        .unwrap()
    })
}

fn note_re() -> &'static Regex {
    NOTE_RE.get_or_init(|| Regex::new(r"^[A-G][#b]?$").unwrap())
}

/// Maps a header name to a section kind.
fn section_kind(name: &str) -> SectionKind {
    match name.to_lowercase().as_str() {
        "intro" => SectionKind::Intro,
        "verse" => SectionKind::Verse,
        s if s.starts_with("pre") && s.contains("chorus") => SectionKind::PreChorus,
        "chorus" => SectionKind::Chorus,
        "bridge" => SectionKind::Bridge,
        "solo" => SectionKind::Solo,
        "interlude" => SectionKind::Interlude,
        "instrumental" => SectionKind::Instrumental,
        "outro" => SectionKind::Outro,
        "hook" => SectionKind::Other("hook".into()),
        "refrain" => SectionKind::Other("refrain".into()),
        other => SectionKind::Other(other.into()),
    }
}

/// Parses raw tab text into a `Song` using heuristic classification.
///
/// Contiguous metadata/noise/blank lines at the top form the preamble: metadata
/// is extracted and the whole preamble is excluded from the song body. A
/// chord-only line immediately above a lyric line is merged into that lyric so
/// both inline `[Chord]` input and the raw column-aligned chord-over-lyrics
/// layout produce the same `LineKind::Lyric` representation.
pub fn parse(raw: &str) -> Song {
    let lines: Vec<&str> = raw.lines().collect();
    let (mut song, preamble) = analyze_preamble(&lines);

    let mut current = Section {
        kind: SectionKind::Intro,
        lines: Vec::new(),
    };
    let mut pending_chords: Option<String> = None;

    for line in &lines[preamble..] {
        let trimmed = line.trim();
        let kind = classify_line(trimmed);
        match kind {
            LineKind::SectionHeader(kind, _) => {
                flush_pending(&mut current, &mut pending_chords);
                if current.lines.is_empty() {
                    current.kind = kind;
                } else {
                    song.sections.push(std::mem::replace(
                        &mut current,
                        Section {
                            kind,
                            lines: Vec::new(),
                        },
                    ));
                }
            }
            LineKind::ChordOnly(_) => {
                flush_pending(&mut current, &mut pending_chords);
                pending_chords = Some((*line).to_string());
            }
            LineKind::Lyric(_) | LineKind::Ambiguous(_) => {
                if let Some(chords) = pending_chords.take() {
                    current
                        .lines
                        .push(LineKind::Lyric(merge_chord_line(&chords, line)));
                } else {
                    current.lines.push(kind);
                }
            }
            LineKind::Blank | LineKind::Noise(_) => {
                flush_pending(&mut current, &mut pending_chords);
            }
            LineKind::Fretboard(_) => {
                flush_pending(&mut current, &mut pending_chords);
                current.lines.push(kind);
            }
        }
    }
    flush_pending(&mut current, &mut pending_chords);
    if !current.lines.is_empty() {
        song.sections.push(current);
    }
    song
}

fn flush_pending(current: &mut Section, pending: &mut Option<String>) {
    if let Some(chords) = pending.take() {
        current.lines.push(LineKind::ChordOnly(chords));
    }
}

/// Extracts metadata from the preamble of raw tab text.
#[allow(dead_code)]
pub fn extract_metadata(raw: &str) -> Song {
    let lines: Vec<&str> = raw.lines().collect();
    analyze_preamble(&lines).0
}

/// Number of leading lines `parse` consumes as preamble (metadata, noise, and
/// title/artist candidates). Triage uses this to mark preamble lines without
/// duplicating the parser's rules (ADR0014).
pub fn preamble_len(raw: &str) -> usize {
    let lines: Vec<&str> = raw.lines().collect();
    analyze_preamble(&lines).1
}

/// Tracks whether title/artist were explicitly labelled in the input, so the
/// TUI can mark inferred values as correctable guesses (ADR0010).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MetadataProvenance {
    pub title_labelled: bool,
    pub artist_labelled: bool,
}

/// Scans the raw input for explicit title/artist labels.
pub fn metadata_provenance(raw: &str) -> MetadataProvenance {
    let mut provenance = MetadataProvenance::default();
    for line in raw.lines() {
        let Some(captures) = meta_re().captures(line.trim()) else {
            continue;
        };
        let label = captures[1].to_lowercase();
        let value = captures
            .get(2)
            .map(|m| m.as_str().trim())
            .unwrap_or_default();
        if value.is_empty() {
            continue;
        }
        match label.as_str() {
            "song" | "song name" | "title" => provenance.title_labelled = true,
            "artist" | "artist name" | "band" => provenance.artist_labelled = true,
            _ => {}
        }
    }
    provenance
}

/// Maps a (possibly newly edited) line to a section kind for triage promotion.
pub fn section_kind_from_header(line: &str) -> SectionKind {
    if let Some(captures) = section_header_re().captures(line) {
        if let Some(name) = captures.get(1) {
            return section_kind(name.as_str());
        }
    }
    let trimmed = line
        .trim()
        .trim_matches(|ch| matches!(ch, '[' | ']' | '(' | ')' | '*' | ':'))
        .trim();
    if trimmed.is_empty() {
        SectionKind::Other("section".into())
    } else {
        SectionKind::Other(trimmed.to_lowercase())
    }
}

/// Extracts labelled metadata and infers missing title/artist from leading bare
/// header lines. Also returns the index where the song body starts.
///
/// At most the first two bare (unclassified) lines before the first section,
/// chord, or tab line are treated as title/artist candidates. Candidates are
/// only consumed for fields that were not explicitly labelled; unused
/// candidates are left in the body. Scanning stops as soon as anything that can
/// only belong to the song body is seen, so lyrics after the body has started
/// are never reclassified.
fn analyze_preamble(lines: &[&str]) -> (Song, usize) {
    let mut song = Song::default();
    let mut consumed = vec![false; lines.len()];
    let mut candidates = Vec::new();
    let mut scan_end = 0;
    let mut in_chord_diagram = false;

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.contains("chords used") {
            in_chord_diagram = true;
        }
        if trimmed.is_empty() || is_metadata_line(trimmed) || is_noise(trimmed) {
            consumed[index] = true;
            scan_end = index + 1;
            if trimmed.is_empty() {
                in_chord_diagram = false;
            }
            continue;
        }
        if in_chord_diagram && (is_chord_only_line(trimmed) || fretboard_re().is_match(trimmed)) {
            consumed[index] = true;
            scan_end = index + 1;
            continue;
        }
        if is_body_start_line(trimmed) {
            break;
        }
        if candidates.len() < 2 {
            candidates.push(index);
            consumed[index] = true;
            scan_end = index + 1;
            continue;
        }
        break;
    }

    for index in 0..scan_end {
        if consumed[index] && is_metadata_line(lines[index].trim()) {
            apply_metadata_line(&mut song, lines[index].trim());
        }
    }

    let mut remaining = candidates.into_iter();
    if song.title.is_none() {
        if let Some(index) = remaining.next() {
            song.title = Some(lines[index].trim().to_string());
        }
    }
    if song.artist.is_none() {
        if let Some(index) = remaining.next() {
            song.artist = Some(lines[index].trim().to_string());
        }
    }
    for index in remaining {
        consumed[index] = false;
    }

    let body_start = (0..scan_end)
        .find(|&index| !consumed[index])
        .unwrap_or(scan_end);
    (song, body_start)
}

/// True for lines that unambiguously begin the song body rather than a header.
fn is_body_start_line(line: &str) -> bool {
    section_header_re().is_match(line)
        || fretboard_re().is_match(line)
        || is_chord_only_line(line)
        || !inline_chords(line).is_empty()
}

fn is_metadata_line(line: &str) -> bool {
    meta_re().is_match(line)
}

/// Applies an explicitly labelled metadata line (`Song:`, `Artist:`, `Key:`,
/// `Capo:`, `Tuning:`, …) to the song. Returns true if the line was consumed.
fn apply_metadata_line(song: &mut Song, line: &str) -> bool {
    let Some(captures) = meta_re().captures(line) else {
        return false;
    };
    let label = captures[1].to_lowercase();
    let value = captures
        .get(2)
        .map(|m| m.as_str().trim())
        .unwrap_or_default();

    match label.as_str() {
        "song" | "song name" | "title" => {
            if !value.is_empty() {
                song.title = Some(value.to_string());
            }
        }
        "artist" | "artist name" | "band" => {
            if !value.is_empty() {
                song.artist = Some(value.to_string());
            }
        }
        "key" | "key signature" => song.key = normalize_key(value),
        "capo" => song.capo = normalize_capo(value),
        "tuning" => {
            if !value.is_empty() {
                song.tuning = Some(value.to_string());
            }
        }
        // Difficulty/author/tabbed-by are preamble metadata we consume but
        // do not render.
        _ => {}
    }
    true
}

fn normalize_key(value: &str) -> Option<String> {
    let value = value.split(['(', '[']).next().unwrap_or(value).trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn normalize_capo(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let lower = value.to_lowercase();
    if lower.contains("none") || lower.contains("no capo") || lower == "0" || lower.contains("open")
    {
        return None;
    }
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        Some(digits)
    } else {
        Some(value.to_string())
    }
}

/// Heuristically classifies a single line.
pub fn classify_line(line: &str) -> LineKind {
    if line.is_empty() {
        return LineKind::Blank;
    }
    if section_header_re().is_match(line) {
        let name = section_header_re()
            .captures(line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("");
        return LineKind::SectionHeader(section_kind(name), line.to_string());
    }
    if fretboard_re().is_match(line) {
        return LineKind::Fretboard(line.to_string());
    }
    let inline = inline_chords(line);
    if !inline.is_empty() {
        // A line with inline chord markers: chords must all be valid.
        if inline.iter().all(|c| is_chord(c)) {
            return LineKind::Lyric(line.to_string());
        }
        return LineKind::Ambiguous(line.to_string());
    }
    // Tuning blocks such as `E A D G B E` look exactly like chord-only lines,
    // so they must be ruled out first.
    if is_tuning_line(line) {
        return LineKind::Noise(line.to_string());
    }
    if is_chord_only_line(line) {
        return LineKind::ChordOnly(line.to_string());
    }
    if is_noise(line) {
        return LineKind::Noise(line.to_string());
    }
    LineKind::Ambiguous(line.to_string())
}

/// A line consisting only of chord tokens (optionally with repeat counts like `2x`).
fn is_chord_only_line(line: &str) -> bool {
    let tokens = chord_tokens(line);
    if tokens.is_empty() {
        return false;
    }
    let mut any_chord = false;
    for token in &tokens {
        if is_chord(&token.text) {
            any_chord = true;
        } else if !is_repeat(&token.text) {
            return false;
        }
    }
    any_chord
}

fn is_repeat(token: &str) -> bool {
    let lower = token.to_lowercase();
    let digits = lower
        .strip_suffix('x')
        .or_else(|| lower.strip_prefix('x'))
        .unwrap_or("");
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// Detects typical UG/website noise lines.
fn is_noise(line: &str) -> bool {
    if is_punctuation_only(line) {
        return true;
    }
    if url_re().is_match(line) || star_re().is_match(line) {
        return true;
    }
    if is_tuning_line(line) {
        return true;
    }
    let lower = line.to_lowercase();
    const NOISE_PATTERNS: &[&str] = &[
        "tabbed by",
        "tuning:",
        "tuning ",
        "capo:",
        "capo ",
        "key:",
        "key -",
        "chords used",
        "www.",
        "http",
        "ultimate-guitar",
        "ultimate guitar",
        "was this info helpful",
        "difficulty",
        "author:",
        "submit correction",
        "upgrade",
        "ad-free",
        "ad free",
        "go ad free",
        "unlock",
        "subscribe",
        "premium",
        "get the app",
        "download the app",
        "install the app",
        "transpose",
        "auto scroll",
        "auto-scroll",
        "play along",
        "official tab",
        "interactive tab",
        "song:",
        "title:",
        "artist:",
        "band:",
    ];
    NOISE_PATTERNS.iter().any(|pattern| lower.contains(pattern))
}

fn is_tuning_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.contains("tuning") || lower.trim() == "standard" {
        return true;
    }
    const TUNING_PHRASES: &[&str] = &[
        "drop d",
        "drop c",
        "drop b",
        "half step",
        "full step",
        "half-step",
        "open g",
        "open d",
        "open e",
        "dadgad",
        "standard (",
        "standard tuning",
        "e a d g b e",
        "eb ab db gb bb eb",
    ];
    if TUNING_PHRASES.iter().any(|phrase| lower.contains(phrase)) {
        return true;
    }
    let tokens = chord_tokens(line);
    tokens.len() >= 5 && tokens.iter().all(|token| note_re().is_match(&token.text))
}

fn is_punctuation_only(line: &str) -> bool {
    let mut saw_non_space = false;
    for ch in line.chars() {
        if ch.is_whitespace() {
            continue;
        }
        saw_non_space = true;
        if ch.is_alphanumeric() {
            return false;
        }
    }
    saw_non_space
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_section_headers() {
        for (line, kind) in [
            ("[Verse 1]", SectionKind::Verse),
            ("[Chorus]", SectionKind::Chorus),
            ("[Pre-Chorus]", SectionKind::PreChorus),
            ("*Bridge*", SectionKind::Bridge),
            ("Intro:", SectionKind::Intro),
            ("Guitar Solo", SectionKind::Solo),
            ("[Riff]", SectionKind::Other("riff".into())),
        ] {
            assert_eq!(
                classify_line(line),
                LineKind::SectionHeader(kind, line.into())
            );
        }
    }

    #[test]
    fn classifies_fretboard() {
        for line in ["e|---3---3---|", "B|-1---0---|", "E|----------- ------3-|"] {
            assert!(matches!(classify_line(line), LineKind::Fretboard(_)));
        }
    }

    #[test]
    fn classifies_chord_only() {
        for line in [
            "Am  Fmaj7  C/G",
            "Am  F  C  G  2x",
            "| Am | F | C |",
            "(Am)  F,  C  2x",
        ] {
            assert!(
                matches!(classify_line(line), LineKind::ChordOnly(_)),
                "expected {line} to be chord-only"
            );
        }
    }

    #[test]
    fn classifies_noise() {
        for line in [
            "Tabbed by: someone",
            "https://www.ultimate-guitar.com/tab/x",
            "Submit corrections",
            "Upgrade to Pro",
            "Go ad-free with Ultimate Guitar",
            "4.5 stars out of 5",
            "★★★★☆",
            "Difficulty: Expert",
            "Author: someone",
            "Tuning: Standard (E A D G B E)",
            "Standard (E A D G B E)",
            "Drop D",
            "E A D G B E",
            "---",
            "...",
            "Chords used:",
        ] {
            assert!(
                matches!(classify_line(line), LineKind::Noise(_)),
                "expected {line} to be noise, got {:?}",
                classify_line(line)
            );
        }
    }

    #[test]
    fn tuning_block_is_not_chord_only() {
        assert!(matches!(classify_line("E A D G B E"), LineKind::Noise(_)));
        assert!(matches!(classify_line("C G Am F"), LineKind::ChordOnly(_)));
    }

    #[test]
    fn classifies_inline_chord_lyrics() {
        assert!(matches!(
            classify_line("[Am]Let it [F]be, let it [C]be"),
            LineKind::Lyric(_)
        ));
    }

    #[test]
    fn invalid_inline_chord_is_ambiguous() {
        assert!(matches!(
            classify_line("[Hm]Not really a chord"),
            LineKind::Ambiguous(_)
        ));
    }

    #[test]
    fn plain_text_is_ambiguous() {
        assert!(matches!(
            classify_line("some line the classifier won't know"),
            LineKind::Ambiguous(_)
        ));
    }

    #[test]
    fn extracts_labelled_metadata() {
        let raw = "\
Song: Let It Be
Artist: The Beatles
Tabbed by: example
Tuning: Standard (E A D G B E)
Key: C
Capo: none

[Verse 1]
[Am]When I find myself in [F]times of trouble";
        let song = extract_metadata(raw);
        assert_eq!(song.title.as_deref(), Some("Let It Be"));
        assert_eq!(song.artist.as_deref(), Some("The Beatles"));
        assert_eq!(song.key.as_deref(), Some("C"));
        assert_eq!(song.capo, None);
        assert_eq!(song.tuning.as_deref(), Some("Standard (E A D G B E)"));
    }

    #[test]
    fn metadata_label_variants() {
        let raw = "\
Title - Hey Jude
Band: The Beatles
Key signature: F
Capo: 2nd fret
";
        let song = extract_metadata(raw);
        assert_eq!(song.title.as_deref(), Some("Hey Jude"));
        assert_eq!(song.artist.as_deref(), Some("The Beatles"));
        assert_eq!(song.key.as_deref(), Some("F"));
        assert_eq!(song.capo.as_deref(), Some("2"));
    }

    #[test]
    fn infers_title_and_artist_from_unlabelled_preamble() {
        let raw = "\nLet It Be\nThe Beatles\n\n[Verse]\n[Am]When I find myself";
        let song = parse(raw);
        assert_eq!(song.title.as_deref(), Some("Let It Be"));
        assert_eq!(song.artist.as_deref(), Some("The Beatles"));
        assert_eq!(song.sections.len(), 1);
        assert_eq!(song.sections[0].kind, SectionKind::Verse);
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::Lyric("[Am]When I find myself".into())]
        );
    }

    #[test]
    fn infers_only_title_when_no_second_bare_line_exists() {
        let raw = "Hey Jude\n\n[Verse]\nNa na na";
        let song = parse(raw);
        assert_eq!(song.title.as_deref(), Some("Hey Jude"));
        assert_eq!(song.artist, None);
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::Ambiguous("Na na na".into())]
        );
    }

    #[test]
    fn explicit_labels_disable_inference() {
        let raw = "Song: Labelled\nArtist: Someone\nSome Bare Line\n[Verse]\nHello";
        let song = parse(raw);
        assert_eq!(song.title.as_deref(), Some("Labelled"));
        assert_eq!(song.artist.as_deref(), Some("Someone"));
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::Ambiguous("Some Bare Line".into())]
        );
        assert_eq!(
            song.sections[1].lines,
            vec![LineKind::Ambiguous("Hello".into())]
        );
    }

    #[test]
    fn mixed_labelled_title_infers_missing_artist() {
        let raw = "Title: Hey Jude\nThe Beatles\n[Verse]\nNa na na";
        let song = parse(raw);
        assert_eq!(song.title.as_deref(), Some("Hey Jude"));
        assert_eq!(song.artist.as_deref(), Some("The Beatles"));
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::Ambiguous("Na na na".into())]
        );
    }

    #[test]
    fn inference_does_not_consume_body_after_content_started() {
        let raw = "Title: X\nArtist: Y\n[Am]A lyric line\nAnother lyric line";
        let song = parse(raw);
        assert_eq!(song.title.as_deref(), Some("X"));
        assert_eq!(song.artist.as_deref(), Some("Y"));
        assert_eq!(
            song.sections[0].lines,
            vec![
                LineKind::Lyric("[Am]A lyric line".into()),
                LineKind::Ambiguous("Another lyric line".into())
            ]
        );
    }

    #[test]
    fn metadata_ignores_noise_without_labels() {
        let song = extract_metadata("Tabbed by: x\nhttps://example.com/tab\n\n[Verse]\nHello");
        assert_eq!(song.title, None);
        assert_eq!(song.artist, None);
        assert_eq!(song.key, None);
        assert_eq!(song.capo, None);
        assert_eq!(song.tuning, None);
    }

    #[test]
    fn parse_excludes_preamble_from_sections() {
        let raw = "\
Song: Let It Be
Artist: The Beatles
Tuning: Standard

[Verse 1]
[Am]When I find myself in [F]times of trouble";
        let song = parse(raw);
        assert_eq!(song.title.as_deref(), Some("Let It Be"));
        assert_eq!(song.artist.as_deref(), Some("The Beatles"));
        assert_eq!(song.sections.len(), 1);
        assert_eq!(song.sections[0].kind, SectionKind::Verse);
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::Lyric(
                "[Am]When I find myself in [F]times of trouble".into()
            )]
        );
    }

    #[test]
    fn parses_two_line_chord_format() {
        let raw = "\
[Verse]
Am        F
When I find myself
C             G
in times of trouble";
        let song = parse(raw);
        let lines = &song.sections[0].lines;
        assert_eq!(
            lines,
            &vec![
                LineKind::Lyric("[Am]When I fin[F]d myself".into()),
                LineKind::Lyric("[C]in times of tr[G]ouble".into()),
            ]
        );
    }

    #[test]
    fn parses_two_line_chord_format_with_bars_and_repeats() {
        let raw = "\
[Chorus]
| Am | F | C | G | 2x
Let it be, let it be, let it be, let it be";
        let song = parse(raw);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::Lyric("Le[Am]t it [F]be, [C]let [G]it be, let it be, let it be".into())
        );
    }

    #[test]
    fn chord_line_without_lyric_is_kept_as_chord_only() {
        let raw = "[Intro]\nAm  F  C  G\n\n[Verse]\n[Am]Hello";
        let song = parse(raw);
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::ChordOnly("Am  F  C  G".into())]
        );
    }

    #[test]
    fn chords_beyond_lyric_are_appended() {
        let raw = "[Verse]\nAm             G\nHello";
        let song = parse(raw);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::Lyric("[Am]Hello[G]".into())
        );
    }

    #[test]
    fn multiple_chords_in_merged_line() {
        let raw = "[Verse]\nAm  F   C    G\none two three four";
        let song = parse(raw);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::Lyric("[Am]one [F]two [C]three[G] four".into())
        );
    }

    #[test]
    fn provenance_distinguishes_labels_from_inference() {
        let labelled = metadata_provenance("Song: X\nArtist: Y\n[Verse]\nHello");
        assert!(labelled.title_labelled);
        assert!(labelled.artist_labelled);

        let inferred = metadata_provenance("\nX\nY\n[Verse]\nHello");
        assert!(!inferred.title_labelled);
        assert!(!inferred.artist_labelled);

        let partial = metadata_provenance("Title: X\nY\n[Verse]\nHello");
        assert!(partial.title_labelled);
        assert!(!partial.artist_labelled);
    }

    #[test]
    fn section_kind_from_header_handles_known_and_arbitrary_names() {
        assert_eq!(section_kind_from_header("[Chorus]"), SectionKind::Chorus);
        assert_eq!(
            section_kind_from_header("Pre-Chorus"),
            SectionKind::PreChorus
        );
        assert_eq!(
            section_kind_from_header("mystery line"),
            SectionKind::Other("mystery line".into())
        );
    }
}
