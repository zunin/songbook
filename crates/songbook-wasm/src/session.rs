use crate::actions::{map_action, map_key};
use crate::error::SessionError;
use crate::model::{
    self, BookPartSnapshot, PreviewRowSnapshot, SessionSnapshot, TabSplitPreviewSnapshot,
};
use songbook_core::book;
use songbook_core::latex::render::render;
use songbook_core::parser::classify::parse;
use songbook_core::transpose::transpose_song;
use songbook_core::triage::{Key, Phase, TabSplitPlan, TriageState};

/// Browser-friendly state machine over a single in-memory song.
///
/// The session owns the raw-line triage model from `songbook-core`; nothing is
/// written to disk and no network or PDF code is involved. An optional
/// transposition offset (see [`Session::transpose`]) applies on top of the
/// triaged song for every derived view.
#[derive(Debug)]
pub struct Session {
    state: TriageState,
    /// Net transposition in semitones applied on top of the triaged song.
    transpose: i32,
    /// One-use core-generated operation. Only its read-only preview is exposed.
    pending_tab_split: Option<TabSplitPlan>,
}

impl Session {
    /// Parses `raw` and starts the review at the metadata phase.
    pub fn new(raw: &str) -> Result<Self, SessionError> {
        if raw.trim().is_empty() {
            return Err(SessionError::EmptyInput);
        }
        let song = parse(raw);
        if song.sections.is_empty() {
            return Err(SessionError::NoSections);
        }
        Ok(Self {
            state: TriageState::new(song, raw),
            transpose: 0,
            pending_tab_split: None,
        })
    }

    /// Transposes the current triaged song by `delta` semitones (negative =
    /// down). Delta-based: +2 then −1 leaves a net +1. The offset applies to
    /// every derived view — snapshots, preview rows, exports, and book parts —
    /// and survives triage edits and undo, because the triage model keeps the
    /// untransposed raw lines and the offset is applied on top.
    pub fn transpose(&mut self, delta: i32) -> Result<(), SessionError> {
        self.pending_tab_split = None;
        self.transpose += delta;
        Ok(())
    }

    /// The triaged song with the session's transposition applied.
    fn current_song(&self) -> songbook_core::parser::song::Song {
        let mut song = self.state.finish();
        transpose_song(&mut song, self.transpose);
        song
    }

    /// Full snapshot of metadata, phase, and every raw review line.
    pub fn snapshot(&self) -> SessionSnapshot {
        model::snapshot(&self.state, self.transpose)
    }

    /// Stage an explicit raw-column split and return core's literal preview.
    /// A new request drops the previous plan, even if the request is invalid.
    /// Planning does not change review, selection, messages or undo history.
    pub fn plan_tab_split(
        &mut self,
        after_column: usize,
    ) -> Result<TabSplitPreviewSnapshot, SessionError> {
        self.pending_tab_split = None;
        let plan = self
            .state
            .plan_tab_split(after_column)
            .map_err(|error| SessionError::TabSplit(error.to_string()))?;
        let preview = plan.preview().into();
        self.pending_tab_split = Some(plan);
        Ok(preview)
    }

    /// Consume the pending plan and atomically apply it through core. Core also
    /// validates revision/selection/data; failure consumes the plan without edits.
    /// Returns the same fresh, transposition-aware snapshot as `snapshot()`.
    pub fn apply_tab_split(&mut self) -> Result<SessionSnapshot, SessionError> {
        let plan = self
            .pending_tab_split
            .take()
            .ok_or(SessionError::NoTabSplitPlan)?;
        self.state
            .apply_tab_split(plan)
            .map_err(|error| SessionError::TabSplit(error.to_string()))?;
        Ok(self.snapshot())
    }

    /// Drop the plan only, without a review action, message or undo mutation.
    pub fn cancel_tab_split(&mut self) -> SessionSnapshot {
        self.pending_tab_split = None;
        self.snapshot()
    }

