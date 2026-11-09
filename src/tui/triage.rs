use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use songbook_core::tablature::TabRowRole;
use songbook_core::triage::{line_kind_label, Phase, ReviewLine, TriageState, Verdict};

pub fn draw(frame: &mut Frame, state: &TriageState, confirm_quit: bool) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(4),
        ])
        .split(area);
    draw_metadata(frame, chunks[0], state);
    draw_main(frame, chunks[1], state);
    draw_footer(frame, chunks[2], state);
    if confirm_quit {
        draw_quit_confirm(frame, area);
    }
}

fn draw_metadata(frame: &mut Frame, area: Rect, state: &TriageState) {
    let focused = state.phase == Phase::Metadata;
    let mut lines = Vec::new();
    for index in 0..state.metadata.field_count() {
        let active = focused && index == state.metadata_field;
        let marker = if active { ">" } else { " " };
        let inferred = if state.metadata.field_inferred(index) {
            " (inferred)"
        } else {
            ""
        };
        let text = format!(
            "{} {:<7} {}{}",
            marker,
            state.metadata.field_label(index),
            state.metadata.field(index),
            inferred
        );
        let style = if active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }

    let title = if focused {
        "Metadata (Tab/Up/Down select · type to edit · Ctrl+U clear · Enter continue · Esc quit)"
    } else {
        "Metadata"
    };
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_main(frame: &mut Frame, area: Rect, state: &TriageState) {
    if state.phase == Phase::Triage {
        draw_triage_input(frame, area, state);
        return;
    }

    let title = match state.phase {
        Phase::Metadata => "Triage — review inferred metadata first".to_string(),
        Phase::Triage => "Triage".to_string(),
        Phase::Done => "Triage complete".to_string(),
        Phase::Preview => "Preview".to_string(),
        Phase::Quit => "Triage cancelled".to_string(),
    };

    let mut lines = Vec::new();
    match state.phase {
        Phase::Metadata => {
            lines.push(Line::from(
                "Check the inferred title/artist and optional key, capo, and tuning.",
            ));
            lines.push(Line::from(format!(
                "{} flagged line(s) waiting for review.",
                state.flags.len()
            )));
        }
        Phase::Triage => lines.push(Line::from("Input review")),
        Phase::Preview => {
            lines.push(Line::from(
                "Showing the structure preview screen (drawn by preview::draw).",
            ));
        }
        Phase::Done => {
            lines.push(Line::from(
                "Decisions applied; continuing to render/export.",
            ));
        }
        Phase::Quit => {
            lines.push(Line::from("Quit without saving."));
        }
    }

    if let Some(message) = &state.message {
        lines.push(Line::from(Span::styled(
            message.clone(),
            Style::default().fg(Color::Magenta),
        )));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_triage_input(frame: &mut Frame, area: Rect, state: &TriageState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(8)])
        .split(area);

    // "dot = preamble" instead of the marker itself (UX review F15): the
    // marker is a middot, so "… × dropped · · preamble" read as one stray
    // "· ·" separator collision.
    let title = format!(
        "Input · {} lines · {} flagged · {} pending · > current · ? pending · ✓ kept · × dropped · dot = preamble{}",
        state.lines.len(),
        state.flags.len(),
        state.pending_flags(),
        transpose_suffix(state)
    );
    // A shared prefix width keeps source columns aligned even when a row's
    // review status changes. The entire unwrapped line remains pannable.
    let label_width = state
        .lines
        .iter()
        .map(|line| {
            let was = if line.effective_label() == line_kind_label(&line.kind) {
                0
            } else {
                8 + line_kind_label(&line.kind).chars().count()
            };
            review_label(line).chars().count() + " · preamble".chars().count() + was + 2
        })
        .max()
        .unwrap_or(0);
    let label_width = if state
        .lines
        .iter()
        .any(|line| line.tab_provenance().is_some())
    {
        label_width
    } else {
        0
    };
    let lines: Vec<Line> = state
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let number = format!("{:>4} ", index + 1);
            let current = index == state.position;
            let (marker, status, color) = if line.verdict == Verdict::Pending {
                ("?", "pending", Color::Yellow)
            } else if matches!(line.verdict, Verdict::Discard | Verdict::Noise) {
                ("×", "dropped", Color::Red)
            } else if line.preamble && line.verdict == Verdict::Keep {
                ("·", "preamble", Color::DarkGray)
            } else if line.is_dropped() {
                // Keep on a blank/noise-kind line exports nothing: the same
                // honest ×/dropped the web status reports for it.
                ("×", "dropped", Color::Red)
            } else if line.edited.is_some() {
                ("✎", "edited", Color::Magenta)
            } else {
                ("✓", "kept", Color::Green)
            };
            let marker = if current { ">" } else { marker };
            let style = if current {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };
            // Label names what the line IS NOW; the muted "was" clause only
            // when it differs from the parser's label (UX advisor rule).
            let label = review_label(line);
            let classifier = line_kind_label(&line.kind);
            let was = if line.effective_label() == classifier {
                String::new()
            } else {
                format!(" · was: {classifier}")
            };
            Line::from(vec![
                Span::styled(number, Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{marker} "), style),
                Span::styled(
                    format!("{:<label_width$} ", format!("[{label} · {status}{was}]")),
                    Style::default().fg(color),
                ),
                Span::styled(line.text().to_string(), style),
            ])
        })
        .collect();
    let scroll = state.position.saturating_sub(4);
    let input = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((
            scroll.min(u16::MAX as usize) as u16,
            state.horizontal_scroll,
        ));
    frame.render_widget(input, chunks[0]);

    let mut details = if let Some(line) = state.current_line() {
        let flag_position = state
            .flags
            .iter()
            .position(|index| *index == state.position);
        let flag_text = match flag_position {
            Some(position) => {
                format!("Flag {}/{} · ", position + 1, state.flags.len())
            }
            None if line.preamble => "Preamble line · ".to_string(),
            None => String::new(),
        };
        let status = format!(
            "{flag_text}classifier: {} · verdict: {}",
            state.classifier_label(),
            state.verdict_label()
        );
        let mut details = vec![Line::from(status)];
        if let Some(source) = line.tab_provenance() {
            let role = match source.role() {
                TabRowRole::Annotation => format!("annotation {}", source.original_row()),
                TabRowRole::String => format!(
                    "string {}/{}",
                    source.original_row(),
                    source.original_rows()
                ),
            };
            details.push(Line::from(format!(
                "Source: fretboard · {role} · input line {} · source block line {}",
                source.source_line(),
                source.origin() + 1
            )));
        }
        details
    } else {
        vec![Line::from("No input lines."), Line::from("")]
    };
    details.extend(state.message.iter().cloned().map(Line::from));
    details.extend(
        songbook_core::tablature::warnings(&state.finish())
            .into_iter()
            .map(Line::from),
    );
    if !state.editing {
        details.push(Line::from(
            "t: explicitly attach selected adjacent comment to the following eligible six-string block; core rejects invalid associations.",
        ));
    }
    let selected = Block::default()
        .borders(Borders::ALL)
        .title("Selected line");
    let inner = selected.inner(chunks[1]);
    frame.render_widget(selected, chunks[1]);
    let detail_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(details).wrap(Wrap { trim: true }),
        detail_areas[0],
    );
    if let Some(line) = state.current_line() {
        let text = if state.editing {
            format!("Editing: {}█", state.edit_buffer)
        } else {
            format!("Line: {}", line.text())
        };
        // Literal source is never softly wrapped, including this duplicate
        // selected-row view. Append-only editing shows its tail and caret.
        let pan = if state.editing {
            (text
                .chars()
                .count()
                .saturating_sub(detail_areas[1].width as usize))
            .min(u16::MAX as usize) as u16
        } else {
            state.horizontal_scroll
        };
        frame.render_widget(Paragraph::new(text).scroll((0, pan)), detail_areas[1]);
    }
}

