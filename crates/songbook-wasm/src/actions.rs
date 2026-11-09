use crate::error::SessionError;
use songbook_core::triage::{Key, Phase, TriageState};

/// Maps a semantic browser action name to the core triage key.
///
/// Actions are phase-independent; `Session::handle_action` deliberately relies
/// on `TriageState::handle`, which ignores keys that do not apply to the
/// current phase. Use `map_key` for physical-key input, where the phase decides
/// whether a letter is navigation or text.
pub fn map_action(action: &str) -> Result<Key, SessionError> {
    let trimmed = action.trim();
    if trimmed == "F" {
        return Ok(Key::PrevFlag);
    }
    // "+" and "-" would both normalize to "_" (the replacement list includes
    // them), so the bare transposition symbols are matched before that step.
    match trimmed {
        "+" => return Ok(Key::TransposeUp),
        "-" => return Ok(Key::TransposeDown),
        "0" => return Ok(Key::TransposeReset),
        _ => {}
    }
    let normalized = trimmed.to_ascii_lowercase().replace(['-', ' ', '+'], "_");
    let key = match normalized.as_str() {
        "next" | "down" | "j" | "arrow_down" => Key::Next,
        "prev" | "previous" | "up" | "k" | "arrow_up" => Key::Prev,
        "next_flag" | "flag_next" | "f" => Key::NextFlag,
        "prev_flag" | "previous_flag" | "flag_prev" | "shift_f" => Key::PrevFlag,
        "confirm" | "enter" | "return" | "keep" => Key::Confirm,
        "discard" | "drop" | "d" => Key::Discard,
        "lyric" | "l" => Key::Lyric,
        "chord_only" | "chord" | "c" => Key::ChordOnly,
        "section_header" | "header" | "h" => Key::SectionHeader,
        "noise" | "n" => Key::Noise,
        "tab_annotation" => Key::TabAnnotation,
        "edit" | "e" => Key::Edit,
        "undo" | "u" => Key::Undo,
        "auto_accept" | "accept_rest" | "a" => Key::AutoAccept,
        "skip" | "preview" | "s" => Key::Skip,
        "quit" | "q" => Key::Quit,
        "escape" | "esc" => Key::Escape,
        "ctrl_c" | "ctrlc" | "control_c" => Key::CtrlC,
        "tab" => Key::Tab,
        "back_tab" | "backtab" | "shift_tab" => Key::BackTab,
        "backspace" => Key::Backspace,
        "clear_field" | "clear" | "ctrl_u" => Key::ClearField,
        "back" | "b" => Key::Back,
        "page_up" | "pageup" | "pgup" => Key::PageUp,
        "page_down" | "pagedown" | "pgdn" => Key::PageDown,
        "transpose_up" | "transpose_plus" | "plus" => Key::TransposeUp,
        "transpose_down" | "transpose_minus" | "minus" => Key::TransposeDown,
        "transpose_reset" | "reset_transpose" => Key::TransposeReset,
        _ => return Err(SessionError::UnknownAction(action.to_string())),
    };
    Ok(key)
}

