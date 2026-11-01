use crate::parser::chord::merge_chord_line;
use crate::parser::classify::{
    classify_line, metadata_provenance, preamble_len, section_kind_from_header,
};
use crate::parser::song::{LineKind, Section, SectionKind, Song};
use crate::preview;

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

/// One raw input line plus its review decision (ADR0014).
#[derive(Debug, Clone)]
pub struct ReviewLine {
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
                if self.preamble {
                    return None;
                }
                match &self.kind {
                    LineKind::Blank | LineKind::Noise(_) => None,
                    LineKind::Ambiguous(_) => Some(LineKind::Lyric(text)),
                    LineKind::Lyric(_) => Some(LineKind::Lyric(text)),
                    LineKind::ChordOnly(_) => Some(LineKind::ChordOnly(raw_text)),
                    LineKind::Fretboard(_) => Some(LineKind::Fretboard(text)),
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
}

pub fn line_kind_label(kind: &LineKind) -> &'static str {
    match kind {
        LineKind::Lyric(_) => "lyric",
        LineKind::ChordOnly(_) => "chord-only",
        LineKind::SectionHeader(_, _) => "section header",
        LineKind::Fretboard(_) => "fretboard",
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

#[derive(Debug, Clone)]
struct Snapshot {
    metadata: Metadata,
    metadata_field: usize,
    lines: Vec<ReviewLine>,
    position: usize,
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
    undo: Vec<Snapshot>,
}

impl TriageState {
    pub fn new(song: Song, raw: &str) -> Self {
        let provenance = metadata_provenance(raw);
        let metadata =
            Metadata::from_song(&song, provenance.title_labelled, provenance.artist_labelled);
        let preamble = preamble_len(raw);
        let lines: Vec<ReviewLine> = raw
            .lines()
            .enumerate()
            .map(|(index, line)| {
                let kind = classify_line(line.trim());
                let preamble = index < preamble;
                let flagged =
                    !preamble && matches!(kind, LineKind::Ambiguous(_) | LineKind::Fretboard(_));
                ReviewLine {
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
            undo: Vec::new(),
        }
    }

    pub fn current_line(&self) -> Option<&ReviewLine> {
        self.lines.get(self.position)
    }

    fn current_line_mut(&mut self) -> Option<&mut ReviewLine> {
        self.lines.get_mut(self.position)
    }

    pub fn classifier_label(&self) -> String {
        self.current_line()
            .map(|line| line_kind_label(&line.kind).to_string())
            .unwrap_or_default()
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
                if line.preamble {
                    "keep (preamble · metadata consumed, not in body)".into()
                } else {
                    match &line.kind {
                        LineKind::Ambiguous(_) => "keep (resolved as lyric)".into(),
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
        if key == Key::CtrlC {
            self.phase = Phase::Quit;
            return;
        }
        match self.phase {
            Phase::Metadata => self.handle_metadata(key),
            Phase::Triage => self.handle_triage(key),
            Phase::Preview => self.handle_preview(key),
            Phase::Done | Phase::Quit => {}
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
        let snapshot = self.snapshot();
        self.undo.push(snapshot);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            metadata: self.metadata.clone(),
            metadata_field: self.metadata_field,
            lines: self.lines.clone(),
            position: self.position,
        }
    }

    fn undo(&mut self) {
        if let Some(snapshot) = self.undo.pop() {
            self.metadata = snapshot.metadata;
            self.metadata_field = snapshot.metadata_field;
            self.lines = snapshot.lines;
            self.position = snapshot.position;
            self.editing = false;
            self.edit_buffer.clear();
            self.message = Some("Undone.".into());
        } else {
            self.message = Some("Nothing to undo.".into());
        }
    }

    pub fn finish(&self) -> Song {
        let mut song = Song::default();
        self.metadata.apply_to(&mut song);
        song.sections = rebuild_sections(&self.lines);
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
            "Title here\nArtist here\n\nStandard (E A D G B E)\n[Intro]\nAm  F  C  G\n2x\n[Verse 1]\n[Am]Hello there\nmystery line\n[F]Bye\ne|---3---|\nWas this info helpful? Yes No\n[Chorus]\n[C]La la\n[Outro]\n",
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
}
