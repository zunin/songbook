use super::chord::{chord_token_spans, chord_tokens, inline_chords, is_chord, merge_chord_line};
use super::song::{LineKind, Section, SectionKind, Song};
use regex::Regex;
use std::sync::OnceLock;

static SECTION_HEADER_RE: OnceLock<Regex> = OnceLock::new();
static BRACKET_TAG_RE: OnceLock<Regex> = OnceLock::new();
static FRETBOARD_RE: OnceLock<Regex> = OnceLock::new();
static META_RE: OnceLock<Regex> = OnceLock::new();
static LABEL_SCAN_RE: OnceLock<Regex> = OnceLock::new();
static LABEL_ONLY_RE: OnceLock<Regex> = OnceLock::new();
static FINGERING_RE: OnceLock<Regex> = OnceLock::new();
static URL_RE: OnceLock<Regex> = OnceLock::new();
static STAR_RE: OnceLock<Regex> = OnceLock::new();
static NOTE_RE: OnceLock<Regex> = OnceLock::new();

/// Labels recognized in metadata lines (`Song:`, `Key signature:`, `Written by
/// …`, …). Order matters: the longest spelling of a prefix must come first
/// (`song name` before `song`) because the alternation is leftmost-first.
const META_LABELS: &[&str] = &[
    r"song\s+name",
    r"artist\s+name",
    r"key\s+signature",
    r"tabbed\s+by",
    r"transcribed\s+by",
    r"written\s+by",
    r"music\s+by",
    r"lyrics\s+by",
    r"lyric\s+by",
    r"words\s+by",
    "song",
    "title",
    "artist",
    "band",
    "album",
    "key",
    "capo",
    "tuning",
    "difficulty",
    "author",
    "written",
    "transcribed",
];

/// Section-ish vocabulary accepted inside a whole-line bracketed tag
/// (`[Verse 1]`, `[Chorus]`, `[Note]`, `[Chords]`, `[Tab]`, …).
const BRACKET_TAG_WORDS: &[&str] = &[
    "instrumental",
    "interlude",
    "breakdown",
    "strumming",
    "prechorus",
    "pre-chorus",
    "pre chorus",
    "refrain",
    "chords",
    "chord",
    "notes",
    "note",
    "intro",
    "verse",
    "chorus",
    "bridge",
    "solo",
    "outro",
    "riff",
    "break",
    "coda",
    "ending",
    "tuning",
    "tab",
    "tabs",
    "hook",
    "lead",
    "tag",
    "end",
];

/// Headings that introduce section content but carry no value of their own
/// (`Chords:`, `Note:`). Whole-line occurrences are noise.
const CONTENT_LABELS: &[&str] = &[
    "chords used",
    "strumming pattern",
    "chords",
    "chord",
    "notes",
    "note",
    "lyrics",
    "lyric",
    "rhythm",
    "tabs",
    "tab",
    "strumming",
];

const PROSE_WORD_MIN: usize = 8;

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

/// Matches a line that is *entirely* a bracketed section tag: `[Verse 1]`,
/// `[Chorus]`, `[Note]`, `[Chords]`, …. Case-insensitive; a trailing
/// number or parenthetical is allowed.
fn bracket_tag_re() -> &'static Regex {
    BRACKET_TAG_RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)^\s*\[\s*({}\b)[^\]]*\]\s*$",
            BRACKET_TAG_WORDS.join("|")
        ))
        .unwrap()
    })
}

/// Section name of a whole-line bracketed tag, if the line is one.
fn bracket_tag_name(line: &str) -> Option<&str> {
    bracket_tag_re()
        .captures(line)
        .and_then(|captures| captures.get(1))
        .map(|name| name.as_str())
}

/// Matches a whole-line content label such as `Chords:` or `Note:`.
fn label_only_re() -> &'static Regex {
    LABEL_ONLY_RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)^\s*(?:{})\s*(?::|[-–—])\s*$",
            CONTENT_LABELS.join("|")
        ))
        .unwrap()
    })
}

/// Matches the fingering part of a chord definition (`x02220`, `3-2-0-0-0-3`).
fn fingering_re() -> &'static Regex {
    FINGERING_RE.get_or_init(|| Regex::new(r"^[xX0-9\-/\\hpbr~()\s]+$").unwrap())
}

/// Candidate labelled notation row. The payload is opaque; only the label
/// and its first notation character are recognized, never notes or frets.
/// Digit/delimiter-leading forms are promoted contextually by `literal_rows`.
fn literal_candidate(line: &str) -> bool {
    let Some((label, body)) = line.trim().split_once('|') else {
        return false;
    };
    !label.is_empty()
        && label.len() <= 8
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'#')
        && (body.starts_with(['-', '|'])
            || (body.as_bytes().first().is_some_and(u8::is_ascii_digit)
                && body.contains(['-', '|'])))
}

