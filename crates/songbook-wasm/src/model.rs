use serde::Serialize;
use songbook_core::parser::song::Song;
use songbook_core::preview::{self, RowStyle};
use songbook_core::tablature::{TabSplitBounds, TabSplitPreview, TabSplitRow};
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
    /// Stable one-based input location, shared by all generated descendants.
    pub source_line: usize,
    /// Stable role and association for tab rows; null on ordinary lines.
    pub tab_role: Option<String>,
    pub tab_origin: Option<usize>,
    pub tab_instance: Option<usize>,
    pub original: String,
    pub text: String,
    /// The parser's kind label for the raw line.
    pub classifier: String,
    /// What the line is now (`ReviewLine::effective_label`): the parser's label
    /// while pending or preamble-consumed, the effective outcome once decided.
    /// The label slot's single source of truth.
    pub effective_label: String,
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
    /// Core warnings for the current decisions and net session transposition.
    pub warnings: Vec<String>,
    /// True when any literal tab rows are retained, even if export is blocked.
    pub has_tablature: bool,
    /// Core eligibility for the selected block; omitted when it cannot be split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_split_bounds: Option<TabSplitBoundsSnapshot>,
    pub metadata: MetadataSnapshot,
    pub lines: Vec<LineSnapshot>,
}

/// Read-only core bounds. Review indices are zero-based (`end` exclusive),
/// source lines are one-based, and origin/instance IDs are opaque song-local IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSplitBoundsSnapshot {
    pub start: usize,
    pub end: usize,
    pub annotation_count: usize,
    pub source_line: usize,
    pub origin: usize,
    pub instance: usize,
    pub prefix_columns: usize,
    pub max_row_columns: usize,
    pub min_after_column: usize,
    pub max_after_column: usize,
}

impl From<&TabSplitBounds> for TabSplitBoundsSnapshot {
    fn from(bounds: &TabSplitBounds) -> Self {
        Self {
            start: bounds.start,
            end: bounds.end,
            annotation_count: bounds.annotation_count,
            source_line: bounds.source_line,
            origin: bounds.origin,
            instance: bounds.instance,
            prefix_columns: bounds.prefix_columns,
            max_row_columns: bounds.max_row_columns,
            min_after_column: bounds.min_after_column,
            max_after_column: bounds.max_after_column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSplitRowSnapshot {
    pub source_line: usize,
    pub original_row: usize,
    pub original_columns: usize,
    pub prefix: String,
    pub padding_columns: usize,
    pub left: String,
    pub left_columns: usize,
    pub right: String,
    pub right_columns: usize,
    pub right_label_only: bool,
}

impl From<&TabSplitRow> for TabSplitRowSnapshot {
    fn from(row: &TabSplitRow) -> Self {
        Self {
            source_line: row.source_line,
            original_row: row.original_row,
            original_columns: row.original_columns,
            prefix: row.prefix.clone(),
            padding_columns: row.padding_columns,
            left: row.left.clone(),
            left_columns: row.left_columns,
            right: row.right.clone(),
            right_columns: row.right_columns,
            right_label_only: row.right_label_only,
        }
    }
}

/// Display data only: no validation state or executable plan crosses into JS.
/// Apply uses the privately stored core plan, never these serialized rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSplitPreviewSnapshot {
    pub bounds: TabSplitBoundsSnapshot,
    pub after_column: usize,
    pub annotations: Vec<TabSplitRowSnapshot>,
    pub rows: [TabSplitRowSnapshot; 6],
    pub warnings: Vec<String>,
}

impl From<&TabSplitPreview> for TabSplitPreviewSnapshot {
    fn from(preview: &TabSplitPreview) -> Self {
        Self {
            bounds: (&preview.bounds).into(),
            after_column: preview.after_column,
            annotations: preview.annotations.iter().map(Into::into).collect(),
            rows: std::array::from_fn(|index| (&preview.rows[index]).into()),
            warnings: preview.warnings.clone(),
        }
    }
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
    let mut song = state.finish();
    songbook_core::transpose::transpose_song(&mut song, transpose);
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
        transpose: transpose.saturating_add(state.transpose),
        warnings: songbook_core::tablature::warnings(&song),
        has_tablature: songbook_core::tablature::has_tablature(&song),
        tab_split_bounds: state.tab_split_bounds().ok().as_ref().map(Into::into),
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
        source_line: line.source_line(),
        tab_role: line
            .tab_provenance()
            .map(|provenance| match provenance.role() {
                songbook_core::tablature::TabRowRole::Annotation => "annotation".to_string(),
                songbook_core::tablature::TabRowRole::String => "string".to_string(),
            }),
        tab_origin: line.tab_provenance().map(|provenance| provenance.origin()),
        tab_instance: line
            .tab_provenance()
            .map(|provenance| provenance.instance()),
        original: line.original.clone(),
        text: line.text().to_string(),
        classifier: line_kind_label(&line.kind).to_string(),
        effective_label: line.effective_label().to_string(),
        preamble: line.preamble,
        flag: line.is_flag(),
        verdict: verdict_name(line.verdict).to_string(),
        status: status_name(line).to_string(),
        edited: line.edited.is_some(),
    }
}

pub(crate) fn preview_rows(song: &Song, metadata: &Metadata) -> Vec<PreviewRowSnapshot> {
    let mut annotations: Vec<String> = song
        .sections
        .iter()
        .flat_map(|section| section.lines.iter())
        .filter_map(|line| match line {
            songbook_core::parser::song::LineKind::TabAnnotation(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    preview::build(song, metadata)
        .into_iter()
        .map(|row| {
            let style = if row.style == RowStyle::Tablature {
                if let Some(index) = annotations.iter().position(|text| text == &row.text) {
                    annotations.remove(index);
                    "tab-annotation"
                } else {
                    "tablature"
                }
            } else {
                style_name(row.style)
            };
            PreviewRowSnapshot {
                text: row.text,
                style: style.to_string(),
            }
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
    } else if line.is_dropped() {
        // Keep on a blank/noise-kind line exports nothing (effective_kind is
        // None): the honest status is dropped, not kept.
        "dropped"
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
        RowStyle::Tablature => "tablature",
        RowStyle::Note => "note",
        RowStyle::Blank => "blank",
    }
}