    /// Structure preview rows for the current decisions and metadata.
    pub fn preview_rows(&self) -> Vec<PreviewRowSnapshot> {
        model::preview_rows(&self.current_song(), &self.state.metadata)
    }

    /// Renders the current decisions as songs-package LaTeX.
    pub fn export_tex(&self, standalone: bool) -> Result<String, SessionError> {
        render(&self.current_song(), standalone)
            .map_err(|error| SessionError::Render(error.to_string()))
    }

    /// The song's book part (ADR0019): the non-standalone LaTeX body —
    /// exactly what [`Session::export_tex`] with `false` returns — plus its
    /// distinct chords as core's book composer expects them. The body is
    /// handed back to `composeBook` verbatim; no LaTeX is assembled in
    /// JavaScript.
    pub fn book_parts(&self) -> Result<BookPartSnapshot, SessionError> {
        let song = self.current_song();
        let body = render(&song, false).map_err(|error| SessionError::Render(error.to_string()))?;
        let chords = book::distinct_chords(&song);
        Ok(BookPartSnapshot { body, chords })
    }

    /// Applies a semantic action such as `next`, `discard`, `undo`, or `skip`.
    pub fn handle_action(&mut self, action: &str) -> Result<(), SessionError> {
        let key = map_action(action)?;
        self.pending_tab_split = None;
        self.state.handle(key);
        Ok(())
    }

    /// Applies a browser key name, respecting the current phase and edit mode.
    pub fn handle_key(&mut self, key: &str) -> Result<(), SessionError> {
        let key = map_key(&self.state, key)?;
        self.pending_tab_split = None;
        self.state.handle(key);
        Ok(())
    }

    /// Inserts exactly one character into the current edit buffer or metadata
    /// field. Rejects input when no text field is active.
    pub fn insert_char(&mut self, value: &str) -> Result<(), SessionError> {
        if !self.can_type() {
            return Err(SessionError::NotEditing);
        }
        let mut chars = value.chars();
        let ch = match (chars.next(), chars.next()) {
            (Some(ch), None) => ch,
            _ => return Err(SessionError::InvalidChar(value.to_string())),
        };
        self.pending_tab_split = None;
        self.state.handle(Key::Char(ch));
        Ok(())
    }

    /// Inserts every character of `text` into the current edit buffer or
    /// metadata field.
    pub fn insert_text(&mut self, text: &str) -> Result<(), SessionError> {
        if !self.can_type() {
            return Err(SessionError::NotEditing);
        }
        self.pending_tab_split = None;
        for ch in text.chars() {
            self.state.handle(Key::Char(ch));
        }
        Ok(())
    }