/// Recognize consecutive labelled candidates, including unsupported 4/5/7-row
/// runs so they cannot evade export validation. A lone broad digit/delimiter
/// candidate remains conservative (ordinary prose/tables need not be tabs);
/// lone hyphen-leading rows keep the existing literal recognition.
fn literal_rows(lines: &[&str]) -> Vec<bool> {
    let mut literal = vec![false; lines.len()];
    let mut start = 0;
    while start < lines.len() {
        let mut end = start;
        while end < lines.len() && literal_candidate(lines[end]) {
            end += 1;
        }
        if end > start {
            if end - start > 1
                || lines[start]
                    .trim()
                    .split_once('|')
                    .is_some_and(|(_, body)| body.starts_with('-'))
            {
                literal[start..end].fill(true);
            }
            start = end;
        } else {
            start += 1;
        }
    }
    literal
}

/// A conservative parenthetical chord/fingering comment. One or more
/// whitespace-separated parenthesized items are allowed; each item is either a
/// chord in the existing syntax (including `*` decorations) or exactly six
/// string positions containing only digits and `x`/`X`. In particular, ordinary
/// parenthetical lyrics and open-ended hyphenated "fingering" prose do not
/// qualify for automatic association.
fn parenthetical_fingering_comment(line: &str) -> bool {
    let mut any = false;
    for token in line.split_whitespace() {
        let Some(inner) = token.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
            return false;
        };
        let six_positions = inner.len() == 6
            && inner
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'x' || b == b'X')
            && inner.bytes().any(|b| b.is_ascii_digit());
        if !six_positions && !is_chord(chord_shape(inner)) {
            return false;
        }
        any = true;
    }
    any
}

/// Literal chord heading, with `or` allowed only between valid chord tokens.
/// This is annotation recognition only, not a change to chord/lyric semantics.
fn tab_chord_heading(line: &str) -> bool {
    if is_chord_only_line(line.trim()) {
        return true;
    }
    let tokens: Vec<_> = line.split_whitespace().collect();
    !tokens.is_empty()
        && tokens.iter().enumerate().all(|(index, token)| {
            is_chord(chord_shape(token))
                || (*token == "or"
                    && index > 0
                    && index + 1 < tokens.len()
                    && is_chord(chord_shape(tokens[index - 1]))
                    && is_chord(chord_shape(tokens[index + 1])))
        })
}

/// Recognize only an explicit annotation prefix immediately above an original
/// six-labelled-string run. At least one parenthetical fingering/chord comment
/// is required; one adjacent chord heading may appear before or after the cues.
/// No blanks,
/// sections, metadata, prose, unrelated chord headings, or other tab runs are
/// crossed, and unsupported partial runs never acquire annotations.
fn tab_annotations(lines: &[&str], literal: &[bool]) -> Vec<bool> {
    let mut annotations = vec![false; lines.len()];
    let mut block_start = 0;
    while block_start < lines.len() {
        if !literal[block_start] {
            block_start += 1;
            continue;
        }
        let mut block_end = block_start + 1;
        while block_end < lines.len() && literal[block_end] {
            block_end += 1;
        }
        if block_end - block_start == 6 && block_start > 0 {
            let mut cursor = block_start;
            while cursor > 0 && parenthetical_fingering_comment(lines[cursor - 1]) {
                cursor -= 1;
            }
            let mut evidence = cursor < block_start;
            // At most one adjacent heading, with qualifying cues on either
            // side. Never cross a second heading to find distant evidence.
            if cursor > 0 && tab_chord_heading(lines[cursor - 1]) {
                cursor -= 1;
                let before_heading = cursor;
                while cursor > 0 && parenthetical_fingering_comment(lines[cursor - 1]) {
                    cursor -= 1;
                }
                evidence |= cursor < before_heading;
            }
            // Explicit parenthetical evidence is mandatory. This avoids
            // absorbing a genuine chord-only line before an unrelated tab.
            if evidence {
                annotations[cursor..block_start].fill(true);
            }
        }
        block_start = block_end;
    }
    annotations
}

fn fretboard_re() -> &'static Regex {
    FRETBOARD_RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*[A-G][#b]?\|(?:[-]|[-0-9hbpsl/\\~^xv\(\)\|\s])+\|?\s*$").unwrap()
    })
}

fn meta_re() -> &'static Regex {
    META_RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)^\s*({})\b\s*(?::|[-–—])?\s*(.*)$",
            META_LABELS.join("|")
        ))
        .unwrap()
    })
}

/// Matches a known label followed by its delimiter, anywhere in a line. Used
/// to find the extra labels glued into the value of a run-together line.
fn label_scan_re() -> &'static Regex {
    LABEL_SCAN_RE.get_or_init(|| {
        Regex::new(&format!(r"(?i)({})\s*(?::|[-–—])", META_LABELS.join("|"))).unwrap()
    })
}

