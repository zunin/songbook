use crate::latex::render::chorus_signature;
use crate::parser::song::{LineKind, Song};
use crate::triage::{section_label, Metadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStyle {
    Header,
    Meta,
    Section,
    Lyric,
    Chord,
    Tablature,
    Note,
    Blank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRow {
    pub text: String,
    pub style: RowStyle,
}

impl PreviewRow {
    fn new(style: RowStyle, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    fn header(text: impl Into<String>) -> Self {
        Self::new(RowStyle::Header, text)
    }

    fn meta(text: impl Into<String>) -> Self {
        Self::new(RowStyle::Meta, text)
    }

    fn section(text: impl Into<String>) -> Self {
        Self::new(RowStyle::Section, text)
    }

    fn lyric(text: impl Into<String>) -> Self {
        Self::new(RowStyle::Lyric, text)
    }

    fn chord(text: impl Into<String>) -> Self {
        Self::new(RowStyle::Chord, text)
    }

    fn note(text: impl Into<String>) -> Self {
        Self::new(RowStyle::Note, text)
    }

    fn blank() -> Self {
        Self::new(RowStyle::Blank, "")
    }
}

/// Builds the read-only structure preview shown before export. It mirrors the
/// renderer's section-level decisions (renderable lines only, repeated choruses
/// collapsed) without emulating LaTeX or a PDF page.
pub fn build(song: &Song, metadata: &Metadata) -> Vec<PreviewRow> {
    let mut rows = Vec::new();

    rows.push(PreviewRow::header("Metadata"));
    for index in 0..metadata.field_count() {
        let value = metadata.field(index).trim();
        let shown = if value.is_empty() { "—" } else { value };
        let inferred = if metadata.field_inferred(index) {
            " (inferred)"
        } else {
            ""
        };
        rows.push(PreviewRow::meta(format!(
            "{:<8} {}{}",
            metadata.field_label(index),
            shown,
            inferred
        )));
    }

    rows.push(PreviewRow::blank());
    rows.push(PreviewRow::header("Sections"));

    let mut previous_chorus: Option<String> = None;
    let mut repeat_noted = false;
    let mut any_section = false;

    for (section_index, section) in song.sections.iter().enumerate() {
        let signature = chorus_signature(song, section_index);
        if let Some(signature) = &signature {
            if previous_chorus.as_deref() == Some(signature) {
                if !repeat_noted {
                    rows.push(PreviewRow::note(
                        "(repeats the previous chorus — renderer emits a repeat note)",
                    ));
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

        let renderable: Vec<&LineKind> = section
            .lines
            .iter()
            .filter(|line| {
                matches!(
                    line,
                    LineKind::Lyric(_)
                        | LineKind::ChordOnly(_)
                        | LineKind::Fretboard(_)
                        | LineKind::TabAnnotation(_)
                )
            })
            .collect();
        if renderable.is_empty() {
            continue;
        }

        any_section = true;
        rows.push(PreviewRow::section(format!(
            "[{}]",
            section_label(&section.kind)
        )));
        for (line_index, line) in section.lines.iter().enumerate() {
            if let Some((index, group)) = song
                .tab_groups
                .iter()
                .enumerate()
                .find(|(_, g)| g.section == section_index && g.annotation_start == line_index)
            {
                let label = if group.start == group.end {
                    "Retained tab annotation group without strings"
                } else if group.original_rows == 6 {
                    "Literal six-string tab block"
                } else {
                    "Unsupported tab run"
                };
                rows.push(PreviewRow::note(format!(
                    "{label} {} · source line {} · {} retained rows ({} originally)",
                    index + 1,
                    group.source_line,
                    group.end - group.start,
                    group.original_rows
                )));
            }
            match line {
                LineKind::Lyric(text) => rows.push(PreviewRow::lyric(format!("  {text}"))),
                LineKind::ChordOnly(text) => rows.push(PreviewRow::chord(format!("  {text}"))),
                LineKind::Fretboard(text) | LineKind::TabAnnotation(text) => {
                    rows.push(PreviewRow::new(RowStyle::Tablature, text))
                }
                _ => {}
            }
        }
        rows.push(PreviewRow::blank());
    }

    if !any_section {
        rows.push(PreviewRow::note("(no renderable sections)"));
    }
    for warning in crate::tablature::warnings(song) {
        rows.push(PreviewRow::note(warning));
    }

    rows
}

pub fn line_count(song: &Song, metadata: &Metadata) -> usize {
    build(song, metadata).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::classify::parse;
    use crate::triage::TriageState;

    #[test]
    fn line_count_matches_built_rows() {
        let raw = "Song: Test\nArtist: Band\n[Verse]\n[Am]Hello";
        let song = parse(raw);
        let state = TriageState::new(song.clone(), raw);
        let rows = build(&song, &state.metadata);
        assert_eq!(line_count(&song, &state.metadata), rows.len());
        assert!(rows.iter().any(|row| row.text == "[Verse]"));
    }

    #[test]
    fn repeated_chorus_is_collapsed_in_preview_model() {
        let raw = "[Chorus]\n[F]Same line\n\n[Chorus]\n[F]Same line";
        let song = parse(raw);
        let state = TriageState::new(song.clone(), raw);
        let rows = build(&song, &state.metadata);
        assert_eq!(rows.iter().filter(|row| row.text == "[Chorus]").count(), 1);
        assert!(rows.iter().any(|row| row.style == RowStyle::Note));
    }
}