/// Maps a browser key name to the core triage key, respecting the current phase
/// and edit mode. This mirrors the native TUI's key handling without depending
/// on crossterm.
pub fn map_key(state: &TriageState, key: &str) -> Result<Key, SessionError> {
    let trimmed = key.trim();
    let lower = trimmed.to_ascii_lowercase();
    if matches!(lower.as_str(), "ctrl+c" | "ctrlc" | "control+c") {
        return Ok(Key::CtrlC);
    }

    if state.phase == Phase::Metadata {
        return match lower.as_str() {
            "tab" => Ok(Key::Tab),
            "shift+tab" | "backtab" | "back_tab" => Ok(Key::BackTab),
            "up" | "arrowup" | "arrow_up" => Ok(Key::Prev),
            "down" | "arrowdown" | "arrow_down" => Ok(Key::Next),
            "enter" | "return" => Ok(Key::Confirm),
            "esc" | "escape" => Ok(Key::Escape),
            "backspace" => Ok(Key::Backspace),
            "ctrl+u" | "clear" | "clear_field" => Ok(Key::ClearField),
            _ => char_key(trimmed, key),
        };
    }

    if state.editing {
        return match lower.as_str() {
            "enter" | "return" => Ok(Key::Confirm),
            "esc" | "escape" => Ok(Key::Escape),
            "backspace" => Ok(Key::Backspace),
            _ => char_key(trimmed, key),
        };
    }

    if state.phase == Phase::Preview {
        return match lower.as_str() {
            "up" | "arrowup" | "arrow_up" | "k" => Ok(Key::Prev),
            "down" | "arrowdown" | "arrow_down" | "j" => Ok(Key::Next),
            "pageup" | "page_up" | "pgup" => Ok(Key::PageUp),
            "pagedown" | "page_down" | "pgdn" => Ok(Key::PageDown),
            "+" | "=" => Ok(Key::TransposeUp),
            "-" => Ok(Key::TransposeDown),
            "0" => Ok(Key::TransposeReset),
            "enter" | "return" => Ok(Key::Confirm),
            "esc" | "escape" | "b" => Ok(Key::Back),
            "q" => Ok(Key::Quit),
            _ => Err(SessionError::UnknownKey(key.to_string())),
        };
    }

    if trimmed == "F" || lower == "shift+f" {
        return Ok(Key::PrevFlag);
    }
    let mapped = match lower.as_str() {
        "up" | "arrowup" | "arrow_up" | "k" => Key::Prev,
        "down" | "arrowdown" | "arrow_down" | "j" => Key::Next,
        "f" => Key::NextFlag,
        "pageup" | "page_up" | "pgup" => Key::PageUp,
        "pagedown" | "page_down" | "pgdn" => Key::PageDown,
        "enter" | "return" => Key::Confirm,
        "d" => Key::Discard,
        "l" => Key::Lyric,
        "c" => Key::ChordOnly,
        "h" => Key::SectionHeader,
        "n" => Key::Noise,
        "e" => Key::Edit,
        "u" => Key::Undo,
        "a" => Key::AutoAccept,
        "s" => Key::Skip,
        "+" | "=" => Key::TransposeUp,
        "-" => Key::TransposeDown,
        "0" => Key::TransposeReset,
        "q" | "esc" | "escape" => Key::Quit,
        "backspace" => Key::Backspace,
        _ => return char_key(trimmed, key),
    };
    Ok(mapped)
}

