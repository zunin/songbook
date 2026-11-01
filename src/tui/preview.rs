use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use songbook_core::preview::{build, PreviewRow, RowStyle};
use songbook_core::triage::TriageState;
use std::path::Path;

fn row_style(style: RowStyle) -> Style {
    match style {
        RowStyle::Header => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        RowStyle::Meta => Style::default(),
        RowStyle::Section => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
        RowStyle::Lyric => Style::default(),
        RowStyle::Chord => Style::default().fg(Color::Green),
        RowStyle::Note => Style::default().fg(Color::Yellow),
        RowStyle::Blank => Style::default(),
    }
}

fn row_to_line(row: &PreviewRow) -> Line<'static> {
    Line::from(Span::styled(row.text.clone(), row_style(row.style)))
}

pub fn draw(frame: &mut Frame, state: &TriageState, out_path: &Path) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], out_path);
    draw_body(frame, chunks[1], state);
    draw_footer(frame, chunks[2]);
}

fn draw_header(frame: &mut Frame, area: Rect, out_path: &Path) {
    let lines = vec![
        Line::from("Structure preview — what the renderer will emit (not a PDF simulation)."),
        Line::from(Span::styled(
            format!("Output: {}", out_path.display()),
            Style::default().fg(Color::Magenta),
        )),
    ];
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_body(frame: &mut Frame, area: Rect, state: &TriageState) {
    let song = state.finish();
    let rows = build(&song, &state.metadata);
    let max_scroll = rows.len().saturating_sub(1);
    let scroll = state.preview_scroll.min(max_scroll);
    let lines: Vec<Line> = rows.iter().map(row_to_line).collect();
    let title = match state.transpose_label() {
        Some(label) => format!("Song structure (metadata + sections) · {label}"),
        None => "Song structure (metadata + sections)".to_string(),
    };
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((scroll as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let legend = concat!(
        "j/k or Up/Down scroll · PgUp/PgDn page · ",
        "+/- transpose · 0 reset · ",
        "Enter export · Esc/b back to triage · q quit"
    );
    let paragraph = Paragraph::new(legend)
        .block(Block::default().borders(Borders::ALL).title("Keys"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use songbook_core::parser::classify::parse;
    use songbook_core::preview::line_count;
    use songbook_core::triage::{Key, TriageState};

    fn rendered(state: &TriageState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let path = Path::new("/tmp/out.tex");
        terminal.draw(|frame| draw(frame, state, path)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn preview_state(raw: &str) -> TriageState {
        let mut state = TriageState::new(parse(raw), raw);
        state.handle(Key::Confirm);
        state.handle(Key::Skip);
        state
    }

    #[test]
    fn draws_preview_header_metadata_sections_and_output_path() {
        let state = preview_state(
            "Song: Test\nArtist: Band\nKey: D\nCapo: 2\n[Verse]\n[Am]Hello\nmystery line",
        );
        let text = rendered(&state, 120, 40);
        assert!(text.contains("Preview"));
        assert!(text.contains("not a PDF simulation"));
        assert!(text.contains("/tmp/out.tex"));
        assert!(text.contains("Title"));
        assert!(text.contains("Artist"));
        assert!(text.contains("[Verse]"));
        assert!(text.contains("[Am]Hello"));
        assert!(text.contains("Enter export"));
    }

    #[test]
    fn shows_inferred_metadata_hints() {
        let state = preview_state("\nLet It Be\nThe Beatles\n[Verse]\n[Am]Hello");
        let text = rendered(&state, 120, 40);
        assert!(text.contains("Let It Be"));
        assert!(text.contains("(inferred)"));
    }

    #[test]
    fn scrolling_reveals_later_content() {
        let numbered: Vec<String> = (1..=20).map(|n| format!("line {n:02}")).collect();
        let raw = format!("Song: Long\n[Verse]\n{}", numbered.join("\n"));
        let mut state = preview_state(&raw);
        let before = rendered(&state, 80, 20);
        assert!(before.contains("[Verse]"));

        state.handle(Key::PageDown);
        state.handle(Key::PageDown);
        let after = rendered(&state, 80, 20);
        assert!(state.preview_scroll > 0);
        assert!(after.contains("line 20"));
        assert!(!after.contains("[Verse]"));
    }

    #[test]
    fn page_down_scrolls_and_clamps() {
        let state = preview_state("Song: Long\n[Verse]\n[Am]one\n[Am]two");
        let count = line_count(&state.finish(), &state.metadata);
        let mut state = state;
        state.handle(Key::PageDown);
        assert!(state.preview_scroll > 0);
        for _ in 0..100 {
            state.handle(Key::PageDown);
        }
        assert_eq!(state.preview_scroll, count.saturating_sub(1));
        for _ in 0..100 {
            state.handle(Key::PageUp);
        }
        assert_eq!(state.preview_scroll, 0);
    }

    #[test]
    fn transposing_rerenders_chords_and_labels_the_offset() {
        let mut state = preview_state("Song: Test\n[Verse]\n[Am]Hello");
        state.handle(Key::TransposeUp);
        state.handle(Key::TransposeUp);
        let text = rendered(&state, 120, 40);
        assert!(text.contains("[Bm]Hello"), "{text}");
        assert!(text.contains("Transposed +2"), "{text}");
        assert!(text.contains("+/- transpose"), "{text}");

        state.handle(Key::TransposeReset);
        let text = rendered(&state, 120, 40);
        assert!(text.contains("[Am]Hello"), "{text}");
        assert!(!text.contains("Transposed"), "{text}");
    }
}
