pub mod preview;
mod split;
pub mod triage;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use songbook_core::parser::song::Song;
use songbook_core::triage::{Key, Phase, TriageState};
use std::io::{IsTerminal, Stdout};
use std::path::Path;

pub enum Outcome {
    Saved(Song),
    Quit,
}

pub fn run(song: Song, raw: &str, out_path: &Path, semitones: i32) -> anyhow::Result<Outcome> {
    run_with_terminal(
        song,
        raw,
        std::io::stdin().is_terminal(),
        out_path,
        semitones,
    )
}

pub(crate) fn run_with_terminal(
    song: Song,
    raw: &str,
    interactive: bool,
    out_path: &Path,
    semitones: i32,
) -> anyhow::Result<Outcome> {
    if !interactive {
        eprintln!("stdin is not a TTY; skipping triage and trusting heuristics (same as --yes).");
        return Ok(Outcome::Saved(song));
    }
    run_interactive(song, raw, out_path, semitones)
}

fn review_state(song: Song, raw: &str, semitones: i32) -> TriageState {
    let mut state = TriageState::new(song, raw);
    state.transpose = semitones;
    state
}

fn run_interactive(
    song: Song,
    raw: &str,
    out_path: &Path,
    semitones: i32,
) -> anyhow::Result<Outcome> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut state = review_state(song, raw, semitones);

    let loop_result = run_loop(&mut terminal, &mut state, out_path);

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
    loop_result?;

    match state.phase {
        Phase::Quit => Ok(Outcome::Quit),
        _ => Ok(Outcome::Saved(state.finish())),
    }
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut TriageState,
    out_path: &Path,
) -> anyhow::Result<()> {
    // Two-step quit (UX review F1): the TUI layer intercepts q/Esc before the
    // core sees them, so neither core nor the wasm session changes — the web
    // already guards the same action with its own confirm dialog.
    let mut quit_confirm = QuitConfirm::default();
    let mut split_modal = None;
    loop {
        let confirming = quit_confirm.active();
        let mut area = ratatui::layout::Rect::default();
        terminal.draw(|frame| {
            area = frame.area();
            if state.phase == Phase::Preview {
                preview::draw(frame, state, out_path);
            } else {
                triage::draw(frame, state, confirming);
            }
            if let Some(modal) = &split_modal {
                split::SplitModal::draw(modal, frame);
            }
        })?;
        if matches!(state.phase, Phase::Done | Phase::Quit) {
            return Ok(());
        }
        if let Event::Key(key) = event::read()? {
            handle_terminal_key(state, &mut quit_confirm, &mut split_modal, key, area);
        }
    }
}

/// Terminal-only controls precede shared key mapping. Modal keys must never
/// leak into quit confirmation or normal actions (which would stale the plan).
fn handle_terminal_key(
    state: &mut TriageState,
    quit_confirm: &mut QuitConfirm,
    split_modal: &mut Option<split::SplitModal>,
    key: KeyEvent,
    area: ratatui::layout::Rect,
) {
    if key.kind == KeyEventKind::Release {
        return;
    }
    if map_key(key, state) == Some(Key::CtrlC) {
        *split_modal = None;
        quit_confirm.active = false;
        state.handle(Key::CtrlC);
        return;
    }
    if let Some(modal) = split_modal {
        if modal.handle_key(key, state, area) {
            *split_modal = None;
        }
        return;
    }
    if key.code == KeyCode::F(2)
        && key.modifiers.is_empty()
        && state.phase == Phase::Triage
        && !state.editing
    {
        quit_confirm.active = false;
        *split_modal = Some(split::SplitModal::open(state));
        return;
    }
    // Some(true): second quit press; Some(false): consumed to raise confirm.
    // None: map normally, also cancelling any raised quit confirmation.
    match quit_confirm.handle_key(&key, state) {
        Some(true) => state.handle(Key::Quit),
        Some(false) => {}
        None => {
            if let Some(mapped) = map_key(key, state) {
                handle_review_key(state, mapped);
            }
        }
    }
}

/// Keep terminal review alive when export would fail. Validate the same core
/// song body before forwarding Confirm; standalone wrapping adds no further
/// semantic validation. Core/wasm confirmation semantics remain unchanged.
fn handle_review_key(state: &mut TriageState, key: Key) {
    if state.phase == Phase::Preview && key == Key::Confirm {
        if let Err(error) = songbook_core::latex::render::render(&state.finish(), false) {
            state.message = Some(format!(
                "Export blocked; press Esc to return to triage and Edit/Discard. {error}"
            ));
            return;
        }
        state.message = None;
    }
    state.handle(key);
}

