use crate::actions::{map_action, map_key};
use crate::error::SessionError;
use crate::model::{self, BookPartSnapshot, PreviewRowSnapshot, SessionSnapshot};
use songbook_core::book;
use songbook_core::latex::render::render;
use songbook_core::parser::classify::parse;
use songbook_core::transpose::transpose_song;
use songbook_core::triage::{Key, Phase, TriageState};

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
        })
    }

    /// Transposes the current triaged song by `delta` semitones (negative =
    /// down). Delta-based: +2 then −1 leaves a net +1. The offset applies to
    /// every derived view — snapshots, preview rows, exports, and book parts —
    /// and survives triage edits and undo, because the triage model keeps the
    /// untransposed raw lines and the offset is applied on top.
    pub fn transpose(&mut self, delta: i32) -> Result<(), SessionError> {
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
        self.state.handle(key);
        Ok(())
    }

    /// Applies a browser key name, respecting the current phase and edit mode.
    pub fn handle_key(&mut self, key: &str) -> Result<(), SessionError> {
        let key = map_key(&self.state, key)?;
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
        self.state.handle(Key::Char(ch));
        Ok(())
    }

    /// Inserts every character of `text` into the current edit buffer or
    /// metadata field.
    pub fn insert_text(&mut self, text: &str) -> Result<(), SessionError> {
        if !self.can_type() {
            return Err(SessionError::NotEditing);
        }
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
        assert_eq!(flag.verdict, "pending");
        assert_eq!(flag.status, "pending");
        assert!(snapshot.lines.iter().any(|line| line.status == "kept"));
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
        let mut session = Session::new(raw).unwrap();
        session.handle_action("confirm").unwrap();
        session.handle_action("prev").unwrap(); // onto "D  G  2x"
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