    fn can_type(&self) -> bool {
        self.state.editing || self.state.phase == Phase::Metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = "Song: Test\nArtist: Band\n[Verse]\n[Am]Hello\nmystery line";
    const TAB: &str = "  e|--0--12--|  \n  B|--1------|  \n  G|--2------|  \n  D|--3------|  \n  A|--4------|  \n  E|--5------|  \n";

    #[test]
    fn literal_tabs_snapshot_preview_export_and_book_are_consistent() {
        let mut session = Session::new(TAB).unwrap();
        assert!(session.snapshot().has_tablature);
        assert!(session.snapshot().warnings.is_empty());
        assert_eq!(session.snapshot().pending_flags, 6);
        let preview = session.preview_rows();
        assert_eq!(
            preview
                .iter()
                .filter(|r| r.style == "tablature")
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            TAB.lines().collect::<Vec<_>>()
        );
        let body = session.export_tex(false).unwrap();
        assert_eq!(session.book_parts().unwrap().body, body);
        assert!(session.book_parts().unwrap().chords.is_empty());
        session.handle_action("confirm").unwrap();
        session.handle_action("auto_accept").unwrap();
        assert_eq!(session.export_tex(false).unwrap(), body);
    }

    #[test]
    fn manual_tab_annotation_action_is_ferried_with_provenance_and_split_preview() {
        let raw = format!("[Verse]\n[C]Heading\n(fingering)\n{TAB}");
        let mut session = Session::new(&raw).unwrap();
        session.handle_action("confirm").unwrap();
        let heading = index_of(&session.snapshot(), "[C]Heading");
        let comment = index_of(&session.snapshot(), "(fingering)");
        assert_eq!(session.snapshot().position, comment);

        // An ordinary intervening comment cannot be crossed or absorbed.
        session.handle_action("prev").unwrap();
        assert_eq!(session.snapshot().position, heading);
        let before = session.snapshot().lines;
        session.handle_action("tab_annotation").unwrap();
        assert_eq!(session.snapshot().lines, before);
        assert!(session
            .snapshot()
            .message
            .as_deref()
            .unwrap()
            .contains("immediately before"));

        // Associate the nearest comment first; the action advances one row,
        // so explicitly navigate back twice to associate the earlier heading.
        session.handle_action("next").unwrap();
        assert_eq!(session.snapshot().position, comment);
        session.handle_action("tab_annotation").unwrap();
        assert_eq!(session.snapshot().lines[heading].tab_role, None);
        session.handle_action("prev").unwrap();
        session.handle_action("prev").unwrap();
        assert_eq!(session.snapshot().position, heading);
        session.handle_action("tab_annotation").unwrap();
        let snapshot = session.snapshot();
        let string = &snapshot.lines[comment + 1];
        assert_eq!(string.tab_role.as_deref(), Some("string"));
        assert!(string.tab_origin.is_some());
        assert!(string.tab_instance.is_some());
        for (index, text) in [(heading, "[C]Heading"), (comment, "(fingering)")] {
            let annotation = &snapshot.lines[index];
            assert_eq!(annotation.text, text);
            assert_eq!(annotation.source_line, index + 1);
            assert_eq!(annotation.tab_role.as_deref(), Some("annotation"));
            assert_eq!(annotation.tab_origin, string.tab_origin);
            assert_eq!(annotation.tab_instance, string.tab_instance);
            assert_eq!(annotation.effective_label, "tab annotation");
        }
        let literal: Vec<_> = session
            .preview_rows()
            .into_iter()
            .filter(|row| row.style == "tab-annotation" || row.style == "tablature")
            .collect();
        assert_eq!(literal[0].style, "tab-annotation");
        assert_eq!(literal[1].style, "tab-annotation");
        assert_eq!(
            literal
                .iter()
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>(),
            ["[C]Heading", "(fingering)"]
                .into_iter()
                .chain(TAB.lines())
                .collect::<Vec<_>>()
        );

        let split = session.plan_tab_split(10).unwrap();
        assert_eq!(
            session.snapshot(),
            snapshot,
            "planning must not mutate review"
        );
        assert_eq!(split.bounds.annotation_count, 2);
        assert_eq!(
            (split.bounds.start, split.bounds.end),
            (heading, comment + 7)
        );
        assert_eq!(split.bounds.origin, string.tab_origin.unwrap());
        assert_eq!(split.bounds.instance, string.tab_instance.unwrap());
        assert_eq!(split.annotations.len(), 2);
        for (row, index) in split.annotations.iter().zip([heading, comment]) {
            let text = &snapshot.lines[index].text;
            assert_eq!(row.source_line, index + 1);
            assert_eq!(row.original_row, index - heading + 1);
            assert_eq!(row.left, text[..10.min(text.len())]);
            assert_eq!(row.right, format!("    {}", text.get(10..).unwrap_or("")));
            assert!(row.prefix.is_empty());
            assert_eq!(row.padding_columns, 4);
        }
        assert!(split
            .warnings
            .iter()
            .any(|warning| warning.contains("annotations appear above both")));

        let applied = session.apply_tab_split().unwrap();
        let annotations: Vec<_> = applied
            .lines
            .iter()
            .filter(|line| line.tab_role.as_deref() == Some("annotation"))
            .collect();
        assert_eq!(annotations.len(), 4);
        for (child, rows) in annotations.chunks_exact(2).enumerate() {
            for (index, line) in rows.iter().enumerate() {
                let row = &split.annotations[index];
                assert_eq!(&line.text, if child == 0 { &row.left } else { &row.right });
                assert_eq!(line.source_line, row.source_line);
                assert_eq!(line.tab_origin, string.tab_origin);
                assert_ne!(line.tab_instance, string.tab_instance);
                assert_eq!(line.tab_instance, rows[0].tab_instance);
            }
        }
        assert_ne!(annotations[0].tab_instance, annotations[2].tab_instance);
        assert_eq!(
            session
                .preview_rows()
                .into_iter()
                .filter(|row| row.style == "tab-annotation")
                .map(|row| row.text)
                .collect::<Vec<_>>(),
            annotations
                .iter()
                .map(|line| line.text.clone())
                .collect::<Vec<_>>()
        );
        session.handle_action("undo").unwrap();
        assert_eq!(session.snapshot().lines, snapshot.lines);
    }

    #[test]
    fn tab_warning_follows_both_session_and_action_offsets_and_reset() {
        use songbook_core::tablature::UNCHANGED_WARNING;
        let mut session = Session::new(&format!("[Verse]\n[C]Words\n{TAB}")).unwrap();
        let original_tabs: Vec<_> = session
            .preview_rows()
            .into_iter()
            .filter(|r| r.style == "tablature")
            .collect();
        session.transpose(2).unwrap();
        assert_eq!(session.snapshot().warnings, [UNCHANGED_WARNING]);
        assert!(session
            .export_tex(false)
            .unwrap()
            .contains(UNCHANGED_WARNING));
        assert!(session
            .preview_rows()
            .iter()
            .any(|r| r.text == UNCHANGED_WARNING));
        assert_eq!(
            session
                .preview_rows()
                .into_iter()
                .filter(|r| r.style == "tablature")
                .collect::<Vec<_>>(),
            original_tabs
        );
        session.transpose(-2).unwrap();
        assert!(session.snapshot().warnings.is_empty());
        session.handle_action("transpose_up").unwrap();
        assert_eq!(session.snapshot().transpose, 1);
        assert_eq!(session.snapshot().warnings, [UNCHANGED_WARNING]);
        session.handle_action("transpose_reset").unwrap();
        assert!(session.snapshot().warnings.is_empty());
    }

    #[test]
    fn partial_tabs_warn_and_discard_all_clears_presence_and_transpose_warning() {
        let mut session = Session::new(TAB).unwrap();
        session.handle_action("confirm").unwrap();
        session.handle_action("discard").unwrap();
        assert!(session.snapshot().warnings[0].contains("5 retained rows"));
        assert!(session
            .export_tex(false)
            .unwrap()
            .contains("Partial six-string"));
        session.handle_action("undo").unwrap();
        assert!(session.snapshot().warnings.is_empty());
        session.transpose(3).unwrap();
        for _ in 0..6 {
            session.handle_action("discard").unwrap();
        }
        assert!(!session.snapshot().has_tablature);
        assert!(session.snapshot().warnings.is_empty());
        assert!(!session
            .preview_rows()
            .iter()
            .any(|r| r.style == "tablature"));
    }

    #[test]
    fn unsupported_and_wide_tabs_preview_but_export_and_book_fail_honestly() {
        for raw in [
            "e|--0--|\n".repeat(4),
            format!("e|{}\n", "-".repeat(55)).repeat(6),
        ] {
            let session = Session::new(&raw).unwrap();
            assert!(session.snapshot().has_tablature);
            assert!(!session.snapshot().warnings.is_empty());
            assert!(session
                .preview_rows()
                .iter()
                .any(|r| r.style == "tablature"));
            assert!(session.export_tex(false).is_err());
            assert!(session.book_parts().is_err());
        }
    }

    #[test]
    fn core_validation_rejects_stale_stored_plans_and_consumes_them() {
        // Bypass adapter invalidation deliberately: the opaque plan must still
        // be guarded by core's revision and captured selection/data validation.
        for mutation in 0..4 {
            let mut session = Session::new(TAB).unwrap();
            session.handle_action("confirm").unwrap();
            session.plan_tab_split(10).unwrap();
            match mutation {
                0 => session.state.position = 1,
                1 => session.state.lines[2].edited = Some("G|--edited--|".into()),
                2 => {
                    session.state.handle(Key::Next);
                    session.state.handle(Key::Prev);
                }
                _ => session.state.lines[2].verdict = songbook_core::triage::Verdict::Discard,
            }
            let before = session.snapshot();
            let error = session.apply_tab_split().unwrap_err();
            assert!(
                matches!(error, SessionError::TabSplit(ref message) if message.contains("stale"))
            );
            assert_eq!(session.snapshot(), before);
            assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
            session.handle_action("undo").unwrap();
            assert_eq!(
                session.snapshot().message.as_deref(),
                Some("Nothing to undo.")
            );
            assert_eq!(session.snapshot().lines, before.lines);
        }
    }

    fn session() -> Session {
        Session::new(RAW).unwrap()
    }

    fn index_of(snapshot: &SessionSnapshot, needle: &str) -> usize {
        snapshot
            .lines
            .iter()
            .find(|line| line.text.contains(needle))
            .unwrap_or_else(|| panic!("no line contains {needle:?}"))
            .index
    }

    #[test]
    fn new_session_exposes_metadata_and_every_raw_line() {
        let session = session();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.phase, "metadata");
        assert_eq!(snapshot.line_count, RAW.lines().count());
        assert_eq!(snapshot.metadata.title, "Test");
        assert_eq!(snapshot.metadata.artist, "Band");
        assert_eq!(snapshot.metadata.fields.len(), 5);
        assert_eq!(snapshot.flag_count, 1);
        assert_eq!(snapshot.pending_flags, 1);
        let flag = snapshot.lines.iter().find(|line| line.flag).unwrap();
        assert_eq!(flag.text, "mystery line");
        assert_eq!(flag.classifier, "ambiguous");
        assert_eq!(flag.effective_label, "ambiguous");
        assert_eq!(flag.verdict, "pending");
        assert_eq!(flag.status, "pending");
        assert!(snapshot.lines.iter().any(|line| line.status == "kept"));
    }