/// Two-step quit (UX review F1): a single q/Esc in the metadata or triage
/// phase must not destroy every decision at once. The first press raises a
/// centered confirm; a second consecutive quit-press quits; any other key
/// cancels. Ctrl+C keeps its immediate semantics, edit mode owns its own keys
/// (Esc cancels the edit, letters type), and the preview phase is excluded —
/// Esc means "back to triage" there, and its q stays a direct quit.
#[derive(Debug, Default)]
struct QuitConfirm {
    active: bool,
}

impl QuitConfirm {
    /// Consumes a key event. Returns `Some(true)` to quit now, `Some(false)`
    /// when the quit-press was consumed to raise the confirm (the key must
    /// not also reach the core — Esc/q would quit directly), and `None` when
    /// the key is not a quit press and should be mapped normally.
    fn handle_key(&mut self, key: &KeyEvent, state: &TriageState) -> Option<bool> {
        if !is_quit_press(key, state) {
            self.active = false;
            return None;
        }
        if self.active {
            return Some(true);
        }
        self.active = true;
        Some(false)
    }

    fn active(&self) -> bool {
        self.active
    }
}

/// Whether `key` is a quit press in the current phase: `Esc`/`q` in triage,
/// only `Esc` in metadata (there `q` types into the field, so it must not
/// quit). Control-modified keys are never quit presses — Ctrl+C is handled
/// separately and keeps its immediate semantics.
fn is_quit_press(key: &KeyEvent, state: &TriageState) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) || state.editing {
        return false;
    }
    match state.phase {
        Phase::Metadata => key.code == KeyCode::Esc,
        Phase::Triage => matches!(key.code, KeyCode::Esc | KeyCode::Char('q')),
        _ => false,
    }
}

