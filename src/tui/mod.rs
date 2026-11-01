pub mod preview;
pub mod state;
pub mod triage;

use crate::parser::song::Song;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use state::{Key, Phase, TriageState};
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
    loop {
        terminal.draw(|frame| {
            if state.phase == Phase::Preview {
                preview::draw(frame, state, out_path);
            } else {
                triage::draw(frame, state);
            }
        })?;
        if matches!(state.phase, Phase::Done | Phase::Quit) {
            return Ok(());
        }
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            if let Some(mapped) = map_key(key, state) {
                state.handle(mapped);
            }
        }
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
        KeyCode::Char(ch) if !ctrl => Some(Key::Char(ch)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::latex::render::render;
    use crate::parser::classify::parse;

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
            map_key(key_event(KeyCode::Enter, KeyModifiers::NONE), &state),
            Some(Key::Confirm)
        );
    }
}