/// Splits a metadata line into one `(label, value)` part per label occurrence.
///
/// Scraped tabs regularly lose the newline between two labelled lines, leaving
/// a run-together line such as `Tuning: E A D G B EKey: ACapo: No capo`. Every
/// known label therefore starts a new part, and text glued to the *front* of a
/// label (the `E` in `… B EKey:`) stays in the previous value — which is what
/// we want: the tuning value ends where the next label begins.
fn metadata_parts(line: &str) -> Vec<(&str, &str)> {
    let text = line.trim();
    let starts: Vec<usize> = label_scan_re()
        .find_iter(text)
        .map(|found| found.start())
        .collect();
    // Only lines that *begin* with a label are metadata; anything else (a
    // lyric line mentioning "key:") stays out of the metadata path.
    let begins_with_label = starts
        .first()
        .is_some_and(|&start| text[..start].trim().is_empty());
    if !begins_with_label {
        // A credit such as `Written by X` carries no delimiter, so the line is
        // one part instead of a run-together line to split.
        return metadata_part(text).into_iter().collect();
    }
    starts
        .iter()
        .enumerate()
        .filter_map(|(index, &start)| {
            let end = starts.get(index + 1).copied().unwrap_or(text.len());
            metadata_part(text[start..end].trim())
        })
        .collect()
}

/// The `(label, value)` of a single metadata part, if the text is a labelled
/// line at all.
fn metadata_part(text: &str) -> Option<(&str, &str)> {
    let captures = meta_re().captures(text)?;
    let label = captures.get(1)?.as_str();
    let value = captures.get(2).map_or("", |value| value.as_str().trim());
    is_metadata_part(text, label, value).then_some((label, value))
}

/// A part counts as metadata when a delimiter follows the label (`Key: A`,
/// `Capo - 2`) or when the label is a multi-word credit (`Written by X`) that
/// no lyric line would start with. A single bare word (`Key to the highway`)
/// is a lyric, not metadata.
fn is_metadata_part(part: &str, label: &str, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let after_label = part[label.len()..].trim_start();
    let explicit = after_label.starts_with([':', '-', '–', '—']);
    explicit || label.split_whitespace().count() > 1
}

/// Lower-cases a label and collapses its inner whitespace (`Song  Name` →
/// `song name`) so both spellings match the same field.
fn normalize_label(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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
///
/// The heuristics are deliberately mechanical:
///
/// * A line that is entirely a bracketed tag (`[Verse 1]`, `[Note]`,
///   `[Chords]`, `[Tab]`) is a section header — never lyric, never ambiguous.
/// * `ChordName:` groups followed by fingering characters (`F#m:   244222`,
///   `Bm/A:  x04432`) are chord definitions: noise, not lyric.
/// * Whole-line content labels (`Chords:`, `Note:`) are noise.
/// * Known metadata labels (`Song:`, `Artist:`, `Album:`, `Key:`, `Capo:`,
///   `Tuning:`, `Written by …`, `Transcribed by …`, …) are consumed as
///   metadata rather than kept as lyric, and a run-together line is split at
///   every label occurrence (`Tuning: E A D G B EKey: ACapo: No capo`).
/// * A bare preamble line of `PROSE_WORD_MIN` words or more is a wrapped
///   performance note, not a title: it is consumed (out of the body) and never
///   guessed as title or artist. Shorter bare lines stay title/artist
///   candidates.
/// * Chord tokens may carry a `*` decoration (`Em*` is `Em` at that column) and
///   bare repeat counts (`x12`, `4x`) are annotations: never chords, never
///   lyric, and they leave the merged columns untouched.
///
/// Known limitations (accepted, see `docs/adr`):
///
/// * A chord-only line with no lyric line below it in its section — an intro
///   chord before the next section tag — stays a chord-only line.
/// * Title/artist inference never looks at noise, headers, chord definitions,
///   chord-only lines, metadata, or wrapped prose, but it is still a guess:
///   without a `Song:`/`Title:` label the first genuine lyric line is used.
pub fn parse(raw: &str) -> Song {
    let lines: Vec<&str> = raw.lines().collect();
    let analysis = analyze_preamble(&lines);
    let mut song = analysis.song;
    let classified = classify_lines_from(&lines, analysis.body_start);

    let mut current = Section {
        kind: SectionKind::Intro,
        lines: Vec::new(),
    };
    let mut pending_chords: Option<String> = None;

    for (line, classified) in lines.iter().zip(&classified) {
        if classified.preamble {
            continue;
        }
        let kind = classified.kind.clone();
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
                    let plain = normalize_chord_line(&chords);
                    current
                        .lines
                        .push(LineKind::Lyric(merge_chord_line(&plain, line)));
                } else {
                    current.lines.push(kind);
                }
            }
            LineKind::Blank | LineKind::Noise(_) => {
                flush_pending(&mut current, &mut pending_chords);
            }
            LineKind::Fretboard(_) | LineKind::TabAnnotation(_) => {
                flush_pending(&mut current, &mut pending_chords);
                current.lines.push(kind);
            }
        }
    }
    flush_pending(&mut current, &mut pending_chords);
    if !current.lines.is_empty() {
        song.sections.push(current);
    }
    if analysis.skipped_prose && song.title.is_none() {
        song.title = first_lyric_line(&song.sections);
    }
    let original: Vec<_> = classified.iter().map(|c| c.kind.clone()).collect();
    let provenance = crate::tablature::source_provenance(&original);
    let retained: Vec<_> = classified
        .iter()
        .zip(provenance)
        .filter(|(c, _)| {
            !c.preamble && matches!(c.kind, LineKind::Fretboard(_) | LineKind::TabAnnotation(_))
        })
        .map(|(_, source)| source)
        .collect();
    crate::tablature::attach(&mut song, &retained);
    song
}

