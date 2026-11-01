use crate::parser::chord::{chord_tokens, is_chord};
use crate::parser::song::{LineKind, Section, SectionKind, Song};
use anyhow::Result;
use regex::Regex;
use std::sync::OnceLock;

const PREAMBLE_TEMPLATE: &str = include_str!("../../templates/preamble.tex");

static LATEX_SPECIAL_RE: OnceLock<Regex> = OnceLock::new();

fn latex_special_re() -> &'static Regex {
    LATEX_SPECIAL_RE.get_or_init(|| Regex::new(r"([&%$#_{}~^\\])").unwrap())
}

/// Escapes LaTeX special characters in lyric text.
fn escape(text: &str) -> String {
    latex_special_re().replace_all(text, "\\$1").into_owned()
}

/// Renders inline `[Chord]` markers as songs-package `\[Chord]` macros and
/// escapes the surrounding lyric text.
fn render_lyric(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        if let Some(close_rel) = rest[open..].find(']') {
            let close = open + close_rel;
            let chord = &rest[open + 1..close];
            if crate::parser::chord::is_chord(chord) {
                out.push_str(&escape(&rest[..open]));
                out.push_str("\\[");
                out.push_str(chord);
                out.push(']');
                rest = &rest[close + 1..];
                continue;
            }
        }
        // Not a chord marker; treat the bracket as literal text up to next char.
        out.push_str(&escape(&rest[..open + 1]));
        rest = &rest[open + 1..];
    }
    out.push_str(&escape(rest));
    out
}

fn begin_section(kind: &SectionKind) -> &'static str {
    match kind {
        SectionKind::Verse => "\\beginverse",
        SectionKind::Chorus => "\\beginchorus",
        _ => "\\beginverse*",
    }
}

/// True when a tuning string denotes standard E A D G B E tuning. The
/// comparison is tolerant of case, whitespace, separators, and a leading
/// "standard"/"tuning" word.
pub(crate) fn is_standard_tuning(tuning: &str) -> bool {
    let cleaned = tuning
        .to_lowercase()
        .replace("standard", " ")
        .replace("tuning", " ");
    let Some(notes) = note_tokens(&cleaned) else {
        return false;
    };
    notes.is_empty()
        || notes
            == [
                "e".to_string(),
                "a".to_string(),
                "d".to_string(),
                "g".to_string(),
                "b".to_string(),
                "e".to_string(),
            ]
}

fn note_tokens(value: &str) -> Option<Vec<String>> {
    let mut notes = Vec::new();
    for chunk in value.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, '-' | ',' | '/' | '(' | ')' | '[' | ']')
    }) {
        if chunk.is_empty() {
            continue;
        }
        let chars: Vec<char> = chunk.chars().collect();
        if chars.len() == 6 && chars.iter().all(|ch| is_note_letter(*ch)) {
            notes.extend(chars.iter().map(|ch| ch.to_ascii_lowercase().to_string()));
            continue;
        }
        let mut index = 0;
        while index < chars.len() {
            let ch = chars[index];
            if !is_note_letter(ch) {
                return None;
            }
            let mut token = ch.to_ascii_lowercase().to_string();
            if let Some(next) = chars.get(index + 1) {
                if is_accidental(*next) {
                    token.push(*next);
                    index += 1;
                }
            }
            notes.push(token);
            index += 1;
        }
    }
    Some(notes)
}

fn is_note_letter(ch: char) -> bool {
    ('a'..='g').contains(&ch.to_ascii_lowercase())
}

fn is_accidental(ch: char) -> bool {
    matches!(ch, '#' | 'b' | '♯' | '♭')
}