/// Keep the shared lowercase classifier vocabulary, adding stable tab source
/// role/ordinal from core provenance rather than inferring it from row text.
fn review_label(line: &ReviewLine) -> String {
    match line.tab_provenance() {
        Some(source) => match source.role() {
            TabRowRole::Annotation => {
                format!(
                    "{} · annotation {}",
                    line.effective_label(),
                    source.original_row()
                )
            }
            TabRowRole::String => format!(
                "{} · string {}/{}",
                line.effective_label(),
                source.original_row(),
                source.original_rows()
            ),
        },
        None => line.effective_label().to_string(),
    }
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &TriageState) {
    // Edit mode owns the footer while it is active (UX review F3/F4): the
    // triage keys are inert while typing, and the legend names the honest
    // limitation — the editor is append/backspace-only, no cursor model.
    let legend = if state.editing {
        "type to edit (append-only) · Backspace delete · Enter save · Esc cancel"
    } else {
        match state.phase {
            Phase::Metadata => {
                "Tab/Up/Down field · type edit · Ctrl+U clear · Enter continue · Esc quit · Ctrl+C quit"
            }
            Phase::Preview => {
                "j/k or Up/Down scroll · PgUp/PgDn page · Enter export · Esc/b back to triage · q quit"
            }
            // Same vocabulary as the web buttons (UX review F9): d discard,
            // a accept remaining. Transposition is preview-only (F10), so the
            // +/-/0 keys and their legend entry are gone from triage.
            _ => concat!(
                "F2 split tab block · t tab annotation · j/k line · Left/Right pan (no wrap) · f/F flag · PgUp/PgDn jump · Enter keep · d discard · l lyric · c chord · ",
                "h header · n noise · e edit · u undo · a accept remaining · s preview · q quit"
            ),
        }
    };
    let paragraph = Paragraph::new(legend)
        .block(Block::default().borders(Borders::ALL).title("Keys"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

/// Centered quit confirmation (UX review F1): the first q/Esc raises it, a
/// second consecutive quit-press quits, any other key continues. The copy
/// mirrors the web's start-over confirm ("All triage decisions for this song
/// will be lost").
fn draw_quit_confirm(frame: &mut Frame, area: Rect) {
    let width = 60.min(area.width);
    let height = 4.min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    let text = vec![
        Line::from(Span::styled(
            "Quit? All triage decisions for this song will be lost.",
            Style::default().fg(Color::Yellow),
        )),
        Line::from("q/Esc again to quit, any other key to continue."),
    ];
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Confirm quit")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

/// Persistent transposition indicator for block titles, empty at zero offset.
fn transpose_suffix(state: &TriageState) -> String {
    match state.transpose_label() {
        Some(label) => format!(" · {label}"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use songbook_core::parser::classify::parse;
    use songbook_core::triage::{Key, TriageState};

    fn rendered(state: &TriageState) -> String {
        rendered_at(state, 120, 40)
    }

    fn rendered_at(state: &TriageState, width: u16, height: u16) -> String {
        rendered_with(state, false, width, height)
    }

    fn rendered_with(state: &TriageState, confirm_quit: bool, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw(frame, state, confirm_quit))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn example_state() -> TriageState {
        let raw = "Song: Test\nArtist: Band\n[Verse]\n[Am]Hello\nmystery line";
        TriageState::new(parse(raw), raw)
    }

    #[test]
    fn draws_metadata_screen() {
        let state = example_state();
        let text = rendered(&state);
        assert!(text.contains("Metadata"));
        assert!(text.contains("Title"));
        assert!(text.contains("Artist"));
        assert!(text.contains("Keys"));
    }

    #[test]
    fn draws_full_input_with_flag_status() {
        let mut state = example_state();
        state.handle(Key::Confirm);
        let text = rendered(&state);
        assert!(text.contains("Input"));
        assert!(text.contains("Song: Test"));
        assert!(text.contains("[Verse]"));
        assert!(text.contains("[Am]Hello"));
        // Pending rows show the parser's guess as-is: no effective resolution,
        // no was-clause.
        assert!(
            text.contains("> [ambiguous · pending] mystery line"),
            "{text}"
        );
        // Preamble rows keep the classifier's name; the preamble status and
        // dim color carry the "consumed" explanation.
        assert!(text.contains("[noise · preamble] Song: Test"), "{text}");
        assert!(text.contains("classifier: ambiguous"));
        assert!(text.contains("f/F flag"));
    }

    #[test]
    fn dropped_and_overridden_rows_show_effective_labels() {
        let raw = "Song: T\n[Verse]\n[Am]Hello\n\nF#m:   244222\nmystery line";
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        let text = rendered(&state);
        // Kept blank/noise-kind lines export nothing: honest ×/dropped, with
        // the parser's label in the was clause.
        assert!(
            text.contains("× [dropped · dropped · was: noise] F#m"),
            "{text}"
        );
        assert!(
            text.contains("× [dropped · dropped · was: blank]"),
            "{text}"
        );

        state.handle(Key::Discard);
        let text = rendered(&state);
        assert!(
            text.contains("[discarded · dropped · was: ambiguous] mystery"),
            "{text}"
        );
    }

    #[test]
    fn moves_highlight_to_any_line() {
        let mut state = example_state();
        state.handle(Key::Confirm);
        state.handle(Key::Prev);
        let text = rendered(&state);
        assert!(text.contains("> [lyric · kept] [Am]Hello"));
    }

    #[test]
    fn page_keys_jump_through_long_input() {
        let body = (1..=30)
            .map(|index| format!("[Am] lyric line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let raw = format!("Song: Long\n[Verse]\nmystery line\n{body}");
        let mut state = TriageState::new(parse(&raw), &raw);
        state.handle(Key::Confirm);
        state.handle(Key::PageDown);
        let text = rendered_at(&state, 100, 26);
        assert_eq!(state.position, 12);
        assert!(text.contains("lyric line 10"));
    }

    #[test]
    fn source_rows_show_tab_role_ordinals_and_selected_source_details() {
        let raw = format!(
            "  Em A7  \n(022000) (x02220)\n{}",
            ["e", "B", "G", "D", "A", "E"]
                .map(|label| format!("{label}|{}|", "-".repeat(18)))
                .join("\n")
        );
        let mut state = TriageState::new(parse(&raw), &raw);
        state.handle(Key::Confirm);
        let text = rendered(&state);
        assert!(text.contains("tab annotation · annotation 1"), "{text}");
        assert!(text.contains("tab annotation · annotation 2"), "{text}");
        assert!(text.contains("fretboard · string 1/6"), "{text}");
        assert!(text.contains("fretboard · string 6/6"), "{text}");

        state.position = 0;
        let text = rendered(&state);
        assert!(
            text.contains("Source: fretboard · annotation 1 · input line 1"),
            "{text}"
        );
        assert!(text.contains("classifier: tab annotation"), "{text}");
        state.position = 2;
        let text = rendered(&state);
        assert!(
            text.contains("Source: fretboard · string 1/6 · input line 3"),
            "{text}"
        );
    }

    #[test]
    fn tab_roles_preserve_chosen_verdict_labels_and_shared_source_context() {
        let raw = format!(
            "Song: Synthetic\n[Verse]\n  Em A7  \n{}",
            "e|--0--|\n".repeat(6)
        );
        let mut state = TriageState::new(parse(&raw), &raw);
        state.handle(Key::Confirm);
        state.position = 2;
        state.handle(Key::TabAnnotation);
        state.position = 2;
        let annotation_source = state.lines[2].tab_provenance().unwrap();
        let string_source = state.lines[3].tab_provenance().unwrap();
        assert_eq!(annotation_source.origin(), string_source.origin());
        assert_eq!(annotation_source.instance(), string_source.instance());
        state.handle(Key::Lyric);
        assert_eq!(review_label(&state.lines[2]), "lyric · annotation 1");
        state.handle(Key::Discard);
        assert_eq!(review_label(&state.lines[3]), "discarded · string 1/6");
        state.position = 2;
        let text = rendered(&state);
        assert!(text.contains("lyric · annotation 1"), "{text}");
        assert!(text.contains("was: tab annotation"), "{text}");
        assert!(text.contains("source block line 4"), "{text}");
        assert!(text.contains("t tab annotation"), "{text}");
        assert!(
            text.contains("explicitly attach selected adjacent comment"),
            "{text}"
        );
    }

    #[test]
    fn annotation_and_string_literal_rows_share_columns_without_wrapping() {
        let raw = format!(
            "Song: Synthetic\n[Verse]\n  Em A7{}Dmaj7  \n{}",
            " ".repeat(100),
            "e|--0--|\n".repeat(6)
        );
        let mut state = TriageState::new(parse(&raw), &raw);
        state.handle(Key::Confirm);
        state.position = 2;
        state.handle(Key::TabAnnotation);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|frame| draw(frame, &state, false)).unwrap();
        let rows: Vec<String> = terminal
            .backend()
            .buffer()
            .content()
            .chunks(120)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect())
            .collect();
        let annotation = rows
            .iter()
            .find(|row| row.contains("[tab annotation"))
            .unwrap();
        let string = rows
            .iter()
            .find(|row| row.contains("[fretboard · string 1/6"))
            .unwrap();
        assert_eq!(
            annotation[..annotation.find("Em A7").unwrap()]
                .chars()
                .count()
                - 2,
            string[..string.find("e|--0--|").unwrap()].chars().count()
        );
        assert!(
            !rows.iter().any(|row| row.contains("Dmaj7")),
            "literal tail must not wrap"
        );
    }

    #[test]
    fn draws_editing_state() {
        let mut state = example_state();
        state.handle(Key::Confirm);
        state.handle(Key::Edit);
        let text = rendered(&state);
        assert!(text.contains("Editing:"));
    }

    #[test]
    fn input_title_shows_the_transposition_offset() {
        let mut state = example_state();
        state.handle(Key::Confirm);
        assert!(!rendered(&state).contains("Transposed"));
        state.handle(Key::TransposeUp);
        state.handle(Key::TransposeUp);
        let text = rendered(&state);
        assert!(text.contains("Transposed +2"), "{text}");

        state.handle(Key::TransposeReset);
        assert!(!rendered(&state).contains("Transposed"));
    }

    #[test]
    fn triage_legend_matches_the_web_vocabulary_and_omits_transpose() {
        let mut state = example_state();
        state.handle(Key::Confirm);
        let text = rendered(&state);
        // Web-aligned wording (UX review F9): d discard, a accept remaining.
        assert!(text.contains("d discard"), "{text}");
        assert!(text.contains("a accept remaining"), "{text}");
        assert!(!text.contains("d drop"), "{text}");
        assert!(!text.contains("accept rest"), "{text}");
        // Transposition is preview-only (UX review F10): no keys, no legend.
        assert!(!text.contains("transpose"), "{text}");
        assert!(!text.contains("0 reset"), "{text}");
    }

    #[test]
    fn editing_swaps_the_triage_legend_for_the_edit_legend() {
        let mut state = example_state();
        state.handle(Key::Confirm);
        let text = rendered(&state);
        assert!(text.contains("d discard"), "{text}");

        state.handle(Key::Edit);
        let text = rendered(&state);
        // UX review F3/F4: the triage keys are inert while typing, so the
        // footer names the editing keys and the append-only limitation.
        assert!(text.contains("type to edit (append-only)"), "{text}");
        assert!(text.contains("Backspace delete"), "{text}");
        assert!(text.contains("Enter save"), "{text}");
        assert!(text.contains("Esc cancel"), "{text}");
        assert!(!text.contains("d discard"), "{text}");
        assert!(!text.contains("f/F flag"), "{text}");
    }

    #[test]
    fn quit_confirm_renders_centered_with_the_web_copy() {
        let mut state = example_state();
        state.handle(Key::Confirm);
        let text = rendered_with(&state, true, 120, 40);
        assert!(text.contains("Quit?"), "{text}");
        assert!(
            text.contains("All triage decisions for this song will be lost"),
            "{text}"
        );
        assert!(text.contains("q/Esc again to quit"), "{text}");
        assert!(text.contains("any other key to continue"), "{text}");
        // The confirm title marks the popup.
        assert!(text.contains("Confirm quit"), "{text}");
    }
}