fn char_key(trimmed: &str, original: &str) -> Result<Key, SessionError> {
    if trimmed == "space" {
        return Ok(Key::Char(' '));
    }
    let mut chars = trimmed.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(Key::Char(ch)),
        _ => Err(SessionError::UnknownKey(original.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use songbook_core::parser::classify::parse;

    const RAW: &str = "Song: Test\nArtist: Band\n[Verse]\n[Am]Hello\nmystery line";

    fn state(phase: Phase) -> TriageState {
        let mut state = TriageState::new(parse(RAW), RAW);
        match phase {
            Phase::Metadata => {}
            Phase::Triage => state.handle(Key::Confirm),
            Phase::Preview => {
                state.handle(Key::Confirm);
                state.handle(Key::Skip);
            }
            _ => panic!("unsupported test phase"),
        }
        state
    }

    #[test]
    fn maps_semantic_actions() {
        let cases = [
            ("next", Key::Next),
            ("down", Key::Next),
            ("prev", Key::Prev),
            ("next_flag", Key::NextFlag),
            ("prev_flag", Key::PrevFlag),
            ("F", Key::PrevFlag),
            ("confirm", Key::Confirm),
            ("discard", Key::Discard),
            ("lyric", Key::Lyric),
            ("chord-only", Key::ChordOnly),
            ("section header", Key::SectionHeader),
            ("noise", Key::Noise),
            ("tab_annotation", Key::TabAnnotation),
            ("edit", Key::Edit),
            ("undo", Key::Undo),
            ("auto_accept", Key::AutoAccept),
            ("skip", Key::Skip),
            ("preview", Key::Skip),
            ("quit", Key::Quit),
            ("escape", Key::Escape),
            ("ctrl+c", Key::CtrlC),
            ("back", Key::Back),
            ("page-up", Key::PageUp),
            ("page_down", Key::PageDown),
            ("transpose_up", Key::TransposeUp),
            ("transpose-plus", Key::TransposeUp),
            ("plus", Key::TransposeUp),
            ("+", Key::TransposeUp),
            ("transpose_down", Key::TransposeDown),
            ("transpose-minus", Key::TransposeDown),
            ("minus", Key::TransposeDown),
            ("-", Key::TransposeDown),
            ("transpose_reset", Key::TransposeReset),
            ("reset_transpose", Key::TransposeReset),
            ("0", Key::TransposeReset),
        ];
        for (action, expected) in cases {
            assert_eq!(map_action(action), Ok(expected), "action {action}");
        }
    }

    #[test]
    fn rejects_unknown_actions() {
        assert_eq!(
            map_action("definitely-not-an-action"),
            Err(SessionError::UnknownAction(
                "definitely-not-an-action".to_string()
            ))
        );
    }

    #[test]
    fn maps_metadata_keys() {
        let state = state(Phase::Metadata);
        assert_eq!(map_key(&state, "Tab"), Ok(Key::Tab));
        assert_eq!(map_key(&state, "shift+tab"), Ok(Key::BackTab));
        assert_eq!(map_key(&state, "Down"), Ok(Key::Next));
        assert_eq!(map_key(&state, "Up"), Ok(Key::Prev));
        assert_eq!(map_key(&state, "Enter"), Ok(Key::Confirm));
        assert_eq!(map_key(&state, "Esc"), Ok(Key::Escape));
        assert_eq!(map_key(&state, "ctrl+u"), Ok(Key::ClearField));
        assert_eq!(map_key(&state, "u"), Ok(Key::Char('u')));
        assert_eq!(map_key(&state, "A"), Ok(Key::Char('A')));
    }

    #[test]
    fn maps_triage_keys_and_flag_jumps() {
        let state = state(Phase::Triage);
        assert_eq!(map_key(&state, "j"), Ok(Key::Next));
        assert_eq!(map_key(&state, "k"), Ok(Key::Prev));
        assert_eq!(map_key(&state, "f"), Ok(Key::NextFlag));
        assert_eq!(map_key(&state, "F"), Ok(Key::PrevFlag));
        assert_eq!(map_key(&state, "PgDn"), Ok(Key::PageDown));
        assert_eq!(map_key(&state, "Enter"), Ok(Key::Confirm));
        assert_eq!(map_key(&state, "ctrl+c"), Ok(Key::CtrlC));
        assert_eq!(map_key(&state, "+"), Ok(Key::TransposeUp));
        assert_eq!(map_key(&state, "="), Ok(Key::TransposeUp));
        assert_eq!(map_key(&state, "-"), Ok(Key::TransposeDown));
        assert_eq!(map_key(&state, "0"), Ok(Key::TransposeReset));
        assert_eq!(map_key(&state, "x"), Ok(Key::Char('x')));
    }

    #[test]
    fn editing_mode_treats_letters_as_text() {
        let mut state = state(Phase::Triage);
        state.handle(Key::Edit);
        assert_eq!(map_key(&state, "j"), Ok(Key::Char('j')));
        assert_eq!(map_key(&state, "F"), Ok(Key::Char('F')));
        assert_eq!(map_key(&state, "Enter"), Ok(Key::Confirm));
        assert_eq!(map_key(&state, "Backspace"), Ok(Key::Backspace));
    }

    #[test]
    fn maps_preview_keys() {
        let state = state(Phase::Preview);
        assert_eq!(map_key(&state, "j"), Ok(Key::Next));
        assert_eq!(map_key(&state, "Up"), Ok(Key::Prev));
        assert_eq!(map_key(&state, "Enter"), Ok(Key::Confirm));
        assert_eq!(map_key(&state, "b"), Ok(Key::Back));
        assert_eq!(map_key(&state, "Esc"), Ok(Key::Back));
        assert_eq!(map_key(&state, "q"), Ok(Key::Quit));
        assert_eq!(map_key(&state, "+"), Ok(Key::TransposeUp));
        assert_eq!(map_key(&state, "="), Ok(Key::TransposeUp));
        assert_eq!(map_key(&state, "-"), Ok(Key::TransposeDown));
        assert_eq!(map_key(&state, "0"), Ok(Key::TransposeReset));
    }

    #[test]
    fn transpose_actions_drive_the_same_core_behavior() {
        let raw = "Song: Test\n[Verse]\n[Am]Hello";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        state.handle(map_action("transpose_up").unwrap());
        state.handle(map_action("+").unwrap());
        assert_eq!(state.transpose, 2);
        assert_eq!(state.transpose_label().as_deref(), Some("Transposed +2"));

        state.handle(map_action("0").unwrap());
        assert_eq!(state.transpose, 0);
        state.handle(map_action("-").unwrap());
        let song = state.finish();
        let tex = songbook_core::latex::render::render(&song, false).unwrap();
        assert!(tex.contains("\\[Abm]Hello"), "{tex}");
    }

    #[test]
    fn rejects_unknown_keys() {
        let state = state(Phase::Triage);
        assert_eq!(
            map_key(&state, "definitely-not-a-key"),
            Err(SessionError::UnknownKey("definitely-not-a-key".to_string()))
        );
        let preview = state_for_preview();
        assert_eq!(
            map_key(&preview, "not-a-preview-key"),
            Err(SessionError::UnknownKey("not-a-preview-key".to_string()))
        );
    }

    fn state_for_preview() -> TriageState {
        let mut state = state(Phase::Triage);
        state.handle(Key::Skip);
        state
    }
}
