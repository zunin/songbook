use crate::parser::chord::merge_chord_line;
use crate::parser::classify::{classify_lines, metadata_provenance, section_kind_from_header};
use crate::parser::song::{LineKind, Section, SectionKind, Song};
use crate::preview;
use crate::tablature::{
    self, TabProvenance, TabRowRole, TabSplitBounds, TabSplitPreview, TabSplitRow, MAX_COLUMNS,
};
use crate::transpose::transpose_song;
use anyhow::{bail, Result};

const METADATA_FIELD_COUNT: usize = 5;
const PREVIEW_PAGE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Metadata,
    Triage,
    Preview,
    Done,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Classifier flag awaiting review (ambiguous/fretboard).
    Pending,
    /// Use the classifier's kind; preamble lines stay out of the body.
    Keep,
    Discard,
    Lyric,
    ChordOnly,
    SectionHeader,
    Noise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Next,
    Prev,
    NextFlag,
    PrevFlag,
    Confirm,
    Discard,
    Lyric,
    ChordOnly,
    SectionHeader,
    Noise,
    /// Explicitly keep the selected adjacent row as a literal tab annotation
    /// and associate it with the following complete original six-string block.
    TabAnnotation,
    Edit,
    Undo,
    AutoAccept,
    Skip,
    Quit,
    Escape,
    CtrlC,
    Tab,
    BackTab,
    Char(char),
    Backspace,
    ClearField,
    Back,
    PageUp,
    PageDown,
    /// Song-level transposition, delta-based like the web `Session::transpose`.
    TransposeUp,
    TransposeDown,
    TransposeReset,
    PanLeft,
    PanRight,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    pub title: String,
    pub title_inferred: bool,
    pub artist: String,
    pub artist_inferred: bool,
    pub key: String,
    pub capo: String,
    pub tuning: String,
}

impl Metadata {
    fn from_song(song: &Song, title_labelled: bool, artist_labelled: bool) -> Self {
        let title = song.title.clone().unwrap_or_default();
        let artist = song.artist.clone().unwrap_or_default();
        Self {
            title_inferred: !title.is_empty() && !title_labelled,
            artist_inferred: !artist.is_empty() && !artist_labelled,
            title,
            artist,
            key: song.key.clone().unwrap_or_default(),
            capo: song.capo.clone().unwrap_or_default(),
            tuning: song.tuning.clone().unwrap_or_default(),
        }
    }

    pub fn field_count(&self) -> usize {
        METADATA_FIELD_COUNT
    }

    pub fn field(&self, index: usize) -> &str {
        match index {
            0 => &self.title,
            1 => &self.artist,
            2 => &self.key,
            3 => &self.capo,
            _ => &self.tuning,
        }
    }

    pub fn field_mut(&mut self, index: usize) -> &mut String {
        match index {
            0 => &mut self.title,
            1 => &mut self.artist,
            2 => &mut self.key,
            3 => &mut self.capo,
            _ => &mut self.tuning,
        }
    }

    pub fn field_label(&self, index: usize) -> &'static str {
        match index {
            0 => "Title",
            1 => "Artist",
            2 => "Key",
            3 => "Capo",
            _ => "Tuning",
        }
    }

    pub fn field_inferred(&self, index: usize) -> bool {
        match index {
            0 => self.title_inferred,
            1 => self.artist_inferred,
            _ => false,
        }
    }

    fn apply_to(&self, song: &mut Song) {
        song.title = non_empty(&self.title);
        song.artist = non_empty(&self.artist);
        song.key = non_empty(&self.key);
        song.capo = non_empty(&self.capo);
        song.tuning = non_empty(&self.tuning);
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Status text for a net transposition offset: the new offset while active,
/// the reset confirmation at zero.
fn transpose_message(offset: i32) -> String {
    if offset == 0 {
        "Back to the original key.".into()
    } else {
        format!("Transposed {offset:+}.")
    }
}

/// One raw input line plus its review decision (ADR0014).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewLine {
    /// Stable one-based input location, shared by split descendants.
    source_line: usize,
    /// Assigned once at construction; splitting changes only child instance.
    tab: Option<TabProvenance>,
    /// The original input line, exactly as parsed.
    pub original: String,
    /// The classifier's kind for this line.
    pub kind: LineKind,
    /// Consumed by `parse` as preamble (metadata/noise before the body).
    pub preamble: bool,
    /// Review decision; `Keep`/`Pending` defer to `kind` (and `preamble`).
    pub verdict: Verdict,
    /// Replacement text when the line has been edited.
    pub edited: Option<String>,
}

impl ReviewLine {
    pub fn source_line(&self) -> usize {
        self.source_line
    }

    pub fn tab_provenance(&self) -> Option<TabProvenance> {
        self.tab
    }

    /// Text used for review and rebuild (edited text when present).
    pub fn text(&self) -> &str {
        self.edited.as_deref().unwrap_or(&self.original)
    }

    /// The line's contribution to the rebuilt song: `None` means skipped.
    fn effective_kind(&self) -> Option<LineKind> {
        let raw_text = self.text().to_string();
        let text = raw_text.trim().to_string();
        match self.verdict {
            Verdict::Discard | Verdict::Noise => None,
            Verdict::Lyric => Some(LineKind::Lyric(text)),
            Verdict::ChordOnly => Some(LineKind::ChordOnly(raw_text)),
            Verdict::SectionHeader => Some(LineKind::SectionHeader(
                section_kind_from_header(&text),
                text,
            )),
            Verdict::Keep | Verdict::Pending => {
                if self.preamble && !matches!(self.kind, LineKind::TabAnnotation(_)) {
                    return None;
                }
                match &self.kind {
                    LineKind::Blank | LineKind::Noise(_) => None,
                    LineKind::Ambiguous(_) => Some(LineKind::Lyric(text)),
                    LineKind::Lyric(_) => Some(LineKind::Lyric(text)),
                    LineKind::ChordOnly(_) => Some(LineKind::ChordOnly(raw_text)),
                    LineKind::Fretboard(_) => Some(LineKind::Fretboard(raw_text)),
                    LineKind::TabAnnotation(_) => Some(LineKind::TabAnnotation(raw_text)),
                    LineKind::SectionHeader(kind, _) => {
                        Some(LineKind::SectionHeader(kind.clone(), text))
                    }
                }
            }
        }
    }

    /// Whether the line is a classifier flag (needs review, but navigable).
    pub fn is_flag(&self) -> bool {
        !self.preamble && matches!(self.kind, LineKind::Ambiguous(_) | LineKind::Fretboard(_))
    }

    /// Whether the line contributes nothing to the rebuilt song (`effective_kind`
    /// is `None`): discarded, noise-marked, preamble-consumed, or a kept
    /// blank/noise-kind line. The honest "dropped" condition for UI status.
    pub fn is_dropped(&self) -> bool {
        self.effective_kind().is_none()
    }