    #[test]
    fn line_labels_are_effective_with_honest_status() {
        let raw = "Song: Test\nArtist: Band\n[Verse]\n[Am]Hello\nmystery line\n\nF#m:   244222\n";
        let mut session = Session::new(raw).unwrap();
        session.handle_action("confirm").unwrap();
        let snapshot = session.snapshot();

        // Pending ambiguous: the label is the parser's guess as-is — nothing is
        // decided, so it equals the classifier and no "(was: …)" suffix appears.
        let flag = &snapshot.lines[index_of(&snapshot, "mystery line")];
        assert_eq!(flag.verdict, "pending");
        assert_eq!(flag.effective_label, "ambiguous");
        assert_eq!(flag.classifier, "ambiguous");
        assert_eq!(flag.status, "pending");

        // Kept lyric: the label equals the parser's label (no suffix case).
        let lyric = &snapshot.lines[index_of(&snapshot, "[Am]Hello")];
        assert_eq!(lyric.effective_label, "lyric");
        assert_eq!(lyric.classifier, "lyric");
        assert_eq!(lyric.status, "kept");

        // Preamble metadata is consumed (nothing exports), but the label
        // keeps the parser's name; the "preamble" status explains why.
        let title = &snapshot.lines[index_of(&snapshot, "Song: Test")];
        assert!(title.preamble);
        assert_eq!(title.effective_label, "noise");
        assert_eq!(title.classifier, "noise");
        assert_eq!(title.status, "preamble");
    }