fn is_repeat_token(token: &str) -> bool {
    let lower = token.to_lowercase();
    let digits = lower
        .strip_suffix('x')
        .or_else(|| lower.strip_prefix('x'))
        .unwrap_or("");
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// Renders a chord-only line as bare inline chord macros. Repeat counts such as
/// `2x` are emitted as a separate textual note because they are not chords.
fn render_chord_line(line: &str) -> Vec<String> {
    let mut chords = Vec::new();
    let mut repeats = Vec::new();
    for token in chord_tokens(line) {
        if is_chord(&token.text) {
            chords.push(format!("\\[{}]", token.text));
        } else if is_repeat_token(&token.text) {
            repeats.push(token.text);
        }
    }
    let mut rendered = Vec::new();
    if !chords.is_empty() {
        rendered.push(chords.join(" "));
    }
    if !repeats.is_empty() {
        rendered.push(format!("\\textnote{{{}}}", escape(&repeats.join(" "))));
    }
    rendered
}

fn normalize_for_comparison(text: &str) -> String {
    text.chars()
        .filter(|ch| {
            !ch.is_whitespace() && !matches!(ch, ',' | '.' | ';' | ':' | '!' | '?' | '\'' | '"')
        })
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn chorus_signature(section: &Section) -> Option<String> {
    if section.kind != SectionKind::Chorus {
        return None;
    }
    let mut signature = String::new();
    let mut has_content = false;
    for line in &section.lines {
        match line {
            LineKind::Lyric(text) | LineKind::ChordOnly(text) => {
                signature.push_str(&normalize_for_comparison(text));
                signature.push('\n');
                has_content = true;
            }
            _ => {}
        }
    }
    has_content.then_some(signature)
}

/// Renders a parsed song into songs-package LaTeX.
pub fn render(song: &Song, standalone: bool) -> Result<String> {
    let mut body = String::new();

    body.push_str("\\beginsong{");
    body.push_str(&escape(song.title.as_deref().unwrap_or("Untitled")));
    body.push('}');
    if let Some(artist) = &song.artist {
        body.push_str(&format!("[by={{{}}}]", escape(artist)));
    }
    body.push('\n');

    if let Some(tuning) = &song.tuning {
        if !is_standard_tuning(tuning) {
            body.push_str(&format!("\\textnote{{Tuning: {}}}\n", escape(tuning)));
        }
    }
    if let Some(key) = &song.key {
        body.push_str(&format!("\\textnote{{Key: {}}}\n", escape(key)));
    }
    if let Some(capo) = &song.capo {
        if capo.chars().all(|ch| ch.is_ascii_digit()) {
            body.push_str(&format!("\\capo{{{}}}\n", escape(capo)));
        } else {
            body.push_str(&format!("\\textnote{{Capo: {}}}\n", escape(capo)));
        }
    }

    let mut previous_chorus: Option<String> = None;
    let mut repeat_noted = false;
    for section in &song.sections {
        let signature = chorus_signature(section);
        if let Some(signature) = &signature {
            if previous_chorus.as_deref() == Some(signature) {
                if !repeat_noted {
                    body.push_str("\\textnote{Chorus repeats}\n");
                    repeat_noted = true;
                }
                continue;
            }
            previous_chorus = Some(signature.clone());
            repeat_noted = false;
        } else {
            previous_chorus = None;
            repeat_noted = false;
        }

        let mut rendered = Vec::new();
        for line in &section.lines {
            match line {
                LineKind::Lyric(text) => rendered.push(render_lyric(text)),
                LineKind::ChordOnly(text) => rendered.extend(render_chord_line(text)),
                // Fretboard lines remain parser-only until gtab rendering is
                // designed; triage may promote them later.
                _ => {}
            }
        }
        if rendered.is_empty() {
            continue;
        }
        body.push_str(begin_section(&section.kind));
        body.push('\n');
        for line in rendered {
            body.push_str(&line);
            body.push('\n');
        }
        match section.kind {
            SectionKind::Verse => body.push_str("\\endverse\n"),
            SectionKind::Chorus => body.push_str("\\endchorus\n"),
            _ => body.push_str("\\endverse*\n"),
        }
    }

    body.push_str("\\endsong\n");

    if standalone {
        Ok(PREAMBLE_TEMPLATE.replace("{{SONGS}}", &body))
    } else {
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::classify::parse;

    #[test]
    fn renders_inline_chords() {
        let song = parse("[Am]Let it [F]be, let it [C]be");
        let tex = render(&song, false).unwrap();
        assert!(tex.contains("\\[Am]Let it \\[F]be, let it \\[C]be"));
    }

    #[test]
    fn escapes_specials() {
        assert_eq!(render_lyric("100% of R&D & fun"), "100\\% of R\\&D \\& fun");
    }

    #[test]
    fn standalone_wraps_document() {
        let song = parse("[Verse 1]\n[Am]Test");
        let tex = render(&song, true).unwrap();
        assert!(tex.contains("\\usepackage[chorded]{songs}"));
        assert!(tex.contains("\\beginchorus") || tex.contains("\\beginverse"));
        assert!(tex.starts_with("\\documentclass"));
    }

    #[test]
    fn standard_tuning_comparison_is_tolerant() {
        for tuning in [
            "E A D G B E",
            "e a d g b e",
            "EADGBE",
            "E-A-D-G-B-E",
            "Standard",
            "standard tuning",
            "Standard (E A D G B E)",
        ] {
            assert!(is_standard_tuning(tuning), "{tuning} should be standard");
        }
        for tuning in [
            "D A D G A D",
            "DADGAD",
            "Drop D (D A D G B E)",
            "Eb Ab Db Gb Bb Eb",
            "Open G",
        ] {
            assert!(
                !is_standard_tuning(tuning),
                "{tuning} should not be standard"
            );
        }
    }

    #[test]
    fn renders_metadata_using_supported_songs_macros() {
        let song = parse("Song: X\nArtist: Y\nKey: D\nCapo: 2\n[Verse]\n[Am]Hello");
        let tex = render(&song, false).unwrap();
        assert!(tex.contains("\\beginsong{X}[by={Y}]"));
        assert!(tex.contains("\\textnote{Key: D}"));
        assert!(tex.contains("\\capo{2}"));
        assert!(!tex.contains("key="));
        assert!(!tex.contains("capo="));
    }

    #[test]
    fn renders_non_standard_tuning_only() {
        let song = parse("Song: X\nTuning: D A D G A D\n[Verse]\n[Am]Hello");
        let tex = render(&song, false).unwrap();
        assert!(tex.contains("\\textnote{Tuning: D A D G A D}"));

        let standard = parse("Song: X\nTuning: Standard (E A D G B E)\n[Verse]\n[Am]Hello");
        let tex = render(&standard, false).unwrap();
        assert!(!tex.contains("Tuning:"));
    }

    #[test]
    fn promotes_chord_only_lines() {
        let song = parse("[Intro]\nAm  F  C  G  2x\n\n[Verse]\n[Am]Hello");
        let tex = render(&song, false).unwrap();
        assert!(tex.contains("\\beginverse*\n\\[Am] \\[F] \\[C] \\[G]\n"));
        assert!(tex.contains("\\textnote{2x}"));
        assert!(tex.contains("\\[Am]Hello"));
    }

    #[test]
    fn collapses_consecutive_identical_choruses() {
        let raw = "\
[Verse]
[Am]Verse line

[Chorus]
[F]Same chorus line

[Chorus]
[F]Same chorus line!";
        let song = parse(raw);
        let tex = render(&song, false).unwrap();
        assert_eq!(tex.matches("\\beginchorus").count(), 1);
        assert_eq!(tex.matches("\\textnote{Chorus repeats}").count(), 1);
        assert!(tex.contains("\\[F]Same chorus line"));
    }

    #[test]
    fn keeps_consecutive_choruses_with_different_lyrics() {
        let raw = "\
[Chorus]
[F]First chorus

[Chorus]
[F]Second chorus";
        let song = parse(raw);
        let tex = render(&song, false).unwrap();
        assert_eq!(tex.matches("\\beginchorus").count(), 2);
        assert!(!tex.contains("Chorus repeats"));
    }
}
