pub mod preview;
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

pub fn run(song: Song, raw: &str, out_path: &Path) -> anyhow::Result<Outcome> {
    run_with_terminal(song, raw, std::io::stdin().is_terminal(), out_path)
}

pub(crate) fn run_with_terminal(
    song: Song,
    raw: &str,
    interactive: bool,
    out_path: &Path,
) -> anyhow::Result<Outcome> {
    if !interactive {
        eprintln!("stdin is not a TTY; skipping triage and trusting heuristics (same as --yes).");
        return Ok(Outcome::Saved(song));
    }
    run_interactive(song, raw, out_path)
}

fn run_interactive(song: Song, raw: &str, out_path: &Path) -> anyhow::Result<Outcome> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut state = TriageState::new(song, raw);

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
    loop {
        let confirming = quit_confirm.active();
        terminal.draw(|frame| {
            if state.phase == Phase::Preview {
                preview::draw(frame, state, out_path);
            } else {
                triage::draw(frame, state, confirming);
            }
        })?;
        if matches!(state.phase, Phase::Done | Phase::Quit) {
            return Ok(());
        }
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            // Some(true): second consecutive quit press — quit through the
            // core so every surface shares Phase::Quit semantics.
            // Some(false): the press was consumed to raise the confirm; it
            // must not also reach the core (Esc/q would quit directly).
            // None: not a quit press — map and handle it normally (this also
            // cancels a raised confirm).
            match quit_confirm.handle_key(&key, state) {
                Some(true) => {
                    state.handle(Key::Quit);
                    continue;
                }
                Some(false) => continue,
                None => {}
            }
            if let Some(mapped) = map_key(key, state) {
                state.handle(mapped);
            }
        }
    }
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
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::Enter => Some(Key::Confirm),
        KeyCode::Char('d') => Some(Key::Discard),
        KeyCode::Char('l') => Some(Key::Lyric),
        KeyCode::Char('c') => Some(Key::ChordOnly),
        KeyCode::Char('h') => Some(Key::SectionHeader),
        KeyCode::Char('n') => Some(Key::Noise),
        KeyCode::Char('e') => Some(Key::Edit),
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

    #[test]
    fn non_interactive_falls_back_to_heuristics() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let song = parse(raw);
        let expected = render(&song, false).unwrap();
        let outcome =
            run_with_terminal(song, raw, false, Path::new("/tmp/noninteractive.tex")).unwrap();
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
    fn editing_mode_treats_letters_as_text() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        state.handle(Key::Edit);
        assert_eq!(
            map_key(key_event(KeyCode::Char('j'), KeyModifiers::NONE), &state),
            Some(Key::Char('j'))
        );
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