    #[test]
    fn kept_blank_and_noise_lines_report_dropped_status() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\n\nF#m:   244222\nmystery line";
        let mut session = Session::new(raw).unwrap();
        session.handle_action("confirm").unwrap();
        let snapshot = session.snapshot();

        // Keep on blank/noise-kind lines exports nothing: status and label
        // both read dropped instead of the dishonest kept/✓ pairing.
        let blank = snapshot
            .lines
            .iter()
            .find(|line| line.classifier == "blank" && !line.preamble)
            .unwrap();
        assert_eq!(blank.verdict, "keep");
        assert_eq!(blank.effective_label, "dropped");
        assert_eq!(blank.status, "dropped");

        let definition = snapshot
            .lines
            .iter()
            .find(|line| line.text.contains("F#m:"))
            .unwrap();
        assert!(!definition.preamble);
        assert_eq!(definition.classifier, "noise");
        assert_eq!(definition.verdict, "keep");
        assert_eq!(definition.effective_label, "dropped");
        assert_eq!(definition.status, "dropped");
    }

    #[test]
    fn explicit_picks_discard_and_undo_label_honestly() {
        for (action, label, status) in [
            ("lyric", "lyric", "kept"),
            ("chord_only", "chord-only", "kept"),
            ("section_header", "section header", "kept"),
            ("noise", "noise", "dropped"),
            ("discard", "discarded", "dropped"),
        ] {
            let mut session = session();
            session.handle_action("confirm").unwrap();
            session.handle_action(action).unwrap();
            let snapshot = session.snapshot();
            let line = &snapshot.lines[index_of(&snapshot, "mystery line")];
            assert_eq!(line.effective_label, label, "{action}");
            assert_eq!(line.status, status, "{action}");
        }

        // The label derives from the verdict, not stored state: undo reverts
        // the label with the decision.
        let mut session = session();
        session.handle_action("confirm").unwrap();
        let index = index_of(&session.snapshot(), "mystery line");
        session.handle_action("discard").unwrap();
        assert_eq!(session.snapshot().lines[index].effective_label, "discarded");
        session.handle_action("undo").unwrap();
        let line = &session.snapshot().lines[index];
        assert_eq!(line.verdict, "pending");
        assert_eq!(line.effective_label, "ambiguous");
        assert_eq!(line.status, "pending");
    }

    #[test]
    fn confirm_enters_triage_at_the_first_flag() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.phase, "triage");
        assert_eq!(snapshot.position, index_of(&snapshot, "mystery line"));
    }

    #[test]
    fn flag_jumps_and_line_navigation_move_position() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line\ne|---3---|";
        let mut session = Session::new(raw).unwrap();
        session.handle_action("confirm").unwrap();
        assert_eq!(
            session.snapshot().position,
            index_of(&session.snapshot(), "mystery line")
        );

        session.handle_action("next_flag").unwrap();
        assert_eq!(
            session.snapshot().position,
            index_of(&session.snapshot(), "e|---3---|")
        );
        session.handle_action("prev_flag").unwrap();
        assert_eq!(
            session.snapshot().position,
            index_of(&session.snapshot(), "mystery line")
        );

        session.handle_action("prev").unwrap();
        session.handle_action("prev").unwrap();
        assert_eq!(
            session.snapshot().position,
            index_of(&session.snapshot(), "[Verse]")
        );

        session.handle_action("prev_flag").unwrap();
        assert_eq!(
            session.snapshot().message.as_deref(),
            Some("No more flagged lines.")
        );
    }

    #[test]
    fn discard_and_undo_change_the_exported_song() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        session.handle_action("discard").unwrap();
        assert!(!session.export_tex(false).unwrap().contains("mystery line"));

        session.handle_action("undo").unwrap();
        assert!(session.export_tex(false).unwrap().contains("mystery line"));
    }

    #[test]
    fn book_parts_body_matches_export_tex_and_lists_distinct_chords() {
        let raw = "Song: Test\nArtist: Band\n[Verse]\n[Am]Hello [Am]there\nD  G  2x\nmystery line";
        let session = Session::new(raw).unwrap();
        let parts = session.book_parts().unwrap();
        assert_eq!(parts.body, session.export_tex(false).unwrap());
        assert!(
            parts.body.starts_with("\\beginsong{Test}"),
            "{}",
            parts.body
        );
        assert_eq!(parts.chords, ["Am", "D", "G"]);

        // Discarding the chord-only line removes its chords from the part.
        // The chord-followed text line is contextually a lyric (not a flag), so
        // navigate explicitly to the chord line instead of relying on flags.
        let mut session = Session::new(raw).unwrap();
        session.handle_action("confirm").unwrap();
        let chord_index = index_of(&session.snapshot(), "D  G  2x");
        for _ in 0..chord_index {
            session.handle_action("next").unwrap();
        }
        assert_eq!(session.snapshot().position, chord_index);
        session.handle_action("discard").unwrap();
        let parts = session.book_parts().unwrap();
        assert_eq!(parts.chords, ["Am"]);
    }

    #[test]
    fn editing_a_line_updates_preview_and_export_and_undo_restores_it() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        session.handle_action("edit").unwrap();
        assert!(session.snapshot().editing);
        session.insert_text(" (edited)").unwrap();
        session.handle_action("confirm").unwrap();

        let snapshot = session.snapshot();
        let line = &snapshot.lines[index_of(&snapshot, "mystery line")];
        assert!(line.edited);
        assert_eq!(line.text, "mystery line (edited)");
        assert_eq!(line.status, "edited");
        assert!(session
            .export_tex(false)
            .unwrap()
            .contains("mystery line (edited)"));
        assert!(session
            .preview_rows()
            .iter()
            .any(|row| row.text.contains("mystery line (edited)")));

        session.handle_action("undo").unwrap();
        assert!(!session.export_tex(false).unwrap().contains("(edited)"));
    }

    #[test]
    fn editing_can_be_cancelled() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        session.handle_action("edit").unwrap();
        session.insert_char("!").unwrap();
        session.handle_key("Escape").unwrap();
        let snapshot = session.snapshot();
        assert!(!snapshot.editing);
        assert_eq!(
            snapshot.lines[index_of(&snapshot, "mystery line")].text,
            "mystery line"
        );
    }

    #[test]
    fn preview_rows_and_export_are_available_before_finishing() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        session.handle_action("skip").unwrap();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.phase, "preview");

        let rows = session.preview_rows();
        assert!(rows.iter().any(|row| row.text == "Metadata"));
        assert!(rows
            .iter()
            .any(|row| row.style == "section" && row.text == "[Verse]"));
        assert!(rows.iter().any(|row| row.text.contains("[Am]Hello")));
        assert!(rows.iter().any(|row| row.text.contains("mystery line")));

        let tex = session.export_tex(false).unwrap();
        assert!(tex.contains("\\beginsong{Test}[by={Band}]"));
        assert!(tex.contains("\\[Am]Hello"));
        let standalone = session.export_tex(true).unwrap();
        assert!(standalone.starts_with("\\documentclass"));
    }

    #[test]
    fn preview_scroll_is_clamped() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        session.handle_action("skip").unwrap();
        for _ in 0..50 {
            session.handle_key("PageDown").unwrap();
        }
        let rows = session.preview_rows().len();
        assert_eq!(session.snapshot().preview_scroll, rows.saturating_sub(1));
        for _ in 0..50 {
            session.handle_key("PageUp").unwrap();
        }
        assert_eq!(session.snapshot().preview_scroll, 0);
    }

    #[test]
    fn metadata_editing_changes_export() {
        let mut session = session();
        session.handle_action("clear_field").unwrap();
        session.insert_text("New Title").unwrap();
        session.handle_action("tab").unwrap();
        session.handle_action("confirm").unwrap();
        assert!(session
            .export_tex(false)
            .unwrap()
            .contains("\\beginsong{New Title}[by={Band}]"));
    }

    #[test]
    fn text_insertion_requires_an_active_text_field() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        assert_eq!(session.insert_text("nope"), Err(SessionError::NotEditing));
        assert_eq!(session.insert_char("n"), Err(SessionError::NotEditing));
    }

    #[test]
    fn insert_char_requires_exactly_one_character() {
        let mut session = session();
        assert_eq!(
            session.insert_char("ab"),
            Err(SessionError::InvalidChar("ab".to_string()))
        );
        assert_eq!(
            session.insert_char(""),
            Err(SessionError::InvalidChar(String::new()))
        );
    }

    #[test]
    fn transpose_accumulates_deltas_and_every_view_follows() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nD  G  2x\n";
        let mut session = Session::new(raw).unwrap();

        session.transpose(2).unwrap();
        assert_eq!(session.snapshot().transpose, 2);
        let tex = session.export_tex(false).unwrap();
        assert!(tex.contains("\\[Bm]Hello"), "{tex}");
        assert!(tex.contains("\\[E] \\[A]"), "{tex}");
        assert!(session
            .preview_rows()
            .iter()
            .any(|row| row.text.contains("[Bm]Hello")));

        // +2 then −1 nets +1: the chord names shift again from the original.
        session.transpose(-1).unwrap();
        assert_eq!(session.snapshot().transpose, 1);
        let tex = session.export_tex(false).unwrap();
        assert!(tex.contains("\\[A#m]Hello"), "{tex}");
        assert!(tex.contains("\\[D#] \\[G#]"), "{tex}");
        assert!(session.book_parts().unwrap().chords == ["A#m", "D#", "G#"]);

        // Transposing back to zero restores the byte-exact original.
        session.transpose(-1).unwrap();
        assert_eq!(session.snapshot().transpose, 0);
        let plain = Session::new(raw).unwrap();
        assert_eq!(
            session.export_tex(false).unwrap(),
            plain.export_tex(false).unwrap()
        );
        assert_eq!(session.preview_rows(), plain.preview_rows());
    }

    #[test]
    fn transpose_survives_triage_edits_and_undo() {
        let mut session = session();
        session.handle_action("confirm").unwrap();
        session.handle_action("discard").unwrap(); // drop "mystery line"
        session.transpose(2).unwrap();
        let tex = session.export_tex(false).unwrap();
        assert!(tex.contains("\\[Bm]Hello"), "{tex}");
        assert!(!tex.contains("mystery line"), "{tex}");

        session.handle_action("undo").unwrap();
        assert_eq!(session.snapshot().transpose, 2);
        assert!(session.export_tex(false).unwrap().contains("mystery line"));
    }

    #[test]
    fn rejects_empty_and_sectionless_input() {
        assert_eq!(Session::new("").err(), Some(SessionError::EmptyInput));
        assert_eq!(
            Session::new("   \n\n").err(),
            Some(SessionError::EmptyInput)
        );
        assert_eq!(
            Session::new("Tabbed by: someone\nhttps://example.com/tab").err(),
            Some(SessionError::NoSections)
        );
    }

    #[test]
    fn rejects_unknown_actions_and_keys() {
        let mut session = session();
        assert_eq!(
            session.handle_action("bogus"),
            Err(SessionError::UnknownAction("bogus".to_string()))
        );
        assert_eq!(
            session.handle_key("bogus"),
            Err(SessionError::UnknownKey("bogus".to_string()))
        );
    }
}