/// One raw line's contextual classification: the per-line kind plus whether
/// `parse` consumes the line as preamble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedLine {
    pub kind: LineKind,
    pub preamble: bool,
}

/// Contextually classifies every raw line, using the same pass as `parse`.
///
/// Unlike [`classify_line`], this sees the surrounding lines: a text line
/// immediately below a chord-only line is the chord line's lyric target and is
/// classified as `Lyric` (the chord line itself stays `ChordOnly`). The result
/// has one entry per `raw.lines()` entry, in the same order.
pub fn classify_lines(raw: &str) -> Vec<ClassifiedLine> {
    let lines: Vec<&str> = raw.lines().collect();
    let body_start = analyze_preamble(&lines).body_start;
    classify_lines_from(&lines, body_start)
}

/// The contextual pass over lines whose preamble boundary is already known.
/// `parse` calls this directly so its body classification and the public
/// `classify_lines` are the same pass, not two copies.
fn classify_lines_from(lines: &[&str], body_start: usize) -> Vec<ClassifiedLine> {
    let literal = literal_rows(lines);
    let annotations = tab_annotations(lines, &literal);
    let mut classified = Vec::with_capacity(lines.len());
    let mut pending_chords = false;

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if index < body_start {
            classified.push(ClassifiedLine {
                kind: classify_line(trimmed),
                preamble: true,
            });
            continue;
        }
        let kind = if annotations[index] {
            LineKind::TabAnnotation((*line).to_string())
        } else if literal[index] {
            LineKind::Fretboard((*line).to_string())
        } else {
            classify_line(trimmed)
        };
        let kind = if matches!(kind, LineKind::Fretboard(_)) {
            LineKind::Fretboard((*line).to_string())
        } else {
            kind
        };
        let kind = match kind {
            LineKind::SectionHeader(_, _)
            | LineKind::Blank
            | LineKind::Noise(_)
            | LineKind::Fretboard(_)
            | LineKind::TabAnnotation(_) => {
                pending_chords = false;
                kind
            }
            LineKind::ChordOnly(_) => {
                pending_chords = true;
                kind
            }
            LineKind::Lyric(_) | LineKind::Ambiguous(_) => {
                if std::mem::take(&mut pending_chords) {
                    LineKind::Lyric(trimmed.to_string())
                } else {
                    kind
                }
            }
        };
        classified.push(ClassifiedLine {
            kind,
            preamble: false,
        });
    }
    classified
}

/// Fallback title: the first genuine lyric line of the song with its inline
/// chord markers removed. Used when the preamble held a prose paragraph, which
/// must never become the title.
fn first_lyric_line(sections: &[Section]) -> Option<String> {
    sections
        .iter()
        .flat_map(|section| section.lines.iter())
        .find_map(|line| match line {
            LineKind::Lyric(text) => {
                let title = strip_inline_markers(text);
                (!title.is_empty()).then_some(title)
            }
            _ => None,
        })
}

/// Removes the `[Chord]` markers of a merged lyric line, leaving any bracketed
/// text that is not a chord (ad-libs such as `[oh]`) in place.
fn strip_inline_markers(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close_relative) = rest[open..].find(']') else {
            break;
        };
        let close = open + close_relative;
        let inner = &rest[open + 1..close];
        out.push_str(&rest[..open]);
        if !is_chord(inner) {
            out.push_str(&rest[open..=close]);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out.trim().to_string()
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
    analyze_preamble(&lines).song
}

/// Number of leading lines `parse` consumes as preamble (metadata, noise, and
/// title/artist candidates). Triage uses this to mark preamble lines without
/// duplicating the parser's rules (ADR0014).
pub fn preamble_len(raw: &str) -> usize {
    let lines: Vec<&str> = raw.lines().collect();
    analyze_preamble(&lines).body_start
}