    /// What the line IS NOW, for the review UI's label slot (UX advisor rule):
    /// pending rows name the parser's guess as-is — nothing is decided yet, so
    /// no effective resolution and no implied change — and preamble keeps keep
    /// the parser's name too (the "preamble" status explains they were
    /// consumed). Decided rows name the effective outcome: explicit picks and
    /// Discard/Noise name their decision, and a kept body line resolves
    /// through the same `effective_kind` semantics the rebuild uses (`None`
    /// means the line is dropped from the body).
    pub fn effective_label(&self) -> &'static str {
        match self.verdict {
            Verdict::Pending => line_kind_label(&self.kind),
            Verdict::Discard => "discarded",
            Verdict::Noise => "noise",
            Verdict::Lyric => "lyric",
            Verdict::ChordOnly => "chord-only",
            Verdict::SectionHeader => "section header",
            Verdict::Keep => {
                if self.preamble {
                    line_kind_label(&self.kind)
                } else {
                    match self.effective_kind() {
                        None => "dropped",
                        Some(kind) => line_kind_label(&kind),
                    }
                }
            }
        }
    }
}

pub fn line_kind_label(kind: &LineKind) -> &'static str {
    match kind {
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

pub fn section_label(kind: &SectionKind) -> String {
    match kind {
        SectionKind::Intro => "Intro".into(),
        SectionKind::Verse => "Verse".into(),
        SectionKind::PreChorus => "Pre-Chorus".into(),
        SectionKind::Chorus => "Chorus".into(),
        SectionKind::Bridge => "Bridge".into(),
        SectionKind::Solo => "Solo".into(),
        SectionKind::Interlude => "Interlude".into(),
        SectionKind::Instrumental => "Instrumental".into(),
        SectionKind::Outro => "Outro".into(),
        SectionKind::Other(name) => name.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    metadata: Metadata,
    metadata_field: usize,
    lines: Vec<ReviewLine>,
    position: usize,
    phase: Phase,
    preview_scroll: usize,
    horizontal_scroll: u16,
}

/// A staged, core-generated split. Its validation state and replacement data
/// are private: UI adapters can inspect the preview but cannot supply rows to
/// apply. Dropping a plan cancels without changing review or undo history.
#[derive(Debug, Clone)]
pub struct TabSplitPlan {
    preview: TabSplitPreview,
    expected: Snapshot,
    revision: u64,
}

impl TabSplitPlan {
    pub fn preview(&self) -> &TabSplitPreview {
        &self.preview
    }
}

#[derive(Debug)]
pub struct TriageState {
    pub metadata: Metadata,
    pub metadata_field: usize,
    pub lines: Vec<ReviewLine>,
    /// Raw indices of classifier flags.
    pub flags: Vec<usize>,
    /// Current line index in `lines` (every raw line is navigable).
    pub position: usize,
    pub phase: Phase,
    pub editing: bool,
    pub edit_buffer: String,
    pub message: Option<String>,
    pub preview_scroll: usize,
    pub horizontal_scroll: u16,
    /// Net transposition in semitones applied on top of the triaged song.
    /// Delta-accumulated (like the web `Session::transpose`) and deliberately
    /// not part of the undo stack; `TransposeReset` is the way back.
    pub transpose: i32,
    undo: Vec<Snapshot>,
    /// Monotonic even across undo; prevents a reverted edit/selection accepting
    /// an old plan. Snapshot comparison also detects direct public-field edits.
    review_revision: u64,
    next_tab_instance: usize,
}

impl TriageState {
    pub fn new(song: Song, raw: &str) -> Self {
        let provenance = metadata_provenance(raw);
        let metadata =
            Metadata::from_song(&song, provenance.title_labelled, provenance.artist_labelled);
        let classified = classify_lines(raw);
        let original: Vec<_> = classified.iter().map(|c| c.kind.clone()).collect();
        let tab_provenance = tablature::source_provenance(&original);
        let lines: Vec<ReviewLine> = raw
            .lines()
            .zip(classified)
            .zip(tab_provenance)
            .enumerate()
            .map(|(index, ((line, classified), tab))| {
                let kind = classified.kind;
                let preamble = classified.preamble;
                let flagged =
                    !preamble && matches!(kind, LineKind::Ambiguous(_) | LineKind::Fretboard(_));
                ReviewLine {
                    source_line: index + 1,
                    tab,
                    original: line.to_string(),
                    kind,
                    preamble,
                    verdict: if flagged {
                        Verdict::Pending
                    } else {
                        Verdict::Keep
                    },
                    edited: None,
                }
            })
            .collect();
        let flags: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.is_flag())
            .map(|(index, _)| index)
            .collect();
        let position = flags.first().copied().unwrap_or(0);
        let next_tab_instance = lines.len();
        Self {
            metadata,
            metadata_field: 0,
            lines,
            flags,
            position,
            phase: Phase::Metadata,
            editing: false,
            edit_buffer: String::new(),
            message: None,
            preview_scroll: 0,
            horizontal_scroll: 0,
            transpose: 0,
            undo: Vec::new(),
            review_revision: 0,
            next_tab_instance,
        }
    }

    pub fn current_line(&self) -> Option<&ReviewLine> {
        self.lines.get(self.position)
    }

    fn current_line_mut(&mut self) -> Option<&mut ReviewLine> {
        self.lines.get_mut(self.position)
    }

    /// Eligibility and numeric bounds for thin review UIs. Errors explain why
    /// this current instance cannot be split, including unworkable prefixes.
    /// Only complete, contiguous original-six instances qualify. Any retained
    /// annotations must form the immediate, complete prefix above the strings.
    pub fn tab_split_bounds(&self) -> Result<TabSplitBounds> {
        if self.phase != Phase::Triage || self.editing {
            bail!(
                "Return to line review and finish or cancel editing before splitting a tab block."
            );
        }
        let Some(source) = self.current_line().and_then(|line| line.tab) else {
            bail!("Select a row in a complete original six-string tab block to split.");
        };
        if source.original_rows() != 6 {
            bail!("Cannot split an original {}-row tab run: only complete original six-string blocks are supported.", source.original_rows());
        }
        let indices: Vec<_> = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                line.tab.is_some_and(|tab| {
                    tab.origin() == source.origin() && tab.instance() == source.instance()
                })
            })
            .map(|(index, _)| index)
            .collect();
        let string_indices: Vec<_> = indices
            .iter()
            .copied()
            .filter(|index| {
                self.lines[*index]
                    .tab
                    .is_some_and(|tab| tab.role() == TabRowRole::String)
            })
            .collect();
        let Some(&string_start) = string_indices.first() else {
            bail!("Cannot split retained annotations without all six string rows; restore the complete block first.");
        };
        if string_indices.len() != 6
            || string_indices
                .iter()
                .enumerate()
                .any(|(row, index)| *index != string_start + row)
        {
            bail!("Cannot split a partial or noncontiguous tab instance; restore all six original string rows first.");
        }
        let annotation_count = indices.len() - 6;
        let start = string_start.checked_sub(annotation_count).ok_or_else(|| {
            anyhow::anyhow!("Cannot split an annotation prefix detached from its six string rows.")
        })?;
        if indices
            .iter()
            .enumerate()
            .any(|(row, index)| *index != start + row)
        {
            bail!("Cannot split a partial or noncontiguous tab instance; annotations must immediately precede all six string rows.");
        }
        let mut prefix_columns = 0;
        let mut max_row_columns = 0;
        let mut seen_strings = [false; 6];
        let mut seen_annotations = vec![false; annotation_count];
        for line in &self.lines[start..start + 6 + annotation_count] {
            let tab = line.tab.expect("instance rows have provenance");
            let ordinal = tab.original_row();
            if tab.original_rows() != 6 || ordinal == 0 {
                bail!("Cannot split a tab instance without valid original row ordinals.");
            }
            match tab.role() {
                TabRowRole::String => {
                    if !(1..=6).contains(&ordinal) || seen_strings[ordinal - 1] {
                        bail!(
                            "Cannot split a tab instance without six unique original row ordinals."
                        );
                    }
                    seen_strings[ordinal - 1] = true;
                    if line.preamble
                        || !matches!(line.effective_kind(), Some(LineKind::Fretboard(_)))
                    {
                        bail!("Cannot split a partial or reclassified tab instance; restore all six rows as fretboard first.");
                    }
                    let Some(pipe) = line.text().find('|') else {
                        bail!("Cannot split source line {}: restore its label and first pipe before splitting.", line.source_line());
                    };
                    prefix_columns = prefix_columns.max(pipe + 1);
                }
                TabRowRole::Annotation => {
                    if ordinal > annotation_count || seen_annotations[ordinal - 1] {
                        bail!(
                            "Cannot split a tab instance without unique annotation-row ordinals."
                        );
                    }
                    seen_annotations[ordinal - 1] = true;
                    if !matches!(line.effective_kind(), Some(LineKind::TabAnnotation(_))) {
                        bail!("Cannot split a partial or reclassified tab instance; restore every associated annotation first.");
                    }
                }
            }
            let text = line.text();
            if !text.bytes().all(|byte| (32..=126).contains(&byte)) {
                bail!("Cannot split source line {}: only printable ASCII is supported. Replace tabs/control or Unicode characters with ordinary ASCII before splitting.", line.source_line());
            }
            max_row_columns = max_row_columns.max(text.len());
        }
        if prefix_columns >= MAX_COLUMNS {
            bail!("Cannot split: longest indentation/label/first-pipe prefix occupies {prefix_columns} columns; it must be shorter than {MAX_COLUMNS}. Shorten indentation or labels first.");
        }
        let min_after_column = prefix_columns + 1;
        let max_after_column = MAX_COLUMNS.min(max_row_columns.saturating_sub(1));
        if min_after_column > max_after_column {
            bail!("Cannot split: no notation remains beyond a valid cut after the {prefix_columns}-column prefix. Edit the block or select a longer instance.");
        }
        Ok(TabSplitBounds {
            start,
            end: start + 6 + annotation_count,
            annotation_count,
            source_line: source.origin() + 1,
            origin: source.origin(),
            instance: source.instance(),
            prefix_columns,
            max_row_columns,
            min_after_column,
            max_after_column,
        })
    }

    /// Stage a split after one-based raw column N: P < N <= 56 and N < the
    /// longest current row. This is literal slicing, not a musical operation.
    /// Planning/preview/cancellation never change review state or undo history.
    pub fn plan_tab_split(&self, after_column: usize) -> Result<TabSplitPlan> {
        let bounds = self.tab_split_bounds()?;
        if !(bounds.min_after_column..=bounds.max_after_column).contains(&after_column) {
            bail!("Split after raw column {} through {} inclusive (including indentation, label and first pipe); requested {after_column}.", bounds.min_after_column, bounds.max_after_column);
        }
        let annotations = self.lines[bounds.start..bounds.start + bounds.annotation_count]
            .iter()
            .map(|line| {
                let text = line.text();
                let left = text[..after_column.min(text.len())].to_string();
                // An annotation has no repeated source-text prefix. Its right
                // suffix starts after the same raw cut using only enough spaces
                // to match the right strings' widest repeated label/pipe prefix.
                let prefix = String::new();
                let padding_columns = bounds.prefix_columns;
                let right = format!(
                    "{}{}",
                    " ".repeat(padding_columns),
                    text.get(after_column..).unwrap_or("")
                );
                TabSplitRow {
                    source_line: line.source_line(),
                    original_row: line
                        .tab
                        .expect("bounds validated annotation provenance")
                        .original_row(),
                    original_columns: text.len(),
                    prefix,
                    padding_columns,
                    left_columns: left.len(),
                    right_columns: right.len(),
                    right_label_only: text.len() <= after_column,
                    left,
                    right,
                }
            })
            .collect::<Vec<_>>();
        let rows = std::array::from_fn(|row| {
            let line = &self.lines[bounds.start + bounds.annotation_count + row];
            let text = line.text();
            let prefix_end = text.find('|').expect("bounds validated first pipe") + 1;
            let prefix = text[..prefix_end].to_string();
            let padding_columns = bounds.prefix_columns - prefix_end;
            let left = text[..after_column.min(text.len())].to_string();
            let right = format!(
                "{prefix}{}{}",
                " ".repeat(padding_columns),
                text.get(after_column..).unwrap_or("")
            );
            TabSplitRow {
                source_line: line.source_line(),
                original_row: line
                    .tab
                    .expect("bounds validated provenance")
                    .original_row(),
                original_columns: text.len(),
                prefix,
                padding_columns,
                left_columns: left.len(),
                right_columns: right.len(),
                right_label_only: text.len() <= after_column,
                left,
                right,
            }
        });
        let mut warnings = vec![
            tablature::SPLIT_NOTATION_WARNING.into(),
            tablature::SPLIT_PREFIX_WARNING.into(),
        ];
        if !annotations.is_empty() {
            warnings.push(tablature::SPLIT_ANNOTATION_WARNING.into());
        }
        if rows.iter().any(|row| row.right_label_only) {
            warnings.push(tablature::SPLIT_LABEL_ONLY_WARNING.into());
        }
        if rows
            .iter()
            .chain(&annotations)
            .any(|row| row.right_columns > MAX_COLUMNS)
        {
            warnings.push(tablature::SPLIT_RIGHT_WARNING.into());
        }
        Ok(TabSplitPlan {
            preview: TabSplitPreview {
                bounds,
                after_column,
                annotations,
                rows,
                warnings,
            },
            expected: self.snapshot(),
            revision: self.review_revision,
        })
    }

    /// Atomically replace the block with left annotations/six strings followed
    /// by right annotations/six strings. Focus moves to the first right row for
    /// repeated splitting. Original text,
    /// classifier, verdict, preamble and source identity are copied unchanged;
    /// generated text lives only in `edited`. Records exactly one undo snapshot.
    /// Plans become stale after review actions (including navigation or undo),
    /// or direct changes to the captured lines/position/metadata/view state.
    /// Transposition alone remains nonundoable and does not invalidate a plan.
    pub fn apply_tab_split(&mut self, plan: TabSplitPlan) -> Result<()> {
        if self.editing || plan.revision != self.review_revision || plan.expected != self.snapshot()
        {
            bail!("Tab split plan is stale; preview the current block again before applying.");
        }
        let left_instance = self.next_tab_instance;
        let right_instance = left_instance
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Tab instance identity exhausted."))?;
        let next_instance = right_instance
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Tab instance identity exhausted."))?;
        let preview = plan.preview;
        let originals = &self.lines[preview.bounds.start..preview.bounds.end];
        let annotation_count = preview.bounds.annotation_count;
        let original_annotations = &originals[..annotation_count];
        let original_strings = &originals[annotation_count..];
        let mut replacements = Vec::with_capacity(originals.len() * 2);
        for (instance, right) in [(left_instance, false), (right_instance, true)] {
            for (line, row) in original_annotations.iter().zip(&preview.annotations) {
                let mut fragment = line.clone();
                fragment.tab = fragment.tab.map(|tab| tab.in_instance(instance));
                fragment.edited = Some(if right {
                    row.right.clone()
                } else {
                    row.left.clone()
                });
                replacements.push(fragment);
            }
            for (line, row) in original_strings.iter().zip(&preview.rows) {
                let mut fragment = line.clone();
                fragment.tab = fragment.tab.map(|tab| tab.in_instance(instance));
                fragment.edited = Some(if right {
                    row.right.clone()
                } else {
                    row.left.clone()
                });
                replacements.push(fragment);
            }
        }
        self.record_undo();
        self.lines
            .splice(preview.bounds.start..preview.bounds.end, replacements);
        self.next_tab_instance = next_instance;
        self.position = preview.bounds.start + annotation_count + 6;
        self.horizontal_scroll = 0;
        self.recompute_flags();
        self.message =
            Some("Tab block split; focused first right row. Undo restores the whole split.".into());
        Ok(())
    }

    fn recompute_flags(&mut self) {
        self.flags = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.is_flag())
            .map(|(index, _)| index)
            .collect();
    }

    pub fn classifier_label(&self) -> String {
        self.current_line()
            .map(|line| line_kind_label(&line.kind).to_string())
            .unwrap_or_default()
    }

    /// Persistent status-line label for the net offset; `None` hides it at zero.
    pub fn transpose_label(&self) -> Option<String> {
        (self.transpose != 0).then(|| format!("Transposed {:+}", self.transpose))
    }

    pub fn verdict_label(&self) -> String {
        let Some(line) = self.current_line() else {
            return String::new();
        };
        match line.verdict {
            Verdict::Pending => match &line.kind {
                LineKind::Ambiguous(_) => {
                    "unreviewed (classifier: ambiguous; Enter keeps as lyric)".into()
                }
                _ => format!(
                    "unreviewed (classifier guess: {})",
                    line_kind_label(&line.kind)
                ),
            },
            Verdict::Keep => {
                if line.preamble && !matches!(line.kind, LineKind::TabAnnotation(_)) {
                    "keep (preamble · metadata consumed, not in body)".into()
                } else {
                    match &line.kind {
                        LineKind::Ambiguous(_) => "keep (resolved as lyric)".into(),
                        LineKind::Fretboard(_) => {
                            "keep (literal ASCII tabs; whitespace preserved, no fret transposition)"
                                .into()
                        }
                        LineKind::TabAnnotation(_) => {
                            "keep (literal tab annotation; attached, aligned and not transposed)"
                                .into()
                        }
                        LineKind::Blank | LineKind::Noise(_) => "keep (dropped)".into(),
                        _ => format!("keep ({})", line_kind_label(&line.kind)),
                    }
                }
            }
            Verdict::Discard => "discard".into(),
            Verdict::Lyric => "lyric".into(),
            Verdict::ChordOnly => "chord-only".into(),
            Verdict::SectionHeader => "section header".into(),
            Verdict::Noise => "noise".into(),
        }
    }

    pub fn pending_flags(&self) -> usize {
        self.flags
            .iter()
            .filter(|index| self.lines[**index].verdict == Verdict::Pending)
            .count()
    }

    pub fn handle(&mut self, key: Key) {
        if !matches!(
            key,
            Key::TransposeUp | Key::TransposeDown | Key::TransposeReset
        ) {
            self.review_revision = self.review_revision.wrapping_add(1);
        }
        if key == Key::CtrlC {
            self.phase = Phase::Quit;
            return;
        }
        // Transposition is a session-level offset over the triaged song, not a
        // review decision: it works in every phase, accumulates deltas like the
        // web `Session::transpose`, and is deliberately not undoable — the undo
        // stack covers review edits only, and `TransposeReset` is the way back.
        match key {
            Key::TransposeUp => self.transpose_by(1),
            Key::TransposeDown => self.transpose_by(-1),
            Key::TransposeReset => self.transpose_reset(),
            _ => match self.phase {
                Phase::Metadata => self.handle_metadata(key),
                Phase::Triage => self.handle_triage(key),
                Phase::Preview => self.handle_preview(key),
                Phase::Done | Phase::Quit => {}
            },
        }
    }

    fn handle_metadata(&mut self, key: Key) {
        match key {
            Key::Next | Key::Tab => {
                self.metadata_field = (self.metadata_field + 1) % METADATA_FIELD_COUNT;
            }
            Key::Prev | Key::BackTab => {
                self.metadata_field =
                    (self.metadata_field + METADATA_FIELD_COUNT - 1) % METADATA_FIELD_COUNT;
            }
            Key::Confirm => {
                self.phase = Phase::Triage;
                self.focus_first_flag_or(0);
            }
            Key::Char(ch) => {
                self.record_undo();
                self.metadata.field_mut(self.metadata_field).push(ch);
            }
            Key::Backspace => {
                self.record_undo();
                self.metadata.field_mut(self.metadata_field).pop();
            }
            Key::ClearField => {
                self.record_undo();
                self.metadata.field_mut(self.metadata_field).clear();
            }
            Key::Undo => self.undo(),
            Key::Quit | Key::Escape => self.phase = Phase::Quit,
            _ => {}
        }
    }

    fn handle_triage(&mut self, key: Key) {
        if self.editing {
            match key {
                Key::Char(ch) => self.edit_buffer.push(ch),
                Key::Backspace => {
                    self.edit_buffer.pop();
                }
                Key::Confirm => {
                    self.record_undo();
                    let buffer = std::mem::take(&mut self.edit_buffer);
                    if let Some(line) = self.current_line_mut() {
                        line.edited = Some(buffer);
                        if line.verdict == Verdict::Pending {
                            line.verdict = Verdict::Keep;
                        }
                    }
                    self.editing = false;
                }
                Key::Escape => {
                    self.editing = false;
                    self.edit_buffer.clear();
                }
                _ => {}
            }
            return;
        }

        match key {
            Key::PanLeft => self.horizontal_scroll = self.horizontal_scroll.saturating_sub(8),
            Key::PanRight => self.horizontal_scroll = self.horizontal_scroll.saturating_add(8),
            Key::Next => self.move_to(self.position.saturating_add(1)),
            Key::Prev => self.move_to(self.position.saturating_sub(1)),
            Key::NextFlag => self.move_to_next_flag(1),
            Key::PrevFlag => self.move_to_next_flag(-1),
            Key::PageDown => self.move_to(self.position.saturating_add(10)),
            Key::PageUp => self.move_to(self.position.saturating_sub(10)),
            Key::Confirm => self.decide(Verdict::Keep),
            Key::Discard => self.decide(Verdict::Discard),
            Key::Lyric => self.decide(Verdict::Lyric),
            Key::ChordOnly => self.decide(Verdict::ChordOnly),
            Key::SectionHeader => self.decide(Verdict::SectionHeader),
            Key::Noise => self.decide(Verdict::Noise),
            Key::TabAnnotation => self.associate_tab_annotation(),
            Key::Edit => {
                if let Some(line) = self.current_line() {
                    self.edit_buffer = line.text().to_string();
                    self.editing = true;
                }
            }
            Key::Undo => self.undo(),
            Key::AutoAccept => self.accept_rest(),
            Key::Skip => self.enter_preview(),
            Key::Quit | Key::Escape => self.phase = Phase::Quit,
            _ => {}
        }
    }

    fn handle_preview(&mut self, key: Key) {
        match key {
            Key::PanLeft => self.horizontal_scroll = self.horizontal_scroll.saturating_sub(8),
            Key::PanRight => self.horizontal_scroll = self.horizontal_scroll.saturating_add(8),
            Key::Next => self.scroll_preview(1),
            Key::Prev => self.scroll_preview(-1),
            Key::PageDown => self.scroll_preview(PREVIEW_PAGE as isize),
            Key::PageUp => self.scroll_preview(-(PREVIEW_PAGE as isize)),
            Key::Confirm => self.phase = Phase::Done,
            Key::Back | Key::Escape => self.phase = Phase::Triage,
            Key::Quit => self.phase = Phase::Quit,
            _ => {}
        }
    }

    fn scroll_preview(&mut self, delta: isize) {
        let max = preview::line_count(&self.finish(), &self.metadata).saturating_sub(1);
        let next = self.preview_scroll as isize + delta;
        self.preview_scroll = next.clamp(0, max as isize) as usize;
    }

    fn enter_preview(&mut self) {
        self.preview_scroll = 0;
        self.phase = Phase::Preview;
    }

    fn transpose_by(&mut self, delta: i32) {
        self.transpose += delta;
        self.message = Some(transpose_message(self.transpose));
    }

    fn transpose_reset(&mut self) {
        self.transpose = 0;
        self.message = Some(transpose_message(0));
    }

    fn focus_first_flag_or(&mut self, fallback: usize) {
        self.position = self.flags.first().copied().unwrap_or(fallback);
    }

    fn move_to(&mut self, index: usize) {
        if self.lines.is_empty() {
            return;
        }
        let next = index.min(self.lines.len() - 1);
        if next != self.position {
            self.position = next;
            self.message = None;
        }
    }

    fn move_to_next_flag(&mut self, direction: isize) {
        let candidate = if direction > 0 {
            self.flags
                .iter()
                .copied()
                .find(|index| *index > self.position)
        } else {
            self.flags
                .iter()
                .copied()
                .rev()
                .find(|index| *index < self.position)
        };
        match candidate {
            Some(index) => {
                self.position = index;
                self.message = None;
            }
            None => self.message = Some("No more flagged lines.".into()),
        }
    }

    /// Explicit association is intentionally different from the conservative
    /// parser: a reviewer may mark an arbitrary comment row, but only when it
    /// is directly before (or in the existing annotation prefix of) a complete
    /// retained original-six block. No discarded/blank/prose row is crossed.
    fn associate_tab_annotation(&mut self) {
        let position = self.position;
        let Some(current) = self.current_line() else {
            return;
        };
        if current
            .tab
            .is_some_and(|tab| tab.role() == TabRowRole::String)
            || matches!(current.effective_kind(), Some(LineKind::Fretboard(_)))
        {
            self.message = Some(
                "Select a comment row before a tab block, not one of its six string rows.".into(),
            );
            return;
        }
        let mut string_start = position + 1;
        while self
            .lines
            .get(string_start)
            .is_some_and(|line| matches!(line.effective_kind(), Some(LineKind::TabAnnotation(_))))
        {
            string_start += 1;
        }
        let target = self
            .lines
            .get(string_start)
            .and_then(|line| line.tab)
            .filter(|tab| tab.role() == TabRowRole::String && tab.original_rows() == 6);
        let Some(target) = target else {
            self.message = Some("Tab annotation must be immediately before a complete original six-string block; only already associated annotation rows may intervene.".into());
            return;
        };
        let identity = (target.origin(), target.instance());
        let valid_strings = (0..6).all(|row| {
            self.lines.get(string_start + row).is_some_and(|line| {
                line.tab.is_some_and(|tab| {
                    (tab.origin(), tab.instance()) == identity
                        && tab.role() == TabRowRole::String
                        && tab.original_rows() == 6
                        && tab.original_row() == row + 1
                }) && matches!(line.effective_kind(), Some(LineKind::Fretboard(_)))
            })
        });
        let valid_intervening = self.lines[position + 1..string_start].iter().all(|line| {
            line.tab.is_some_and(|tab| {
                (tab.origin(), tab.instance()) == identity && tab.role() == TabRowRole::Annotation
            })
        });
        if !valid_strings || !valid_intervening {
            self.message = Some("Cannot associate this row: the following block must contain its six retained string rows on stable original instance boundaries.".into());
            return;
        }

        self.record_undo();
        self.lines[position].kind =
            LineKind::TabAnnotation(self.lines[position].text().to_string());
        self.lines[position].verdict = Verdict::Keep;
        for (row, line) in self.lines[position..string_start].iter_mut().enumerate() {
            line.tab = Some(target.annotation(line.source_line, row + 1));
        }
        self.recompute_flags();
        self.move_to(position.saturating_add(1));
        self.message = Some(
            "Line kept as tab annotation and associated with the following six-string block."
                .into(),
        );
    }

    fn decide(&mut self, verdict: Verdict) {
        if self.current_line().is_none() {
            return;
        }
        self.record_undo();
        if let Some(line) = self.current_line_mut() {
            line.verdict = verdict;
        }
        self.message = None;
        self.move_to(self.position.saturating_add(1));
    }

    fn accept_rest(&mut self) {
        self.record_undo();
        for index in &self.flags {
            if *index < self.position {
                continue;
            }
            if self.lines[*index].verdict == Verdict::Pending {
                self.lines[*index].verdict = Verdict::Keep;
            }
        }
        self.enter_preview();
    }

    fn record_undo(&mut self) {
        self.review_revision = self.review_revision.wrapping_add(1);
        let snapshot = self.snapshot();
        self.undo.push(snapshot);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            metadata: self.metadata.clone(),
            metadata_field: self.metadata_field,
            lines: self.lines.clone(),
            position: self.position,
            phase: self.phase,
            preview_scroll: self.preview_scroll,
            horizontal_scroll: self.horizontal_scroll,
        }
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.undo.pop() {
            self.metadata = snapshot.metadata;
            self.metadata_field = snapshot.metadata_field;
            self.lines = snapshot.lines;
            self.position = snapshot.position;
            self.phase = snapshot.phase;
            self.preview_scroll = snapshot.preview_scroll;
            self.horizontal_scroll = snapshot.horizontal_scroll;
            self.recompute_flags();
            self.editing = false;
            self.edit_buffer.clear();
            self.message = Some("Undone.".into());
        } else {
            self.message = Some("Nothing to undo.".into());
        }
    }

    /// The triaged song with the session's transposition offset applied
    /// (`transpose_song` leaves zero and full-octave offsets byte-exact).
    pub fn finish(&self) -> Song {
        let mut song = Song::default();
        self.metadata.apply_to(&mut song);
        song.sections = rebuild_sections(&self.lines);
        let retained: Vec<_> = self
            .lines
            .iter()
            .filter(|line| {
                matches!(
                    line.effective_kind(),
                    Some(LineKind::Fretboard(_) | LineKind::TabAnnotation(_))
                )
            })
            .map(|line| line.tab)
            .collect();
        tablature::attach(&mut song, &retained);
        if self.transpose != 0 {
            transpose_song(&mut song, self.transpose);
        }
        song
    }
}

