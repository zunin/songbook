use super::*;
use ratatui::{backend::TestBackend, Terminal};
use songbook_core::{parser::classify::parse, triage::Key};

fn state(raw: &str) -> TriageState {
    let mut state = TriageState::new(parse(raw), raw);
    state.handle(Key::Confirm);
    state
}

fn press(
    modal: &mut SplitModal,
    state: &mut TriageState,
    code: KeyCode,
    width: u16,
    height: u16,
) -> bool {
    modal.handle_key(
        KeyEvent::new(code, KeyModifiers::NONE),
        state,
        Rect::new(0, 0, width, height),
    )
}

fn rendered(modal: &SplitModal, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| modal.draw(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

fn unequal_tab() -> String {
    [
        format!("  e|{}TAIL|  ", "-".repeat(130)),
        "B|--1--| ".into(),
        " G|--2--|  ".into(),
        "D|--3--| ".into(),
        " A|--4--|  ".into(),
        "E|--5--| ".into(),
    ]
    .join("\n")
}

#[test]
fn numeric_prompt_has_raw_counting_default_and_recoverable_validation() {
    let mut state = state(&unequal_tab());
    let lines = state.lines.clone();
    let mut modal = SplitModal::open(&state);
    let text = rendered(&modal, 72, 24);
    assert!(text.contains(PROMPT_TITLE), "{text}");
    assert!(text.contains("56_"), "{text}");
    assert!(text.contains("indentation"), "{text}");
    assert!(text.contains("string label and first pipe"), "{text}");
    assert!(text.contains("Enter Preview"), "{text}");
    assert!(!text.contains("Enter Apply"));
    // Non-digits never type or trigger ordinary actions; arrows cannot pan the
    // numeric field or core view. Backspace/append remains the only editor.
    for code in [KeyCode::Char('e'), KeyCode::Char('a'), KeyCode::Right] {
        assert!(!press(&mut modal, &mut state, code, 72, 24));
    }
    assert_eq!(modal.pan, 0);
    match &modal.stage {
        Stage::Prompt { input, .. } => assert_eq!(input, "56"),
        _ => panic!("numeric stage expected"),
    }
    for code in [KeyCode::Backspace, KeyCode::Backspace, KeyCode::Enter] {
        assert!(!press(&mut modal, &mut state, code, 72, 24));
    }
    let text = rendered(&modal, 72, 24);
    assert!(text.contains("Invalid column"), "{text}");
    assert!(text.contains("whole raw column number"), "{text}");
    for code in [KeyCode::Char('9'), KeyCode::Char('9'), KeyCode::Enter] {
        press(&mut modal, &mut state, code, 72, 24);
    }
    let text = rendered(&modal, 72, 24);
    assert!(text.contains("requested 99"), "{text}");
    for code in [
        KeyCode::Backspace,
        KeyCode::Backspace,
        KeyCode::Char('5'),
        KeyCode::Char('6'),
        KeyCode::Enter,
    ] {
        press(&mut modal, &mut state, code, 72, 24);
    }
    assert!(matches!(modal.stage, Stage::Preview(_)));
    assert_eq!(state.lines, lines, "preview never mutates review");
    assert!(rendered(&modal, 72, 24).contains("Enter Apply"));
}

#[test]
fn small_terminal_preview_scrolls_all_twelve_literal_rows_and_disclosures() {
    let mut state = state(&unequal_tab());
    let mut modal = SplitModal::open(&state);
    press(&mut modal, &mut state, KeyCode::Enter, 72, 14);
    let preview = match &modal.stage {
        Stage::Preview(plan) => plan.preview().clone(),
        _ => panic!("preview expected"),
    };
    // The document contains all raw rows, in core order, without trimming.
    let doc = modal.document(70);
    let texts: Vec<_> = doc.iter().map(Line::to_string).collect();
    let mut previous = 0;
    for literal in preview
        .rows
        .iter()
        .map(|row| &row.left)
        .chain(preview.rows.iter().map(|row| &row.right))
    {
        let position = texts
            .iter()
            .enumerate()
            .skip(previous)
            .find(|(_, text)| *text == literal)
            .unwrap()
            .0;
        previous = position + 1;
    }
    let original_lines = state.lines.clone();
    let mut all_screens = String::new();
    loop {
        let text = rendered(&modal, 72, 14);
        assert!(
            text.contains(CUT_WARNING),
            "warning must stay readable: {text}"
        );
        assert!(text.contains("Enter Apply"), "{text}");
        assert!(text.contains("Esc cancel"), "{text}");
        all_screens.push_str(&text);
        let previous = modal.scroll;
        press(&mut modal, &mut state, KeyCode::Down, 72, 14);
        if previous == modal.scroll {
            break;
        }
    }
    for phrase in [
        "LEFT six rows",
        "RIGHT six rows",
        "original 141 cols",
        "prefix 2 cols + padding 2 spaces",
        "label-only",
        "OVERFLOW",
        "Remaining overflow",
        "arbitrary cut",
        "fret number or technique",
        "no tails, dashes or notes",
    ] {
        assert!(
            all_screens.contains(phrase),
            "missing {phrase}: {all_screens}"
        );
    }
    assert_eq!(state.lines, original_lines);
    assert_eq!(state.position, 0);
    assert_eq!(state.horizontal_scroll, 0);
    for _ in 0..100 {
        press(&mut modal, &mut state, KeyCode::PageUp, 72, 14);
    }
    assert_eq!(modal.scroll, 0);
    assert!(rendered(&modal, 72, 14).contains("LEFT six rows"));
}

#[test]
fn annotation_rows_render_above_each_child_with_padding_disclosure_at_62_columns() {
    let raw = format!(
        "Song: TUI split\n[Verse]\ncomment with a long literal tail\n{}",
        unequal_tab()
    );
    let mut state = state(&raw);
    let annotation = state
        .lines
        .iter()
        .position(|line| line.original == "comment with a long literal tail")
        .unwrap();
    state.position = annotation;
    state.handle(Key::TabAnnotation);

    let mut modal = SplitModal::open(&state);
    for code in [
        KeyCode::Backspace,
        KeyCode::Backspace,
        KeyCode::Char('1'),
        KeyCode::Char('0'),
        KeyCode::Enter,
    ] {
        press(&mut modal, &mut state, code, 62, 14);
    }
    let preview = match &modal.stage {
        Stage::Preview(plan) => plan.preview(),
        _ => panic!("preview expected"),
    };
    assert_eq!(preview.annotations.len(), 1);
    assert_eq!(preview.bounds.annotation_count, preview.annotations.len());
    assert_eq!(
        preview.bounds.end - preview.bounds.start,
        preview.annotations.len() + 6
    );
    assert_eq!(preview.rows.len(), 6);
    assert!(preview
        .warnings
        .iter()
        .any(|warning| warning.contains("arbitrary cut")));

    let doc = modal.document(60);
    let texts: Vec<_> = doc.iter().map(Line::to_string).collect();
    let left_annotation = &preview.annotations[0].left;
    let left_string = &preview.rows[0].left;
    let right_annotation = &preview.annotations[0].right;
    let right_string = &preview.rows[0].right;
    let left_annotation_pos = texts
        .iter()
        .position(|text| text == left_annotation)
        .unwrap();
    let left_string_pos = texts.iter().position(|text| text == left_string).unwrap();
    let right_annotation_pos = texts
        .iter()
        .position(|text| text == right_annotation)
        .unwrap();
    let right_string_pos = texts.iter().position(|text| text == right_string).unwrap();
    assert!(left_annotation_pos < left_string_pos);
    assert!(left_string_pos < right_annotation_pos);
    assert!(right_annotation_pos < right_string_pos);
    assert!(texts
        .iter()
        .any(|text| text.contains("LEFT six rows: 1 annotation(s) first")));
    assert!(texts
        .iter()
        .any(|text| text.contains("RIGHT six rows: 1 annotation(s) first")));
    assert!(texts.join(" ").contains("prefix 0 cols + padding 4 spaces"));

    let mut all_screens = String::new();
    loop {
        let screen = rendered(&modal, 62, 14);
        all_screens.push_str(&screen);
        assert!(
            screen.contains("no musically safe cut")
                || all_screens.contains("no musically safe cut")
        );
        let previous = modal.scroll;
        press(&mut modal, &mut state, KeyCode::Down, 62, 14);
        if previous == modal.scroll {
            break;
        }
    }
    assert!(all_screens.contains("comment w"));
    assert!(all_screens.contains("no musically safe cut"));
}

#[test]
fn empty_annotation_fragment_is_labelled_outside_unchanged_literal_content() {
    let raw = format!("Song: Synthetic\n[Verse]\n  Em A7  \n{}", unequal_tab());
    let mut state = state(&raw);
    state.position = 2;
    state.handle(Key::TabAnnotation);
    let mut modal = SplitModal::open(&state);
    press(&mut modal, &mut state, KeyCode::Enter, 72, 24);
    let preview = match &modal.stage {
        Stage::Preview(plan) => plan.preview(),
        _ => panic!("preview expected"),
    };
    assert_eq!(preview.annotations.len(), 1);
    assert!(preview.annotations[0].right_label_only);
    let doc: Vec<_> = modal.document(200).iter().map(Line::to_string).collect();
    let label = doc
        .iter()
        .position(|text| text.starts_with("Empty annotation fragment:"))
        .unwrap();
    assert_eq!(doc[label + 1], preview.annotations[0].right);
    assert_eq!(
        doc[label + 1],
        " ".repeat(preview.annotations[0].padding_columns)
    );
}

#[test]
fn annotation_only_overflow_can_be_panned_and_keeps_remaining_overflow_warning() {
    let raw = format!(
        "Song: Synthetic\n[Verse]\ncomment {}TAIL  \n{}",
        "-".repeat(150),
        ["e", "B", "G", "D", "A", "E"]
            .map(|label| format!("{label}|---|"))
            .join("\n")
    );
    let mut state = state(&raw);
    state.position = state
        .lines
        .iter()
        .position(|line| line.original.starts_with("comment "))
        .unwrap();
    state.handle(Key::TabAnnotation);
    let mut modal = SplitModal::open(&state);
    press(&mut modal, &mut state, KeyCode::Enter, 72, 24);
    let preview = match &modal.stage {
        Stage::Preview(plan) => plan.preview(),
        _ => panic!("preview expected"),
    };
    assert!(preview
        .rows
        .iter()
        .all(|row| row.right_columns <= MAX_COLUMNS));
    assert!(preview.annotations[0].right_columns > MAX_COLUMNS);
    let doc: Vec<_> = modal.document(200).iter().map(Line::to_string).collect();
    assert!(doc
        .iter()
        .any(|text| text.starts_with("Remaining overflow:")));
    assert!(doc.iter().any(|text| text.contains("arbitrary cut")));
    for _ in 0..100 {
        press(&mut modal, &mut state, KeyCode::Right, 72, 24);
    }
    assert!(modal.pan > 0);
    let panned: Vec<_> = modal.document(70).iter().map(Line::to_string).collect();
    assert!(panned.iter().any(|text| text.ends_with("TAIL  ")));
    assert!(panned
        .iter()
        .any(|text| text.contains("Remaining overflow")));
}

#[test]
fn horizontal_pan_reveals_literal_tail_not_warnings_or_disclosures() {
    let mut state = state(&unequal_tab());
    let mut modal = SplitModal::open(&state);
    press(&mut modal, &mut state, KeyCode::Enter, 72, 24);
    // Scroll to just before the right child's first row.
    let lines: Vec<_> = modal.document(70).iter().map(Line::to_string).collect();
    modal.scroll = lines
        .iter()
        .position(|text| text.starts_with("RIGHT six"))
        .unwrap();
    let before = rendered(&modal, 72, 24);
    assert!(!before.contains("TAIL|"));
    for _ in 0..100 {
        press(&mut modal, &mut state, KeyCode::Right, 72, 24);
    }
    let after = rendered(&modal, 72, 24);
    assert!(
        after.contains("TAIL|  "),
        "literal trailing spaces survive: {after}"
    );
    assert!(after.contains(CUT_WARNING), "{after}");
    assert!(
        after.contains("prefix 4 cols + padding 0 spaces"),
        "{after}"
    );
    assert!(after.contains("RIGHT six rows"), "{after}");
    assert_eq!(state.horizontal_scroll, 0, "modal pan is local");
    for _ in 0..100 {
        press(&mut modal, &mut state, KeyCode::Left, 72, 24);
    }
    assert_eq!(modal.pan, 0);
    assert_eq!(rendered(&modal, 72, 24), before);
}

#[test]
fn invalid_block_notice_is_actionable_and_escape_cannot_quit() {
    let mut state = state("e|---|\nB|---|\nG|---|\nD|---|");
    let mut modal = SplitModal::open(&state);
    let text = rendered(&modal, 72, 24);
    for phrase in [
        "Split tab block unavailable",
        "original 4-row",
        "only complete",
        "e Edit",
        "u Undo",
        "d Discard",
        "Enter/Esc close",
    ] {
        assert!(text.contains(phrase), "missing {phrase}: {text}");
    }
    assert!(press(&mut modal, &mut state, KeyCode::Esc, 72, 24));
    assert_eq!(state.phase, songbook_core::triage::Phase::Triage);
    assert_eq!(state.lines.len(), 4);
}

#[test]
fn modal_draw_handles_tiny_dimensions_and_resize_without_panicking() {
    let mut state = state(&unequal_tab());
    let mut modal = SplitModal::open(&state);
    for preview in [false, true] {
        if preview {
            press(&mut modal, &mut state, KeyCode::Enter, 72, 24);
        }
        for (width, height) in [(1, 1), (10, 4), (32, 12), (72, 24)] {
            let text = rendered(&modal, width, height);
            assert!(!text.is_empty());
            press(&mut modal, &mut state, KeyCode::PageDown, width, height);
            press(&mut modal, &mut state, KeyCode::PageUp, width, height);
        }
    }
}