/// Result of scanning the leading preamble of raw tab text.
struct Preamble {
    /// Metadata found (and inferred) in the preamble.
    song: Song,
    /// Index of the first line that belongs to the song body.
    body_start: usize,
    /// The preamble contained a wrapped prose paragraph that inference had to
    /// skip, so the title is better taken from the song body.
    skipped_prose: bool,
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
        for (label, value) in metadata_parts(line.trim()) {
            if value.is_empty() {
                continue;
            }
            match normalize_label(label).as_str() {
                "song" | "song name" | "title" => provenance.title_labelled = true,
                "artist" | "artist name" | "band" => provenance.artist_labelled = true,
                _ => {}
            }
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
/// header lines. Also reports the index where the song body starts.
///
/// At most the first two bare (unclassified) lines before the first section,
/// chord, or tab line are treated as title/artist candidates; wrapped prose is
/// consumed instead of being used as one, and when that happens the title falls
/// back to the first genuine lyric line of the body. Candidates are only
/// consumed for fields that were not explicitly labelled; unused candidates are
/// left in the body. Scanning stops as soon as anything that can only belong to
/// the song body is seen, so lyrics after the body has started are never
/// reclassified.
fn analyze_preamble(lines: &[&str]) -> Preamble {
    let mut song = Song::default();
    let mut consumed = vec![false; lines.len()];
    let mut candidates = Vec::new();
    let mut scan_end = 0;
    let mut in_chord_diagram = false;
    let mut skipped_prose = false;
    let literal = literal_rows(lines);
    let annotations = tab_annotations(lines, &literal);

    for (index, line) in lines.iter().enumerate() {
        // Useful literal notation and its explicit annotation prefix win over
        // a preceding chord-definition or diagram heading. Title/artist
        // inference must not consume a parenthetical fingering row as prose.
        if literal[index] || annotations[index] {
            break;
        }
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
        // A wrapped prose note is not a title: consume it and remember that
        // inference has to fall back to the song body.
        if looks_like_prose(trimmed) {
            skipped_prose = true;
            consumed[index] = true;
            scan_end = index + 1;
            continue;
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
    Preamble {
        song,
        body_start,
        skipped_prose,
    }
}

/// True for lines that unambiguously begin the song body rather than a header.
fn is_body_start_line(line: &str) -> bool {
    bracket_tag_name(line).is_some()
        || section_header_re().is_match(line)
        || fretboard_re().is_match(line)
        || is_chord_only_line(line)
        || !inline_chords(line).is_empty()
}

fn is_metadata_line(line: &str) -> bool {
    metadata_parts(line)
        .iter()
        .any(|(_, value)| !value.is_empty())
}

/// Applies an explicitly labelled metadata line (`Song:`, `Artist:`, `Key:`,
/// `Capo:`, `Tuning:`, …) to the song. Returns true if the line was consumed.
///
/// A run-together line is applied part by part, so `Tuning: …Key: ACapo: No
/// capo` fills all three fields.
fn apply_metadata_line(song: &mut Song, line: &str) -> bool {
    let parts = metadata_parts(line);
    let mut applied = false;
    for (label, value) in parts {
        applied = true;
        match normalize_label(label).as_str() {
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
            // Album/written-by/transcribed-by/difficulty/author/tabbed-by are
            // preamble metadata we consume but do not render.
            _ => {}
        }
    }
    applied
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
    if let Some(name) = bracket_tag_name(line) {
        return LineKind::SectionHeader(section_kind(name), line.to_string());
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
    // `F#m:   244222` is a chord definition (noise), not a chord line.
    if is_chord_definition_line(line) {
        return LineKind::Noise(line.to_string());
    }
    if is_chord_only_line(line) {
        return LineKind::ChordOnly(line.to_string());
    }
    if is_noise(line) || is_repeat_only_line(line) {
        return LineKind::Noise(line.to_string());
    }
    LineKind::Ambiguous(line.to_string())
}

/// A line consisting only of chord tokens (optionally decorated like `Em*` and
/// with repeat counts like `2x`).
fn is_chord_only_line(line: &str) -> bool {
    let tokens = chord_tokens(line);
    if tokens.is_empty() {
        return false;
    }
    let mut any_chord = false;
    for token in &tokens {
        if is_chord(chord_shape(&token.text)) {
            any_chord = true;
        } else if !is_repeat(&token.text) {
            return false;
        }
    }
    any_chord
}

/// The bare chord name of a token: a `*` decoration marks a fingering variant
/// (`Em*` is played as `Em`), so it is dropped for classification and merging.
fn chord_shape(token: &str) -> &str {
    token.trim_end_matches('*')
}

/// Rewrites a decorated chord line into the plain, column-aligned form the
/// chord merger expects: `*` decorations are dropped (`Em*` stays at its own
/// column) and repeat counts such as `x12` are blanked out with spaces so they
/// annotate without disturbing any chord's column.
fn normalize_chord_line(line: &str) -> String {
    let mut out = line.to_string();
    for span in chord_token_spans(line).into_iter().rev() {
        if is_chord(chord_shape(&span.token.text)) {
            continue;
        }
        let blank = " ".repeat(span.token.text.chars().count());
        out.replace_range(span.text_start..span.text_end, &blank);
    }
    out
}

/// A chord-definition line: one or more `ChordName:` groups whose values are
/// fingering characters only (`F#m:   244222`, `Bm/A:  x04432`, or several
/// definitions on one line). Noise — the fingering is not meant to be sung.
fn is_chord_definition_line(line: &str) -> bool {
    let mut parts = line.trim().split(':');
    let Some(first) = parts.next() else {
        return false;
    };
    if !is_chord(chord_shape(first.trim())) {
        return false;
    }
    let mut rest: Vec<&str> = parts.collect();
    let Some(last) = rest.pop() else {
        return false;
    };
    for middle in rest {
        // A middle part is the previous fingering plus the next chord name.
        let Some((fingering, name)) = middle.rsplit_once(char::is_whitespace) else {
            return false;
        };
        if !is_fingering(fingering) || !is_chord(chord_shape(name.trim())) {
            return false;
        }
    }
    is_fingering(last)
}

/// Fingering text of a chord definition: string/ fret symbols, digits and
/// separators, with at least one fret number or muted-string marker.
fn is_fingering(text: &str) -> bool {
    let text = text.trim();
    fingering_re().is_match(text) && text.chars().any(|ch| matches!(ch, 'x' | 'X' | '0'..='9'))
}

/// A line made only of repeat counts (`x4`, `2x`): an annotation for the chord
/// line above it, so never a chord and never a lyric.
fn is_repeat_only_line(line: &str) -> bool {
    let tokens = chord_tokens(line);
    !tokens.is_empty() && tokens.iter().all(|token| is_repeat(&token.text))
}

/// A bare preamble line long enough to be a wrapped performance note rather
/// than a title: `PROSE_WORD_MIN` or more words. A verse written in
/// chord-over-lyrics layout alternates a chord line with a lyric line, and the
/// preamble scan stops at the first chord line, so only notes reach this
/// length. Used only to keep notes out of title/artist inference.
fn looks_like_prose(line: &str) -> bool {
    line.split_whitespace().count() >= PROSE_WORD_MIN
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
    // Labelled metadata, whole-line content labels (`Chords:`) and chord
    // definitions (`F#m:   244222`) are preamble content, not lyric.
    if is_metadata_line(line) || label_only_re().is_match(line) || is_chord_definition_line(line) {
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

    /// Name of a line's classification, so tables can assert kinds without
    /// spelling out every `LineKind` payload.
    fn kind_name(line: &str) -> &'static str {
        match classify_line(line) {
            LineKind::Lyric(_) => "lyric",
            LineKind::ChordOnly(_) => "chord-only",
            LineKind::SectionHeader(_, _) => "section header",
            LineKind::Fretboard(_) => "fretboard",
            LineKind::TabAnnotation(_) => "tab annotation",
            LineKind::Noise(_) => "noise",
            LineKind::Blank => "blank",
            LineKind::Ambiguous(_) => "ambiguous",
        }
    }

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

    // --- real-world tab formats (heuristic refinement) -----

    /// Whole-line bracketed tags are section headers, never lyric and never
    /// ambiguous, whatever the section vocabulary word is.
    #[test]
    fn bracketed_tags_are_section_headers() {
        for (line, kind) in [
            ("[Intro]", SectionKind::Intro),
            ("[Verse 1]", SectionKind::Verse),
            ("[verse 2]", SectionKind::Verse),
            ("[Pre-Chorus]", SectionKind::PreChorus),
            ("[Prechorus]", SectionKind::PreChorus),
            ("[Chorus]", SectionKind::Chorus),
            ("[Bridge]", SectionKind::Bridge),
            ("[Solo]", SectionKind::Solo),
            ("[Instrumental]", SectionKind::Instrumental),
            ("[Interlude]", SectionKind::Interlude),
            ("[Outro]", SectionKind::Outro),
            ("[Note]", SectionKind::Other("note".into())),
            ("[notes]", SectionKind::Other("notes".into())),
            ("[Chords]", SectionKind::Other("chords".into())),
            ("[ Tab ]", SectionKind::Other("tab".into())),
            ("[Ending]", SectionKind::Other("ending".into())),
        ] {
            assert_eq!(
                classify_line(line),
                LineKind::SectionHeader(kind, line.into()),
                "expected {line:?} to be a section header"
            );
        }
    }

    /// A bracketed tag opens a section; bracketed text that is not a tag stays
    /// an ad-lib, and parenthetical ad-libs are still kept as lyric.
    #[test]
    fn bracketed_ad_libs_stay_lyric() {
        for line in [
            "[oh]",
            "(Hysterical and)",
            "(One day I am gonna grow wings)",
        ] {
            assert_eq!(kind_name(line), "ambiguous", "{line:?}");
        }
        assert_eq!(kind_name("[Am]Hello"), "lyric");
    }

    /// Regression: a chord-only line with no lyric under it in its section stays
    /// chord-only (documented limitation in `parse`).
    #[test]
    fn orphan_intro_chords_stay_chord_only() {
        let song = parse("[Intro]\nAm  F  C  G\n\n[Verse]\n[Am]Hello");
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::ChordOnly("Am  F  C  G".into())]
        );
        assert_eq!(
            song.sections[1].lines,
            vec![LineKind::Lyric("[Am]Hello".into())]
        );
    }

    #[test]
    fn chord_definition_lines_are_noise() {
        for line in [
            "F#m:   244222",
            "Gmaj7: 320002",
            "Bm/A:  x04432",
            "A: x02220",
            "E: 022100 A: x02220",
            "Cadd9: x32030",
        ] {
            assert_eq!(
                kind_name(line),
                "noise",
                "expected {line:?} to be noise, got {:?}",
                classify_line(line)
            );
        }
    }

    /// Chord definitions are noise, so they never leak into the song body.
    #[test]
    fn chord_definitions_are_dropped_from_the_body() {
        let song = parse("[Chords]\nF#m:   244222\nGmaj7: 320002\n\n[Verse]\n[Am]Hello");
        assert_eq!(song.sections.len(), 1);
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::Lyric("[Am]Hello".into())]
        );
    }

    #[test]
    fn standalone_content_labels_are_noise() {
        for line in ["Chords:", "Note:", "Notes:", "Tab:", "Strumming pattern:"] {
            assert_eq!(kind_name(line), "noise", "expected {line:?} to be noise");
        }
    }

    #[test]
    fn extra_metadata_labels_are_metadata_not_lyric() {
        for line in [
            "Album: Demo Record",
            "Written by A. Writer",
            "Transcribed by B. Scribe",
            "Music by C. Composer",
            "Lyrics by D. Poet",
        ] {
            assert_eq!(kind_name(line), "noise", "expected {line:?} to be noise");
        }
        let song = parse("Album: Demo Record\nWritten by A. Writer\n\n[Verse]\n[Am]Hello");
        assert_eq!(song.title, None);
        assert_eq!(song.artist, None);
        assert_eq!(song.sections.len(), 1);
    }

    #[test]
    fn labelled_metadata_fields_still_win() {
        let song = extract_metadata(
            "Song: Demo\nArtist: Demo Band\nAlbum: Demo Record\nKey: G\nCapo: 3rd fret\nTuning: D A D G B E\n",
        );
        assert_eq!(song.title.as_deref(), Some("Demo"));
        assert_eq!(song.artist.as_deref(), Some("Demo Band"));
        assert_eq!(song.key.as_deref(), Some("G"));
        assert_eq!(song.capo.as_deref(), Some("3"));
        assert_eq!(song.tuning.as_deref(), Some("D A D G B E"));
    }

    /// Scraped tabs lose the newline between labelled lines; the glued label
    /// starts a new field and the glue stays in the previous value.
    #[test]
    fn run_together_metadata_lines_are_split() {
        let song = extract_metadata("Tuning: E A D G B EKey: ACapo: No capo\n");
        assert_eq!(song.tuning.as_deref(), Some("E A D G B E"));
        assert_eq!(song.key.as_deref(), Some("A"));
        assert_eq!(song.capo, None);

        let song = extract_metadata("Song: Demo TitleArtist: Demo Artist\n");
        assert_eq!(song.title.as_deref(), Some("Demo Title"));
        assert_eq!(song.artist.as_deref(), Some("Demo Artist"));

        let song = extract_metadata("Tuning: E A D G B EKey: BmCapo: No capo\n");
        assert_eq!(song.tuning.as_deref(), Some("E A D G B E"));
        assert_eq!(song.key.as_deref(), Some("Bm"));
        assert_eq!(song.capo, None);
    }

    #[test]
    fn run_together_labels_count_as_labelled() {
        let provenance = metadata_provenance("Song: Demo TitleArtist: Demo Artist\n[Verse]\nHello");
        assert!(provenance.title_labelled);
        assert!(provenance.artist_labelled);
    }

    /// A single bare label word without a delimiter is a lyric, not metadata.
    #[test]
    fn bare_label_words_are_not_metadata() {
        assert_eq!(kind_name("Key to the highway"), "ambiguous");
        assert_eq!(kind_name("Band of horses played"), "ambiguous");
    }

    #[test]
    fn decorated_chord_lines_are_chord_only() {
        for line in [
            "Em         Em*  x12",
            "Am Bm Em        x4",
            "Em*",
            "G  2x",
            "Em  Em*",
        ] {
            assert_eq!(
                kind_name(line),
                "chord-only",
                "expected {line:?} to be chord-only"
            );
        }
    }

    /// A bare repeat count is an annotation: never a chord-only line, never
    /// a lyric.
    #[test]
    fn standalone_repeat_counts_are_noise() {
        for line in ["x4", "2x", "x12", "12x"] {
            assert_eq!(kind_name(line), "noise", "expected {line:?} to be noise");
        }
    }

    /// Decorated chords merge at their own column and repeat counts leave the
    /// surrounding columns alone.
    #[test]
    fn decorated_chords_merge_at_their_columns() {
        for (chords, lyric, expected) in [
            (
                "Em         Em*  x12",
                "lyric line here",
                "[Em]lyric line [Em]here",
            ),
            ("Am Bm Em        x4", "xy zw uv", "[Am]xy [Bm]zw [Em]uv"),
            ("Am  2x  F", "hello world foo", "[Am]hello wo[F]rld foo"),
            ("Em*", "abc", "[Em]abc"),
        ] {
            let raw = format!("[Verse]\n{chords}\n{lyric}\n");
            let song = parse(&raw);
            assert_eq!(
                song.sections[0].lines[0],
                LineKind::Lyric(expected.into()),
                "merging {chords:?} over {lyric:?}"
            );
        }
    }

    /// Inference skips noise, headers, chord definitions, chord-only lines,
    /// metadata and wrapped prose: a note paragraph is never the title.
    #[test]
    fn title_inference_skips_prose_notes() {
        let raw = "\
Tuning: E A D G B EKey: DCapo: No capo

The strumming pattern written out here is only a suggestion for beginners.
[Intro]
D  A

[Verse 1]
[D]Alpha beta gamma
";
        let song = parse(raw);
        assert_eq!(song.title.as_deref(), Some("Alpha beta gamma"));
        assert_eq!(song.artist, None);
        assert_eq!(song.key.as_deref(), Some("D"));
        assert_eq!(song.tuning.as_deref(), Some("E A D G B E"));
        // The note paragraph is consumed, and the intro chord with no lyric
        // under it stays chord-only (known limitation).
        assert_eq!(song.sections.len(), 2);
        assert_eq!(
            song.sections[0].lines,
            vec![LineKind::ChordOnly("D  A".into())]
        );
        assert_eq!(
            song.sections[1].lines,
            vec![LineKind::Lyric("[D]Alpha beta gamma".into())]
        );
    }

    /// Regression: with no skipped prose paragraph, the body is not mined for
    /// a title, so a clean tab still needs no guesses.
    #[test]
    fn no_title_guess_without_prose() {
        assert_eq!(parse("[Verse]\n[Am]Hello there").title, None);
    }

    /// Regression table: every line the earlier tests classified must keep
    /// its old kind.
    #[test]
    fn regression_preexisting_inputs_classify_as_before() {
        for (line, expected) in [
            // classifies_section_headers / section_kind_from_header
            ("[Verse 1]", "section header"),
            ("[Chorus]", "section header"),
            ("[Pre-Chorus]", "section header"),
            ("*Bridge*", "section header"),
            ("Intro:", "section header"),
            ("Guitar Solo", "section header"),
            ("[Riff]", "section header"),
            // classifies_fretboard
            ("e|---3---3---|", "fretboard"),
            ("B|-1---0---|", "fretboard"),
            ("E|----------- ------3-|", "fretboard"),
            // classifies_chord_only
            ("Am  Fmaj7  C/G", "chord-only"),
            ("Am  F  C  G  2x", "chord-only"),
            ("| Am | F | C |", "chord-only"),
            ("(Am)  F,  C  2x", "chord-only"),
            ("C G Am F", "chord-only"),
            // classifies_noise
            ("Tabbed by: someone", "noise"),
            ("https://www.ultimate-guitar.com/tab/x", "noise"),
            ("Submit corrections", "noise"),
            ("Upgrade to Pro", "noise"),
            ("Go ad-free with Ultimate Guitar", "noise"),
            ("4.5 stars out of 5", "noise"),
            ("★★★★☆", "noise"),
            ("Difficulty: Expert", "noise"),
            ("Author: someone", "noise"),
            ("Tuning: Standard (E A D G B E)", "noise"),
            ("Standard (E A D G B E)", "noise"),
            ("Drop D", "noise"),
            ("E A D G B E", "noise"),
            ("---", "noise"),
            ("...", "noise"),
            ("Chords used:", "noise"),
            // classifies_inline_chord_lyrics / plain_text_is_ambiguous
            ("[Am]Let it [F]be, let it [C]be", "lyric"),
            ("[Hm]Not really a chord", "ambiguous"),
            ("some line the classifier won't know", "ambiguous"),
            // preamble candidates and bare body lines stay ambiguous
            ("Let It Be", "ambiguous"),
            ("The Beatles", "ambiguous"),
            ("Hey Jude", "ambiguous"),
            ("Na na na", "ambiguous"),
            ("Hello", "ambiguous"),
            ("Some Bare Line", "ambiguous"),
            ("mystery one", "ambiguous"),
            ("mystery line", "ambiguous"),
            ("Another lyric line", "ambiguous"),
            ("", "blank"),
        ] {
            assert_eq!(kind_name(line), expected, "regression on {line:?}");
        }
    }

    /// Contextual classification must agree with the song `parse` builds: same
    /// number of raw lines, and the same ambiguous lines (text and order) kept
    /// in the body. Chord-followed text is `Lyric` in both views.
    #[test]
    fn contextual_classification_agrees_with_parse() {
        let fixtures = [
            "Song: Context Example\nArtist: Example Band\n\n[Verse]\nC       G\nfirst generic line\nAm      F\nsecond generic line\nodd line with no chord above\nDm      G\nthird generic line",
            "[Verse]\n[Am]An inline chord line\nplain ambiguous line\nanother plain line",
            "Title: Demo\nArtist: Demo Band\n[Intro]\nC  G\n\n[Verse]\nDm      F\nfirst generic line\n\nC       G\nsecond generic line",
        ];
        for raw in fixtures {
            let classified = classify_lines(raw);
            assert_eq!(
                classified.len(),
                raw.lines().count(),
                "one classification per raw line for:\n{raw}"
            );

            let classified_ambiguous: Vec<&str> = classified
                .iter()
                .filter(|line| !line.preamble)
                .filter_map(|line| match &line.kind {
                    LineKind::Ambiguous(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();

            let song = parse(raw);
            let parsed_ambiguous: Vec<&str> = song
                .sections
                .iter()
                .flat_map(|section| section.lines.iter())
                .filter_map(|line| match line {
                    LineKind::Ambiguous(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();

            assert_eq!(
                classified_ambiguous, parsed_ambiguous,
                "contextual and parsed ambiguity diverged for:\n{raw}"
            );
        }
    }

    /// The two bracketed-tag classifications that changed: `[Note]`
    /// and `[Chords]` were ambiguous before and are section headers now.
    #[test]
    fn classification_changes_are_bracketed_tags_only() {
        for line in ["[Note]", "[Chords]", "[Tab]", "[Notes]"] {
            assert_eq!(kind_name(line), "section header");
        }
        for line in ["Bm/A:  x04432", "F#m:   244222", "Chords:", "Album: X"] {
            assert_eq!(kind_name(line), "noise");
        }
    }
}