/// Rebuilds song sections from every raw line and its decision, mirroring
/// `parse`: preamble/blank/noise flush pending chords, chord-only lines merge
/// into the next lyric, section headers split sections (ADR0014).
fn rebuild_sections(lines: &[ReviewLine]) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current = Section {
        kind: SectionKind::Intro,
        lines: Vec::new(),
    };
    let mut pending_chords: Option<String> = None;

    for line in lines {
        let raw_text = line.text().to_string();
        match line.effective_kind() {
            None => flush_pending(&mut current, &mut pending_chords),
            Some(LineKind::SectionHeader(kind, _)) => {
                flush_pending(&mut current, &mut pending_chords);
                if current.lines.is_empty() {
                    current.kind = kind;
                } else {
                    sections.push(std::mem::replace(
                        &mut current,
                        Section {
                            kind,
                            lines: Vec::new(),
                        },
                    ));
                }
            }
            Some(LineKind::ChordOnly(_)) => {
                flush_pending(&mut current, &mut pending_chords);
                pending_chords = Some(raw_text);
            }
            Some(LineKind::Lyric(text)) | Some(LineKind::Ambiguous(text)) => {
                if let Some(chords) = pending_chords.take() {
                    current
                        .lines
                        .push(LineKind::Lyric(merge_chord_line(&chords, &raw_text)));
                } else {
                    current.lines.push(LineKind::Lyric(text));
                }
            }
            Some(LineKind::Fretboard(text)) => {
                flush_pending(&mut current, &mut pending_chords);
                current.lines.push(LineKind::Fretboard(text));
            }
            Some(LineKind::TabAnnotation(text)) => {
                flush_pending(&mut current, &mut pending_chords);
                current.lines.push(LineKind::TabAnnotation(text));
            }
            Some(LineKind::Blank) | Some(LineKind::Noise(_)) => {
                flush_pending(&mut current, &mut pending_chords);
            }
        }
    }
    flush_pending(&mut current, &mut pending_chords);
    if !current.lines.is_empty() {
        sections.push(current);
    }
    sections
}

