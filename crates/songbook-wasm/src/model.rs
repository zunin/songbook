use serde::Serialize;
use songbook_core::parser::song::Song;
use songbook_core::preview::{self, RowStyle};
use songbook_core::triage::{line_kind_label, Metadata, Phase, ReviewLine, TriageState, Verdict};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataFieldSnapshot {
    pub index: usize,
    pub label: String,
    pub value: String,
    pub inferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSnapshot {
    pub title: String,
    pub title_inferred: bool,
    pub artist: String,
    pub artist_inferred: bool,
    pub key: String,
    pub capo: String,
    pub tuning: String,
    pub fields: Vec<MetadataFieldSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSnapshot {
    pub index: usize,
    pub original: String,
    pub text: String,
    pub classifier: String,
    pub preamble: bool,
    pub flag: bool,
    pub verdict: String,
    pub status: String,
    pub edited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub phase: String,
    pub position: usize,
    pub line_count: usize,
    pub flag_count: usize,
    pub pending_flags: usize,
    pub flags: Vec<usize>,
    pub editing: bool,
    pub edit_buffer: String,
    pub message: Option<String>,
    pub metadata_field: usize,
    pub preview_scroll: usize,
    /// Net transposition offset in semitones applied to the triaged song.
    pub transpose: i32,
    pub metadata: MetadataSnapshot,
    pub lines: Vec<LineSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRowSnapshot {
    pub text: String,
    pub style: String,
}

/// One triaged song's contribution to a book (ADR0019): the non-standalone
/// LaTeX body — exactly what `export_tex(false)` returns — plus the song's
/// distinct chords as core's book composer expects them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookPartSnapshot {
    pub body: String,
    pub chords: Vec<String>,
}

pub(crate) fn snapshot(state: &TriageState, transpose: i32) -> SessionSnapshot {
    SessionSnapshot {
        phase: phase_name(state.phase).to_string(),
        position: state.position,
        line_count: state.lines.len(),
        flag_count: state.flags.len(),
        pending_flags: state.pending_flags(),
        flags: state.flags.clone(),
        editing: state.editing,
        edit_buffer: state.edit_buffer.clone(),
        message: state.message.clone(),
        metadata_field: state.metadata_field,
        preview_scroll: state.preview_scroll,
        transpose,
        metadata: metadata_snapshot(&state.metadata),
        lines: state
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| line_snapshot(index, line))
            .collect(),
    }
}

fn metadata_snapshot(metadata: &Metadata) -> MetadataSnapshot {
    let fields = (0..metadata.field_count())
        .map(|index| MetadataFieldSnapshot {
            index,
            label: metadata.field_label(index).to_string(),
            value: metadata.field(index).to_string(),
            inferred: metadata.field_inferred(index),
        })
        .collect();
    MetadataSnapshot {
        title: metadata.title.clone(),
        title_inferred: metadata.title_inferred,
        artist: metadata.artist.clone(),
        artist_inferred: metadata.artist_inferred,
        key: metadata.key.clone(),
        capo: metadata.capo.clone(),
        tuning: metadata.tuning.clone(),
        fields,
    }
}

fn line_snapshot(index: usize, line: &ReviewLine) -> LineSnapshot {
    LineSnapshot {
        index,
        original: line.original.clone(),
        text: line.text().to_string(),
        classifier: line_kind_label(&line.kind).to_string(),
        preamble: line.preamble,
        flag: line.is_flag(),
        verdict: verdict_name(line.verdict).to_string(),
        status: status_name(line).to_string(),
        edited: line.edited.is_some(),
    }
}

pub(crate) fn preview_rows(song: &Song, metadata: &Metadata) -> Vec<PreviewRowSnapshot> {
    preview::build(song, metadata)
        .into_iter()
        .map(|row| PreviewRowSnapshot {
            text: row.text,
            style: style_name(row.style).to_string(),
        })
        .collect()
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Metadata => "metadata",
        Phase::Triage => "triage",
        Phase::Preview => "preview",
        Phase::Done => "done",
        Phase::Quit => "quit",
    }
}

fn verdict_name(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pending => "pending",
        Verdict::Keep => "keep",
        Verdict::Discard => "discard",
        Verdict::Lyric => "lyric",
        Verdict::ChordOnly => "chord-only",
        Verdict::SectionHeader => "section-header",
        Verdict::Noise => "noise",
    }
}

fn status_name(line: &ReviewLine) -> &'static str {
    if line.verdict == Verdict::Pending {
        "pending"
    } else if matches!(line.verdict, Verdict::Discard | Verdict::Noise) {
        "dropped"
    } else if line.preamble && line.verdict == Verdict::Keep {
        "preamble"
    } else if line.edited.is_some() {
        "edited"
    } else {
        "kept"
    }
}

fn style_name(style: RowStyle) -> &'static str {
    match style {
        RowStyle::Header => "header",
        RowStyle::Meta => "meta",
        RowStyle::Section => "section",
        RowStyle::Lyric => "lyric",
        RowStyle::Chord => "chord",
        RowStyle::Note => "note",
        RowStyle::Blank => "blank",
    }
}