fn map_key(key: KeyEvent, state: &TriageState) -> Option<Key> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) {
        return Some(Key::CtrlC);
    }

    if state.phase == Phase::Metadata {
        return match key.code {
            KeyCode::Tab => Some(Key::Tab),
            KeyCode::BackTab => Some(Key::BackTab),
            KeyCode::Up => Some(Key::Prev),
            KeyCode::Down => Some(Key::Next),
            KeyCode::Enter => Some(Key::Confirm),
            KeyCode::Esc => Some(Key::Escape),
            KeyCode::Backspace => Some(Key::Backspace),
            KeyCode::Char('u') if ctrl => Some(Key::ClearField),
            KeyCode::Char(ch) if !ctrl => Some(Key::Char(ch)),
            _ => None,
        };
    }

    if state.editing {
        return match key.code {
            KeyCode::Enter => Some(Key::Confirm),
            KeyCode::Esc => Some(Key::Escape),
            KeyCode::Backspace => Some(Key::Backspace),
            KeyCode::Char(ch) if !ctrl => Some(Key::Char(ch)),
            _ => None,
        };
    }

    if state.phase == Phase::Preview {
        return match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(Key::Prev),
            KeyCode::Down | KeyCode::Char('j') => Some(Key::Next),
            KeyCode::Left => Some(Key::PanLeft),
            KeyCode::Right => Some(Key::PanRight),
            KeyCode::PageUp => Some(Key::PageUp),
            KeyCode::PageDown => Some(Key::PageDown),
            KeyCode::Char('+') | KeyCode::Char('=') => Some(Key::TransposeUp),
            KeyCode::Char('-') => Some(Key::TransposeDown),
            KeyCode::Char('0') => Some(Key::TransposeReset),
            KeyCode::Enter => Some(Key::Confirm),
            KeyCode::Esc | KeyCode::Char('b') => Some(Key::Back),
            KeyCode::Char('q') => Some(Key::Quit),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(Key::Prev),
        KeyCode::Down | KeyCode::Char('j') => Some(Key::Next),
        KeyCode::Char('f') => Some(Key::NextFlag),
        KeyCode::Char('F') => Some(Key::PrevFlag),
        KeyCode::Left => Some(Key::PanLeft),
        KeyCode::Right => Some(Key::PanRight),
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::Enter => Some(Key::Confirm),
        KeyCode::Char('d') => Some(Key::Discard),
        KeyCode::Char('l') => Some(Key::Lyric),
        KeyCode::Char('c') => Some(Key::ChordOnly),
        KeyCode::Char('h') => Some(Key::SectionHeader),
        KeyCode::Char('n') => Some(Key::Noise),
        KeyCode::Char('e') => Some(Key::Edit),
        KeyCode::Char('t') if !ctrl => Some(Key::TabAnnotation),
        KeyCode::Char('u') => Some(Key::Undo),
        KeyCode::Char('a') => Some(Key::AutoAccept),
        KeyCode::Char('s') => Some(Key::Skip),
        KeyCode::Char('q') | KeyCode::Esc => Some(Key::Quit),
        KeyCode::Backspace => Some(Key::Backspace),
        // Transposition is preview-only (UX review F10): inert in triage, and
        // the legend no longer advertises it. The preview branch below maps
        // and advertises the same keys.
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char('-') | KeyCode::Char('0')
            if !ctrl =>
        {
            None
        }
        KeyCode::Char(ch) if !ctrl => Some(Key::Char(ch)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use songbook_core::latex::render::render;
    use songbook_core::parser::classify::parse;

    fn terminal_press(
        state: &mut TriageState,
        quit: &mut QuitConfirm,
        modal: &mut Option<split::SplitModal>,
        code: KeyCode,
    ) {
        handle_terminal_key(
            state,
            quit,
            modal,
            KeyEvent::new(code, KeyModifiers::NONE),
            ratatui::layout::Rect::new(0, 0, 72, 24),
        );
    }

    fn split_state() -> TriageState {
        let raw = wide_six_string_tab();
        let mut state = review_state(parse(&raw), &raw, 2);
        state.handle(Key::Confirm);
        state
    }

    #[test]
    fn f2_eligibility_is_contextual_and_does_not_steal_edit_or_metadata_keys() {
        let raw = wide_six_string_tab();
        let mut state = review_state(parse(&raw), &raw, 0);
        let mut quit = QuitConfirm::default();
        let mut modal = None;
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
        assert!(modal.is_none());
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('q'));
        assert!(state.metadata.title.ends_with('q'));
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
        assert_eq!(state.phase, Phase::Triage);
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
        assert!(modal.is_some());
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Esc);
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('e'));
        let original = state.edit_buffer.clone();
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
        assert!(modal.is_none());
        for ch in ['a', 'q', 'j', 'k'] {
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char(ch));
        }
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
        assert!(!state.editing);
        assert_eq!(state.lines.len(), 6, "ordinary Enter saves, never splits");
        assert_eq!(
            state.current_line().unwrap().text(),
            format!("{original}aqjk")
        );
        assert_eq!(state.position, 0);
        assert!(!quit.active());
        state.handle(Key::Skip);
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
        assert!(modal.is_none(), "F2 is triage-only");
    }

    #[test]
    fn numeric_prompt_preview_and_escape_preserve_all_review_state_and_undo() {
        let mut state = split_state();
        state.handle(Key::Edit);
        state.handle(Key::Char(' '));
        state.handle(Key::Confirm);
        state.position = 2;
        state.horizontal_scroll = 16;
        state.preview_scroll = 4;
        state.message = Some("Existing review message".into());
        let lines = state.lines.clone();
        let flags = state.flags.clone();
        let metadata = state.metadata.clone();
        let mut quit = QuitConfirm::default();
        let mut modal = None;
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
        // Default 56 is a suggestion; replace using append/backspace only.
        for code in [
            KeyCode::Backspace,
            KeyCode::Backspace,
            KeyCode::Char('3'),
            KeyCode::Char('0'),
            KeyCode::Enter,
        ] {
            terminal_press(&mut state, &mut quit, &mut modal, code);
        }
        assert!(modal.is_some(), "first Enter only previews");
        assert_eq!(state.lines, lines);
        // Normal actions, letters and view changes stay locked in the modal.
        for code in [
            KeyCode::Char('d'),
            KeyCode::Char('u'),
            KeyCode::Char('a'),
            KeyCode::Char('s'),
            KeyCode::Char('q'),
            KeyCode::Right,
            KeyCode::Down,
            KeyCode::PageDown,
        ] {
            terminal_press(&mut state, &mut quit, &mut modal, code);
        }
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Esc);
        assert!(modal.is_none());
        assert!(!quit.active(), "Esc split must not raise quit confirmation");
        assert_eq!(state.phase, Phase::Triage);
        assert_eq!(state.lines, lines);
        assert_eq!(state.flags, flags);
        assert_eq!(state.metadata, metadata);
        assert_eq!(state.position, 2);
        assert_eq!(state.horizontal_scroll, 16);
        assert_eq!(state.preview_scroll, 4);
        assert_eq!(state.transpose, 2);
        assert_eq!(state.message.as_deref(), Some("Existing review message"));
        assert!(!state.editing);
        state.handle(Key::Undo);
        assert_eq!(state.lines.len(), 6);
        assert!(state.lines[0].edited.is_none(), "cancel adds no undo step");
    }

    #[test]
    fn explicit_apply_uses_stored_plan_focuses_right_and_undo_restores_structure() {
        let mut state = split_state();
        state.position = 3;
        state.horizontal_scroll = 8;
        let lines = state.lines.clone();
        let flags = state.flags.clone();
        let groups = state.finish().tab_groups;
        let mut quit = QuitConfirm::default();
        let mut modal = None;
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
        assert_eq!(state.lines.len(), 6);
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
        assert!(modal.is_none());
        assert_eq!(state.lines.len(), 12);
        assert_eq!(state.position, 6);
        assert_eq!(state.horizontal_scroll, 0);
        assert_eq!(state.finish().tab_groups.len(), 2);
        assert!(state
            .message
            .as_deref()
            .unwrap()
            .contains("focused first right row"));
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('u'));
        assert_eq!(state.lines, lines);
        assert_eq!(state.flags, flags);
        assert_eq!(state.position, 3);
        assert_eq!(state.horizontal_scroll, 8);
        assert_eq!(state.finish().tab_groups, groups);
        assert_eq!(state.transpose, 2, "transpose remains nonundoable");
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('u'));
        assert_eq!(state.message.as_deref(), Some("Nothing to undo."));
    }

    #[test]
    fn invalid_blocks_and_stale_apply_are_recoverable_without_mutation() {
        for raw in [
            "e|-----|\nB|-----|\nG|-----|\nD|-----|",
            "Song: Synthetic\n[C]words",
        ] {
            let mut state = review_state(parse(raw), raw, 0);
            state.handle(Key::Confirm);
            let lines = state.lines.clone();
            let mut quit = QuitConfirm::default();
            let mut modal = None;
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
            assert!(modal.is_some(), "ineligibility has a readable notice");
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Esc);
            assert_eq!(state.lines, lines);
            assert_eq!(state.phase, Phase::Triage);
            assert!(!quit.active());
            assert!(modal.is_none());
        }
        let mut state = split_state();
        let mut quit = QuitConfirm::default();
        let mut modal = None;
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
        // Defensive core stale validation even if a caller changes public state.
        state.position = 1;
        let lines = state.lines.clone();
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
        assert!(
            modal.is_some(),
            "failed Apply shows a notice, not a live plan"
        );
        assert_eq!(state.lines, lines);
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
        assert!(modal.is_none(), "notice Enter cannot reapply a stale plan");
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('e'));
        assert!(state.editing, "normal actions recover after error");
    }

    #[test]
    fn escape_in_prompt_cancels_before_quit_and_ctrl_c_remains_immediate() {
        for preview in [false, true] {
            let mut state = split_state();
            let mut quit = QuitConfirm::default();
            let mut modal = None;
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('q'));
            assert!(quit.active());
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
            assert!(!quit.active());
            if preview {
                terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
            }
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Esc);
            assert_eq!(state.phase, Phase::Triage);
            assert!(!quit.active());
            assert!(modal.is_none());
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
            if preview {
                terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Enter);
            }
            handle_terminal_key(
                &mut state,
                &mut quit,
                &mut modal,
                key_event(KeyCode::Char('c'), KeyModifiers::CONTROL),
                ratatui::layout::Rect::new(0, 0, 72, 24),
            );
            assert_eq!(state.phase, Phase::Quit);
            assert!(modal.is_none());
            assert!(!quit.active());
            assert_eq!(state.lines.len(), 6);
        }
    }

    #[test]
    fn initial_cli_offset_is_visible_in_review_and_reset_restores_original() {
        let raw = format!(
            "Song: Initial\n[Verse]\n[C]Words\n{}",
            "e|--0--12--|\n".repeat(6)
        );
        let mut state = review_state(parse(&raw), &raw, 2);
        assert_eq!(state.transpose, 2);
        assert!(songbook_core::tablature::warnings(&state.finish())
            .contains(&songbook_core::tablature::UNCHANGED_WARNING.to_string()));
        let tex = render(&state.finish(), false).unwrap();
        assert!(tex.contains("\\[D]Words"));
        state.handle(Key::Confirm);
        state.handle(Key::Skip);
        state.handle(Key::TransposeReset);
        assert!(songbook_core::tablature::warnings(&state.finish()).is_empty());
        assert!(render(&state.finish(), false)
            .unwrap()
            .contains("\\[C]Words"));
    }

    fn wide_six_string_tab() -> String {
        ["e", "B", "G", "D", "A", "E"]
            .map(|label| format!("{label}|{}|\n", "-".repeat(54)))
            .concat()
    }

    fn assert_blocked_preview_recovers_after_discard(tab: &str, rows: usize, reason: &str) {
        use songbook_core::triage::Verdict;

        let raw = format!("Song: Blocked review\n[Verse]\n[C]Words\n{tab}");
        let mut state = TriageState::new(parse(&raw), &raw);
        state.transpose = 2;
        assert_eq!(state.phase, Phase::Metadata);
        state.handle(Key::Confirm);
        assert_eq!(state.phase, Phase::Triage);
        assert_eq!(state.pending_flags(), rows);
        let position = state.position;
        let verdicts: Vec<_> = state.lines.iter().map(|line| line.verdict).collect();
        let original_rows = state.finish().tab_groups[0].original_rows;
        assert_eq!(original_rows, rows);

        // Skip previews without accepting pending tabs: they must still block export.
        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        handle_review_key(&mut state, Key::Confirm);
        assert_eq!(state.phase, Phase::Preview);
        assert_eq!(
            state
                .lines
                .iter()
                .map(|line| line.verdict)
                .collect::<Vec<_>>(),
            verdicts
        );
        assert_eq!(state.pending_flags(), rows);
        assert_eq!(state.position, position);
        assert_eq!(state.transpose, 2);
        assert_eq!(state.finish().tab_groups[0].original_rows, original_rows);
        let message = state.message.as_deref().expect("export-blocking message");
        assert!(message.contains("Export blocked"), "{message}");
        assert!(message.contains(reason), "{message}");
        assert!(message.contains("Esc"), "{message}");
        assert!(message.contains("Edit/Discard"), "{message}");

        handle_review_key(&mut state, Key::Escape);
        assert_eq!(state.phase, Phase::Triage);
        assert_eq!(state.position, position);
        assert_eq!(state.transpose, 2);
        for _ in 0..rows {
            state.handle(Key::Discard);
        }
        assert!(state.lines[position..]
            .iter()
            .all(|line| line.verdict == Verdict::Discard));
        assert!(state.finish().tab_groups.is_empty());
        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        handle_review_key(&mut state, Key::Confirm);
        assert_eq!(state.phase, Phase::Done);
        assert_eq!(state.message, None);
        assert_eq!(state.transpose, 2);
        assert!(render(&state.finish(), false)
            .unwrap()
            .contains("\\[D]Words"));
    }

    #[test]
    fn preview_export_guard_preserves_review_state_for_over_width_six_string_tab() {
        let tab = wide_six_string_tab();
        assert!(tab.lines().all(|line| line.len() == 57));
        let raw = format!("Song: Guarded review\n[Verse]\n[C]Words\n{tab}");
        let mut state = TriageState::new(parse(&raw), &raw);
        state.transpose = 2;
        state.handle(Key::Confirm);
        assert_eq!(state.phase, Phase::Triage);
        let position = state.position;
        let verdicts: Vec<_> = state.lines.iter().map(|line| line.verdict).collect();
        let offset = state.transpose;

        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        handle_review_key(&mut state, Key::Confirm);

        assert_eq!(state.phase, Phase::Preview);
        assert_eq!(state.position, position);
        assert_eq!(state.transpose, offset);
        assert_eq!(
            state
                .lines
                .iter()
                .map(|line| line.verdict)
                .collect::<Vec<_>>(),
            verdicts
        );
        let message = state.message.as_deref().expect("actionable export warning");
        assert!(message.contains("Export blocked"), "{message}");
        assert!(message.contains("57 columns, maximum 56"), "{message}");
        assert!(message.contains("Esc"), "{message}");
        assert!(message.contains("Edit/Discard"), "{message}");
    }

    #[test]
    fn preview_confirm_blocks_57_column_tabs_and_discard_recovers() {
        let tab = wide_six_string_tab();
        assert!(tab.lines().all(|line| line.len() == 57));
        assert_blocked_preview_recovers_after_discard(&tab, 6, "57 columns, maximum 56");
    }

    #[test]
    fn preview_confirm_blocks_original_four_row_run_and_discard_recovers() {
        let tab = "e|--0--|\nB|--1--|\nG|--2--|\nD|--3--|\n";
        assert_blocked_preview_recovers_after_discard(tab, 4, "4 rows are unsupported");
    }

    #[test]
    fn preview_confirm_recovers_after_editing_57_column_tabs_to_56() {
        let raw = format!(
            "Song: Edited review\n[Verse]\n[C]Words\n{}",
            wide_six_string_tab()
        );
        let mut state = TriageState::new(parse(&raw), &raw);
        state.transpose = 2;
        state.handle(Key::Confirm);
        assert_eq!(state.phase, Phase::Triage);
        assert_eq!(state.pending_flags(), 6);
        state.handle(Key::Skip);
        handle_review_key(&mut state, Key::Confirm);
        assert_eq!(state.phase, Phase::Preview);
        assert!(state
            .message
            .as_deref()
            .unwrap()
            .contains("57 columns, maximum 56"));

        handle_review_key(&mut state, Key::Escape);
        assert_eq!(state.phase, Phase::Triage);
        for _ in 0..6 {
            state.handle(Key::Edit);
            state.handle(Key::Backspace); // Remove the closing bar and one dash.
            state.handle(Key::Backspace);
            state.handle(Key::Char('|'));
            state.handle(Key::Confirm);
            assert!(!state.editing);
            assert_eq!(state.current_line().unwrap().original.len(), 57);
            assert_eq!(state.current_line().unwrap().text().len(), 56);
            state.handle(Key::Next);
        }
        assert_eq!(state.pending_flags(), 0);
        assert_eq!(state.finish().tab_groups[0].original_rows, 6);
        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        handle_review_key(&mut state, Key::Confirm);
        assert_eq!(state.phase, Phase::Done);
        assert_eq!(state.message, None);
        assert_eq!(state.transpose, 2);
        assert!(render(&state.finish(), false).unwrap().contains("\\hbox{"));
    }

    #[test]
    fn non_interactive_falls_back_to_heuristics() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let song = parse(raw);
        let expected = render(&song, false).unwrap();
        let outcome =
            run_with_terminal(song, raw, false, Path::new("/tmp/noninteractive.tex"), 0).unwrap();
        match outcome {
            Outcome::Saved(reviewed) => {
                assert_eq!(render(&reviewed, false).unwrap(), expected);
            }
            Outcome::Quit => panic!("non-interactive mode must not quit"),
        }
    }

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn horizontal_pan_is_view_only_and_never_steals_typing_or_navigation() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        for code in [KeyCode::Left, KeyCode::Right] {
            assert_eq!(map_key(key_event(code, KeyModifiers::NONE), &state), None);
        }
        state.handle(Key::Confirm);
        let position = state.position;
        let original = render(&state.finish(), false).unwrap();
        let right = map_key(key_event(KeyCode::Right, KeyModifiers::NONE), &state).unwrap();
        state.handle(right);
        assert_eq!(state.horizontal_scroll, 8);
        assert_eq!(state.position, position);
        assert_eq!(render(&state.finish(), false).unwrap(), original);
        state.handle(Key::Edit);
        assert_eq!(
            map_key(key_event(KeyCode::Right, KeyModifiers::NONE), &state),
            None
        );
        for ch in ['h', 'l', 'j', 'k', '+', '-', '0'] {
            assert_eq!(
                map_key(key_event(KeyCode::Char(ch), KeyModifiers::NONE), &state),
                Some(Key::Char(ch))
            );
        }
        state.handle(Key::PanRight);
        assert_eq!(state.horizontal_scroll, 8);
        state.handle(Key::Escape);
        state.handle(Key::Skip);
        assert_eq!(
            map_key(key_event(KeyCode::Left, KeyModifiers::NONE), &state),
            Some(Key::PanLeft)
        );
        state.handle(Key::PanLeft);
        state.handle(Key::PanLeft);
        assert_eq!(state.horizontal_scroll, 0);
        state.horizontal_scroll = u16::MAX;
        state.handle(Key::PanRight);
        assert_eq!(state.horizontal_scroll, u16::MAX);
    }

    #[test]
    fn metadata_mode_accepts_arrows_and_enter() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let state = TriageState::new(parse(raw), raw);
        assert_eq!(
            map_key(key_event(KeyCode::Down, KeyModifiers::NONE), &state),
            Some(Key::Next)
        );
        assert_eq!(
            map_key(key_event(KeyCode::Up, KeyModifiers::NONE), &state),
            Some(Key::Prev)
        );
        assert_eq!(
            map_key(key_event(KeyCode::Enter, KeyModifiers::NONE), &state),
            Some(Key::Confirm)
        );
    }

    #[test]
    fn triage_mode_maps_arrows_enter_and_quit() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        assert_eq!(state.phase, Phase::Triage);
        assert_eq!(
            map_key(key_event(KeyCode::Down, KeyModifiers::NONE), &state),
            Some(Key::Next)
        );
        assert_eq!(
            map_key(key_event(KeyCode::Enter, KeyModifiers::NONE), &state),
            Some(Key::Confirm)
        );
        assert_eq!(
            map_key(key_event(KeyCode::Char('q'), KeyModifiers::NONE), &state),
            Some(Key::Quit)
        );
        assert_eq!(
            map_key(key_event(KeyCode::Char('c'), KeyModifiers::CONTROL), &state),
            Some(Key::CtrlC)
        );
    }

    #[test]
    fn t_associates_adjacent_comment_and_f2_accepts_annotation_or_string_selection() {
        use songbook_core::parser::song::LineKind;

        let tab = ["e", "B", "G", "D", "A", "E"]
            .map(|label| format!("{label}|{}|", "-".repeat(38)))
            .join("\n");
        let raw = format!("Song: TUI annotation\n[Verse]\ncomment with a long literal tail\n{tab}");
        let mut state = review_state(parse(&raw), &raw, 0);
        state.handle(Key::Confirm);
        let comment = state
            .lines
            .iter()
            .position(|line| line.original == "comment with a long literal tail")
            .unwrap();
        state.position = comment;

        assert_eq!(
            map_key(key_event(KeyCode::Char('t'), KeyModifiers::NONE), &state),
            Some(Key::TabAnnotation),
            "t is a triage action"
        );
        let metadata = review_state(parse(&raw), &raw, 0);
        assert_eq!(
            map_key(key_event(KeyCode::Char('t'), KeyModifiers::NONE), &metadata),
            Some(Key::Char('t')),
            "metadata keeps t as text"
        );
        let mut quit = QuitConfirm::default();
        let mut modal = None;
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('t'));
        assert!(matches!(
            state.lines[comment].kind,
            LineKind::TabAnnotation(_)
        ));
        assert_eq!(state.position, comment + 1, "core keeps focus coherent");
        assert!(state.message.as_deref().unwrap().contains("associated"));

        for position in [comment, comment + 1] {
            state.position = position;
            let bounds = state
                .tab_split_bounds()
                .expect("annotation and string select the complete core block");
            assert_eq!(bounds.annotation_count, 1);
            assert_eq!(bounds.start, comment);
            assert_eq!(bounds.end - bounds.start, bounds.annotation_count + 6);
            let mut quit = QuitConfirm::default();
            let mut modal = None;
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::F(2));
            assert!(modal.is_some(), "F2 accepts selected row {position}");
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Esc);
            assert!(modal.is_none());
            assert_eq!(state.phase, Phase::Triage);
        }
    }

    #[test]
    fn invalid_t_association_does_not_mutate_review_or_add_undo() {
        for raw in [
            "Song: Synthetic\n[Verse]\ncomment\nseparator\ne|---|\nB|---|\nG|---|\nD|---|\nA|---|\nE|---|",
            "Song: Synthetic\n[Verse]\ncomment\ne|---|\nB|---|\nG|---|\nD|---|",
        ] {
            let mut state = review_state(parse(raw), raw, 0);
            state.handle(Key::Confirm);
            state.position = state.lines.iter().position(|line| line.original == "comment").unwrap();
            let lines = state.lines.clone();
            let flags = state.flags.clone();
            let position = state.position;
            let mut quit = QuitConfirm::default();
            let mut modal = None;
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('t'));
            assert_eq!(state.lines, lines);
            assert_eq!(state.flags, flags);
            assert_eq!(state.position, position);
            assert!(state.message.as_deref().unwrap().contains("complete original six-string"));
            terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('u'));
            assert_eq!(state.message.as_deref(), Some("Nothing to undo."));
        }
    }

    #[test]
    fn t_types_into_metadata_without_association() {
        let raw = "Song: Synthetic\n[Verse]\ncomment";
        let mut state = review_state(parse(raw), raw, 0);
        let lines = state.lines.clone();
        let before = state.metadata.title.clone();
        let mut quit = QuitConfirm::default();
        let mut modal = None;
        terminal_press(&mut state, &mut quit, &mut modal, KeyCode::Char('t'));
        assert_eq!(state.metadata.title, format!("{before}t"));
        assert_eq!(state.lines, lines);
        assert_eq!(state.phase, Phase::Metadata);
    }

    #[test]
    fn editing_mode_treats_letters_as_text() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        state.handle(Key::Edit);
        for ch in ['j', 't'] {
            assert_eq!(
                map_key(key_event(KeyCode::Char(ch), KeyModifiers::NONE), &state),
                Some(Key::Char(ch))
            );
        }
        let before = state.edit_buffer.clone();
        handle_review_key(&mut state, Key::Char('t'));
        assert_eq!(state.edit_buffer, format!("{before}t"));
        assert_eq!(
            map_key(key_event(KeyCode::Char('+'), KeyModifiers::NONE), &state),
            Some(Key::Char('+'))
        );
        assert_eq!(
            map_key(key_event(KeyCode::Enter, KeyModifiers::NONE), &state),
            Some(Key::Confirm)
        );
    }

    #[test]
    fn transpose_keys_are_inert_in_triage_and_mapped_in_preview() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        assert_eq!(state.phase, Phase::Triage);
        for code in [
            KeyCode::Char('+'),
            KeyCode::Char('='),
            KeyCode::Char('-'),
            KeyCode::Char('0'),
        ] {
            assert_eq!(
                map_key(key_event(code, KeyModifiers::NONE), &state),
                None,
                "transpose keys must be inert in triage (UX review F10)"
            );
        }

        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        for (code, expected) in [
            (KeyCode::Char('+'), Key::TransposeUp),
            (KeyCode::Char('='), Key::TransposeUp),
            (KeyCode::Char('-'), Key::TransposeDown),
            (KeyCode::Char('0'), Key::TransposeReset),
        ] {
            assert_eq!(
                map_key(key_event(code, KeyModifiers::NONE), &state),
                Some(expected)
            );
        }
    }

    #[test]
    fn quit_press_needs_a_second_consecutive_press_in_triage() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        let mut confirm = QuitConfirm::default();
        let q = key_event(KeyCode::Char('q'), KeyModifiers::NONE);
        // First press: consumed to raise the confirm; the key must not also
        // reach the core, where it would quit outright.
        assert_eq!(confirm.handle_key(&q, &state), Some(false));
        assert!(confirm.active());
        assert_eq!(state.phase, Phase::Triage, "the press was consumed");
        // Second consecutive quit press: quit.
        assert_eq!(confirm.handle_key(&q, &state), Some(true));

        // Esc counts as a quit press too, in either order.
        let mut confirm = QuitConfirm::default();
        let esc = key_event(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(confirm.handle_key(&esc, &state), Some(false));
        assert!(confirm.active());
        assert_eq!(confirm.handle_key(&q, &state), Some(true));

        // Any other key cancels a raised confirm instead of quitting, and is
        // mapped normally (None).
        let mut confirm = QuitConfirm::default();
        assert_eq!(confirm.handle_key(&q, &state), Some(false));
        assert!(confirm.active());
        let j = key_event(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(confirm.handle_key(&j, &state), None);
        assert!(!confirm.active());
    }

    #[test]
    fn metadata_quit_confirms_on_esc_but_q_types_into_the_field() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let state = TriageState::new(parse(raw), raw);
        let mut confirm = QuitConfirm::default();
        let q = key_event(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(
            confirm.handle_key(&q, &state),
            None,
            "q types into the metadata field"
        );
        assert!(!confirm.active());
        let esc = key_event(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(confirm.handle_key(&esc, &state), Some(false));
        assert!(confirm.active());
    }

    #[test]
    fn editing_preview_and_ctrl_c_are_not_quit_presses() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        state.handle(Key::Edit);
        let mut confirm = QuitConfirm::default();
        let esc = key_event(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(
            confirm.handle_key(&esc, &state),
            None,
            "Esc cancels the edit, not the session"
        );
        assert!(!confirm.active());

        state.handle(Key::Escape); // leave edit mode
        state.handle(Key::Skip);
        assert_eq!(state.phase, Phase::Preview);
        assert_eq!(
            confirm.handle_key(&esc, &state),
            None,
            "preview Esc means back to triage"
        );

        let ctrl_c = key_event(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(
            confirm.handle_key(&ctrl_c, &state),
            None,
            "Ctrl+C keeps its immediate semantics, not the confirm"
        );
        assert!(!confirm.active());
    }

    #[test]
    fn metadata_mode_types_transpose_characters() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let state = TriageState::new(parse(raw), raw);
        assert_eq!(
            map_key(key_event(KeyCode::Char('+'), KeyModifiers::NONE), &state),
            Some(Key::Char('+'))
        );
        assert_eq!(
            map_key(key_event(KeyCode::Char('-'), KeyModifiers::NONE), &state),
            Some(Key::Char('-'))
        );
        assert_eq!(
            map_key(key_event(KeyCode::Char('0'), KeyModifiers::NONE), &state),
            Some(Key::Char('0'))
        );
    }
}