fn flush_pending(current: &mut Section, pending: &mut Option<String>) {
    if let Some(chords) = pending.take() {
        current.lines.push(LineKind::ChordOnly(chords));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::apply_noninteractive_defaults;
    use crate::latex::render::render;
    use crate::parser::classify::parse;

    fn state_for(raw: &str) -> TriageState {
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        state
    }

    fn flag_index(state: &TriageState, needle: &str) -> usize {
        state
            .lines
            .iter()
            .position(|line| line.original.contains(needle))
            .unwrap_or_else(|| panic!("no line contains {needle:?}"))
    }

    fn rendered_state(state: &TriageState) -> String {
        render(&state.finish(), false).unwrap()
    }

    fn expected_noninteractive_output(raw: &str) -> String {
        let mut song = parse(raw);
        apply_noninteractive_defaults(&mut song);
        render(&song, false).unwrap()
    }

    const AMBIGUOUS_RAW: &str =
        "Song: Test\nArtist: Band\n[Verse]\n[Am]Hello\nmystery line\n[F]Bye";

    #[test]
    fn new_flags_ambiguous_and_fretboard_and_starts_at_first_flag() {
        let raw = "Song: T\n[Verse]\n[Am]Hello\nmystery\ne|---3---|";
        let state = TriageState::new(parse(raw), raw);
        assert_eq!(state.flags.len(), 2);
        assert_eq!(state.position, flag_index(&state, "mystery"));
        assert!(state.lines.iter().any(|line| line.preamble));
    }

    #[test]
    fn unedited_review_matches_noninteractive_defaults() {
        let fixtures = [
            include_str!("../../../examples/let-it-be.txt"),
            include_str!("../../../examples/synthetic.txt"),
            "Title here\nArtist here\n\nStandard (E A D G B E)\n[Intro]\nAm  F  C  G\n2x\n[Verse 1]\n[Am]Hello there\nmystery line\n[F]Bye\ne|---3---|\nB|---1---|\nG|---0---|\nD|---2---|\nA|---3---|\nE|-------|\nWas this info helpful? Yes No\n[Chorus]\n[C]La la\n[Outro]\n",
            "Song: Spaced\nArtist: Band\n[Verse]\n  [Am]Hello there  \n    \n  Dm    G    \n  spaced lyric  \nWas this info helpful? Yes No\n[F]Goodbye\n",
        ];
        for raw in fixtures {
            let state = TriageState::new(parse(raw), raw);
            assert_eq!(
                render(&state.finish(), false).unwrap(),
                expected_noninteractive_output(raw),
                "unedited rebuild drifted from parse for:\n{raw}"
            );
        }
    }

    #[test]
    fn chord_over_lyrics_lines_are_kept_lyrics_not_flags() {
        let raw = "\
Song: Context Example
Artist: Example Band

[Verse]
C       G
first generic line
Am      F
second generic line
odd parenthetical (side note)
Dm      G
third generic line
";
        let state = TriageState::new(parse(raw), raw);
        for needle in [
            "first generic line",
            "second generic line",
            "third generic line",
        ] {
            let line = &state.lines[flag_index(&state, needle)];
            assert!(
                matches!(line.kind, LineKind::Lyric(_)),
                "{needle:?} should be classified as lyric, got {:?}",
                line.kind
            );
            assert_eq!(line.verdict, Verdict::Keep, "{needle:?}");
            assert!(!line.is_flag(), "{needle:?}");
        }

        let odd = &state.lines[flag_index(&state, "odd parenthetical")];
        assert!(matches!(odd.kind, LineKind::Ambiguous(_)), "{:?}", odd.kind);
        assert_eq!(odd.verdict, Verdict::Pending);
        assert_eq!(state.flags.len(), 1);
        assert_eq!(state.pending_flags(), 1);
    }

    #[test]
    fn triage_ambiguous_count_matches_audit() {
        let raw = "\
Song: Context Example
Artist: Example Band

[Verse]
C       G
first generic line
Am      F
second generic line
odd parenthetical (side note)
Dm      G
third generic line
";
        let state = TriageState::new(parse(raw), raw);
        let triage_ambiguous = state
            .lines
            .iter()
            .filter(|line| !line.preamble && matches!(line.kind, LineKind::Ambiguous(_)))
            .count();
        let summary = crate::audit::audit_summary_line(&parse(raw), raw);
        assert_eq!(triage_ambiguous, 1);
        assert_eq!(state.pending_flags(), triage_ambiguous);
        assert!(summary.contains("1 ambiguous kept as lyric"), "{summary}");
    }

    #[test]
    fn preamble_noise_metadata_and_section_kinds_flow_into_review_lines() {
        let raw = "\
Song: Demo Tune
Artist: Demo Band
Tabbed by: Example

[Verse]
F#m:   244222
[Am]A generic line
";
        let state = TriageState::new(parse(raw), raw);

        let title = &state.lines[flag_index(&state, "Song: Demo Tune")];
        assert!(matches!(title.kind, LineKind::Noise(_)), "{:?}", title.kind);
        assert!(title.preamble);
        assert_eq!(title.verdict, Verdict::Keep);

        let artist = &state.lines[flag_index(&state, "Artist: Demo Band")];
        assert!(
            matches!(artist.kind, LineKind::Noise(_)),
            "{:?}",
            artist.kind
        );
        assert!(artist.preamble);

        let credit = &state.lines[flag_index(&state, "Tabbed by: Example")];
        assert!(
            matches!(credit.kind, LineKind::Noise(_)),
            "{:?}",
            credit.kind
        );
        assert!(credit.preamble);

        let blank = state
            .lines
            .iter()
            .find(|line| matches!(line.kind, LineKind::Blank))
            .expect("expected a blank raw line");
        assert!(blank.preamble);

        let verse = &state.lines[flag_index(&state, "[Verse]")];
        assert!(
            matches!(verse.kind, LineKind::SectionHeader(SectionKind::Verse, _)),
            "{:?}",
            verse.kind
        );
        assert!(!verse.preamble);
        assert_eq!(verse.verdict, Verdict::Keep);
        assert!(!verse.is_flag());

        let definition = &state.lines[flag_index(&state, "F#m:   244222")];
        assert!(
            matches!(definition.kind, LineKind::Noise(_)),
            "{:?}",
            definition.kind
        );
        assert!(!definition.preamble);

        let lyric = &state.lines[flag_index(&state, "[Am]A generic line")];
        assert!(matches!(lyric.kind, LineKind::Lyric(_)), "{:?}", lyric.kind);
        assert_eq!(lyric.verdict, Verdict::Keep);
        assert!(!lyric.is_flag());
    }

    #[test]
    fn every_raw_line_is_navigable() {
        let mut state = state_for(AMBIGUOUS_RAW);
        assert_eq!(state.position, flag_index(&state, "mystery"));
        state.handle(Key::Next);
        assert_eq!(state.position, flag_index(&state, "[F]Bye"));
        state.handle(Key::Prev);
        state.handle(Key::Prev);
        assert_eq!(state.position, flag_index(&state, "[Am]Hello"));
        state.handle(Key::NextFlag);
        assert_eq!(state.position, flag_index(&state, "mystery"));
    }

    #[test]
    fn enter_keeps_ambiguous_line_as_lyric() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::Confirm);
        let song = state.finish();
        assert!(song.sections[0]
            .lines
            .iter()
            .any(|line| line == &LineKind::Lyric("mystery line".into())));
    }

    #[test]
    fn effective_label_names_what_the_line_is_now() {
        let index = flag_index(&state_for(AMBIGUOUS_RAW), "mystery");
        for (key, label) in [
            (Key::Confirm, "lyric"), // Keep resolves ambiguous as lyric
            (Key::Discard, "discarded"),
            (Key::Noise, "noise"),
            (Key::Lyric, "lyric"),
            (Key::ChordOnly, "chord-only"),
            (Key::SectionHeader, "section header"),
        ] {
            let mut state = state_for(AMBIGUOUS_RAW);
            state.handle(key);
            assert_eq!(
                state.lines[index].effective_label(),
                label,
                "{key:?} on {index}"
            );
        }

        // Pending names the parser's guess as-is: nothing is decided yet, so no
        // effective resolution and no implied "(was: …)" change.
        let state = state_for(AMBIGUOUS_RAW);
        assert_eq!(state.lines[index].effective_label(), "ambiguous");
    }

    #[test]
    fn kept_blank_and_noise_lines_label_as_dropped() {
        let raw = "Song: T\n[Verse]\n[Am]Hello\n\nF#m:   244222\nmystery line";
        let state = TriageState::new(parse(raw), raw);

        let lyric = &state.lines[flag_index(&state, "[Am]Hello")];
        assert_eq!(lyric.effective_label(), "lyric");
        assert!(!lyric.is_dropped());

        // A kept blank body line exports nothing: dropped, honestly.
        let blank = state
            .lines
            .iter()
            .find(|line| matches!(line.kind, LineKind::Blank))
            .unwrap();
        assert!(!blank.preamble);
        assert_eq!(blank.verdict, Verdict::Keep);
        assert!(blank.is_dropped());
        assert_eq!(blank.effective_label(), "dropped");

        let definition = &state.lines[flag_index(&state, "F#m:")];
        assert!(
            matches!(definition.kind, LineKind::Noise(_)),
            "{:?}",
            definition.kind
        );
        assert!(!definition.preamble);
        assert!(definition.is_dropped());
        assert_eq!(definition.effective_label(), "dropped");
    }

    #[test]
    fn pending_and_preamble_rows_label_with_the_classifier() {
        let raw = "Song: T\n[Verse]\n[Am]Hello\nmystery line";
        let state = TriageState::new(parse(raw), raw);

        // Pending: the label is the parser's guess, equal to the classifier
        // so no "(was: …)" suffix can appear.
        let flag = &state.lines[flag_index(&state, "mystery")];
        assert_eq!(flag.verdict, Verdict::Pending);
        assert_eq!(flag.effective_label(), "ambiguous");
        assert_eq!(flag.effective_label(), line_kind_label(&flag.kind));

        // Preamble: consumed as metadata (is_dropped holds — nothing exports),
        // but the label keeps the parser's name; the "preamble" status
        // carries the explanation, dimmed per the advisor's table.
        let title = &state.lines[flag_index(&state, "Song: T")];
        assert!(title.preamble);
        assert!(title.is_dropped());
        assert_eq!(title.effective_label(), "noise");
        assert_eq!(title.effective_label(), line_kind_label(&title.kind));
    }

    #[test]
    fn discard_removes_line_on_rebuild() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::Discard);
        assert!(!rendered_state(&state).contains("mystery line"));
    }

    #[test]
    fn reclassify_chord_only_and_noise() {
        let mut chord = state_for(AMBIGUOUS_RAW);
        chord.handle(Key::ChordOnly);
        assert!(!rendered_state(&chord).contains("mystery line"));

        let mut noise = state_for(AMBIGUOUS_RAW);
        noise.handle(Key::Noise);
        assert!(!rendered_state(&noise).contains("mystery line"));
    }

    #[test]
    fn reclassify_as_section_header_splits_sections() {
        let mut state = state_for(AMBIGUOUS_RAW);
        let index = flag_index(&state, "mystery");
        state.handle(Key::SectionHeader);
        let song = state.finish();
        assert_eq!(state.lines[index].verdict, Verdict::SectionHeader);
        assert_eq!(
            song.sections.last().unwrap().kind,
            SectionKind::Other("mystery line".into())
        );
    }

    #[test]
    fn edit_updates_line_text() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::Edit);
        state.handle(Key::Char('!'));
        state.handle(Key::Confirm);
        assert_eq!(state.current_line().unwrap().text(), "mystery line!");
        assert!(rendered_state(&state).contains("mystery line!"));
    }

    #[test]
    fn edit_can_be_cancelled() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::Edit);
        state.handle(Key::Char('!'));
        state.handle(Key::Escape);
        assert_eq!(state.current_line().unwrap().text(), "mystery line");
        assert!(!state.editing);
    }

    #[test]
    fn a_false_positive_noise_line_can_be_restored_as_lyric() {
        let raw = "Song: T\n[Verse]\n[Am]Hello\nSubmit corrections\n[F]Bye";
        let mut state = state_for(raw);
        assert!(!rendered_state(&state).contains("Submit corrections"));
        let index = flag_index(&state, "Submit corrections");
        state.move_to(index);
        state.handle(Key::Lyric);
        assert!(rendered_state(&state).contains("Submit corrections"));
    }

    #[test]
    fn a_preamble_line_can_be_restored_to_the_body() {
        let raw = "Song: T\nArtist: A\n[Verse]\n[Am]Hello";
        let mut state = state_for(raw);
        let index = flag_index(&state, "Artist: A");
        state.move_to(index);
        state.handle(Key::Lyric);
        assert!(rendered_state(&state).contains("Artist: A"));
    }

    #[test]
    fn undo_restores_previous_decision_and_edit() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::Discard);
        state.handle(Key::Undo);
        assert_eq!(
            state.lines[flag_index(&state, "mystery")].verdict,
            Verdict::Pending
        );

        state.move_to(flag_index(&state, "mystery"));
        state.handle(Key::Edit);
        state.handle(Key::Char('!'));
        state.handle(Key::Confirm);
        state.handle(Key::Undo);
        assert_eq!(
            state.lines[flag_index(&state, "mystery")].text(),
            "mystery line"
        );
    }

    #[test]
    fn auto_accept_keeps_remaining_flags_and_previews() {
        let raw = "Song: T\n[Verse]\nfirst mystery\nsecond mystery";
        let mut state = state_for(raw);
        state.handle(Key::Discard);
        state.handle(Key::AutoAccept);
        assert_eq!(state.phase, Phase::Preview);
        let song = state.finish();
        let output = render(&song, false).unwrap();
        assert!(!output.contains("first mystery"));
        assert!(output.contains("second mystery"));
    }

    #[test]
    fn no_flags_goes_straight_to_preview_then_exports() {
        let raw = "Song: T\n[Verse]\n[Am]Hello";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        assert_eq!(state.phase, Phase::Triage);
        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        state.handle(Key::Confirm);
        assert_eq!(state.phase, Phase::Done);
    }

    #[test]
    fn preview_back_returns_to_triage_with_state_intact() {
        let raw = "Song: T\n[Verse]\nfirst mystery\nsecond mystery";
        let mut state = state_for(raw);
        state.handle(Key::Discard);
        state.handle(Key::AutoAccept);
        state.handle(Key::Back);
        assert_eq!(state.phase, Phase::Triage);
        state.handle(Key::Undo);
        assert_eq!(
            state.lines[flag_index(&state, "first mystery")].verdict,
            Verdict::Discard
        );
        state.handle(Key::Undo);
        assert_eq!(
            state.lines[flag_index(&state, "first mystery")].verdict,
            Verdict::Pending
        );
    }

    #[test]
    fn quit_and_ctrl_c_leave_without_finishing() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::Quit);
        assert_eq!(state.phase, Phase::Quit);

        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::CtrlC);
        assert_eq!(state.phase, Phase::Quit);
    }

    #[test]
    fn metadata_shows_inferred_values_and_edits() {
        let raw = "\nLet It Be\nThe Beatles\n[Verse]\n[Am]Hello";
        let mut state = TriageState::new(parse(raw), raw);
        assert_eq!(state.metadata.title, "Let It Be");
        assert!(state.metadata.title_inferred);
        assert!(state.metadata.artist_inferred);

        state.handle(Key::ClearField);
        state.handle(Key::Char('X'));
        state.handle(Key::Tab);
        state.handle(Key::Confirm);
        let song = state.finish();
        assert_eq!(song.title.as_deref(), Some("X"));
        assert_eq!(song.artist.as_deref(), Some("The Beatles"));
    }

    #[test]
    fn metadata_clear_removes_key_capo_and_tuning() {
        let raw = "Song: X\nKey: C\nCapo: 2\nTuning: DADGAD\n[Verse]\n[Am]Hello";
        let mut state = TriageState::new(parse(raw), raw);
        for _ in 0..2 {
            state.handle(Key::Tab);
        }
        state.handle(Key::ClearField);
        state.handle(Key::Tab);
        state.handle(Key::ClearField);
        state.handle(Key::Tab);
        state.handle(Key::ClearField);
        let song = state.finish();
        assert_eq!(song.key, None);
        assert_eq!(song.capo, None);
        assert_eq!(song.tuning, None);
    }

    #[test]
    fn metadata_undo_restores_value() {
        let raw = "Song: X\nArtist: Y\n[Verse]\n[Am]Hello";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::ClearField);
        assert_eq!(state.metadata.title, "");
        state.handle(Key::Undo);
        assert_eq!(state.metadata.title, "X");
    }

    #[test]
    fn transpose_keys_shift_chords_in_the_finished_song() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::TransposeUp);
        state.handle(Key::TransposeUp);
        assert_eq!(state.transpose, 2);
        assert_eq!(state.transpose_label().as_deref(), Some("Transposed +2"));
        let rendered = rendered_state(&state);
        assert!(rendered.contains("\\[Bm]Hello"), "{rendered}");
        assert!(rendered.contains("\\[G]Bye"), "{rendered}");
        assert!(!rendered.contains("[Am]Hello"));
    }

    #[test]
    fn transpose_down_and_reset_restore_the_original_output() {
        let mut state = state_for(AMBIGUOUS_RAW);
        let original = rendered_state(&state);
        state.handle(Key::TransposeDown);
        assert_eq!(state.transpose, -1);
        assert_eq!(state.transpose_label().as_deref(), Some("Transposed -1"));
        assert!(rendered_state(&state).contains("\\[Abm]Hello"));

        state.handle(Key::TransposeReset);
        assert_eq!(state.transpose, 0);
        assert_eq!(state.transpose_label(), None);
        assert_eq!(rendered_state(&state), original);
    }

    #[test]
    fn transpose_accumulates_and_reset_covers_any_offset() {
        let mut state = state_for(AMBIGUOUS_RAW);
        for _ in 0..13 {
            state.handle(Key::TransposeUp);
        }
        assert_eq!(state.transpose, 13);
        assert_eq!(state.transpose_label().as_deref(), Some("Transposed +13"));
        state.handle(Key::TransposeReset);
        assert_eq!(state.transpose, 0);
        assert_eq!(
            rendered_state(&state),
            rendered_state(&state_for(AMBIGUOUS_RAW))
        );
    }

    #[test]
    fn transpose_survives_navigation_and_undo() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::TransposeUp);
        state.handle(Key::Next);
        assert_eq!(state.transpose, 1);

        state.handle(Key::Discard);
        state.handle(Key::TransposeUp);
        assert_eq!(state.transpose, 2);
        state.handle(Key::Undo);
        // Undo reverts the review decision, never the transposition offset.
        assert_eq!(state.transpose, 2);
        assert_eq!(
            state.lines[flag_index(&state, "mystery")].verdict,
            Verdict::Pending
        );
        assert!(rendered_state(&state).contains("\\[Bm]Hello"));
    }

    #[test]
    fn transpose_works_from_the_preview_phase() {
        let mut state = state_for(AMBIGUOUS_RAW);
        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        state.handle(Key::TransposeUp);
        assert!(rendered_state(&state).contains("\\[A#m]Hello"));
    }
}
