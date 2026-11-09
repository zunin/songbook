//! Original synthetic literal tabs only: raw columns, not musical validation.
use songbook_core::{
    latex::render::render,
    parser::{
        classify::parse,
        song::{LineKind, Song},
    },
    preview::{build, RowStyle},
    tablature::{validate, warnings, MAX_COLUMNS, SPLIT_NOTATION_WARNING, SPLIT_RIGHT_WARNING},
    triage::{Key, Phase, TriageState, Verdict},
};

fn state(raw: &str) -> TriageState {
    let mut state = TriageState::new(parse(raw), raw);
    state.handle(Key::Confirm);
    state
}

fn tab(width: usize) -> String {
    ["e", "B", "G", "D", "A", "E"]
        .map(|label| format!("{label}|{}\n", "-".repeat(width - 2)))
        .concat()
}

fn tab_rows(song: &Song) -> Vec<&str> {
    song.sections
        .iter()
        .flat_map(|section| &section.lines)
        .filter_map(|line| match line {
            LineKind::Fretboard(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn preview_preserves_unequal_prefixes_tails_and_trailing_spaces_without_trimming() {
    let source = [
        "  d|--0--12--|  ",
        "A|--1--| ",
        " F#|----2---|  ",
        "  C|------3--|  ",
        "G|--4------| ",
        " D|--------5|  ",
    ];
    let raw = source.join("\n");
    let mut state = state(&raw);
    let original_lines = state.lines.clone();
    let original_flags = state.flags.clone();
    let plan = state.plan_tab_split(10).unwrap();
    let preview = plan.preview().clone();
    assert_eq!(state.lines, original_lines);
    assert_eq!(state.flags, original_flags);
    assert_eq!(preview.bounds.prefix_columns, 4);
    assert_eq!(preview.bounds.min_after_column, 5);
    assert_eq!(preview.bounds.max_after_column, 15);
    assert!(preview
        .warnings
        .iter()
        .any(|warning| warning == SPLIT_NOTATION_WARNING));
    for (index, (text, row)) in source.iter().zip(&preview.rows).enumerate() {
        let prefix_len = text.find('|').unwrap() + 1;
        assert_eq!(row.prefix, text[..prefix_len]);
        assert_eq!(row.padding_columns, 4 - prefix_len);
        assert_eq!(row.source_line, index + 1);
        assert_eq!(row.original_row, index + 1);
        assert_eq!(row.original_columns, text.len());
        assert_eq!(row.left, text[..10.min(text.len())]);
        assert_eq!(
            row.right,
            format!(
                "{}{}{}",
                &text[..prefix_len],
                " ".repeat(4 - prefix_len),
                text.get(10..).unwrap_or("")
            )
        );
        assert_eq!(row.left_columns, row.left.len());
        assert_eq!(row.right_columns, row.right.len());
        assert_eq!(row.right_label_only, text.len() <= 10);
        assert_eq!(format!("{}{}", row.left, &row.right[4..]), *text);
    }
    assert_eq!(preview.rows[1].left, "A|--1--| ");
    assert_eq!(preview.rows[1].right, "A|  "); // label-only plus disclosed prefix padding
    assert!(preview.rows[1].right_label_only);
    assert!(preview
        .warnings
        .iter()
        .any(|warning| warning.contains("Some right rows contain only")));
    assert_eq!(preview.rows[0].right, "  d|2--|  ");
    state.apply_tab_split(plan).unwrap();
    assert_eq!(state.lines.len(), 12);
    assert_eq!(state.position, 6);
    assert_eq!(state.flags, (0..12).collect::<Vec<_>>());
    for (index, line) in state.lines.iter().enumerate() {
        let row = index % 6;
        assert_eq!(line.original, original_lines[row].original);
        assert_eq!(line.kind, original_lines[row].kind);
        assert_eq!(line.verdict, original_lines[row].verdict);
        assert_eq!(line.preamble, original_lines[row].preamble);
        assert_eq!(line.source_line(), row + 1);
        assert_eq!(
            line.edited.as_deref(),
            Some(if index < 6 {
                preview.rows[row].left.as_str()
            } else {
                preview.rows[row].right.as_str()
            })
        );
    }
    let song = state.finish();
    assert_eq!(tab_rows(&song).len(), 12); // label-only descendant stays typed Fretboard
    assert_eq!(tab_rows(&song)[7], "A|  ");
    assert_eq!(
        song.tab_groups
            .iter()
            .map(|group| group.end - group.start)
            .collect::<Vec<_>>(),
        [6, 6]
    );
    assert!(warnings(&song).is_empty());
    validate(&song).unwrap();
}

#[test]
fn raw_column_bounds_are_inclusive_and_reject_no_progress_or_overflow() {
    let state = state(&tab(80));
    let bounds = state.tab_split_bounds().unwrap();
    assert_eq!((bounds.min_after_column, bounds.max_after_column), (3, 56));
    for cut in [0, 1, 2, 57, 80, usize::MAX] {
        assert!(state.plan_tab_split(cut).is_err(), "{cut}");
    }
    for cut in [3, 56] {
        state.plan_tab_split(cut).unwrap();
    }
    let state = self::state(&tab(12));
    assert_eq!(state.tab_split_bounds().unwrap().max_after_column, 11);
    assert!(state.plan_tab_split(12).is_err());
    let mut state = self::state(&tab(12));
    for line in &mut state.lines {
        line.edited = Some("e|".into());
    }
    assert!(state
        .tab_split_bounds()
        .unwrap_err()
        .to_string()
        .contains("no notation remains"));
}

#[test]
fn large_indentation_and_labels_have_actionable_bounds_errors() {
    for prefix_width in [56, 60] {
        let raw = format!("{}e|---\n", " ".repeat(prefix_width - 2)).repeat(6);
        let state = state(&raw);
        let error = state.tab_split_bounds().unwrap_err().to_string();
        assert!(
            error.contains(&format!("occupies {prefix_width} columns")),
            "{error}"
        );
        assert!(error.contains("Shorten indentation or labels"), "{error}");
    }
    let raw = format!("{}e|---\n", " ".repeat(53)).repeat(6);
    let state = state(&raw);
    let bounds = state.tab_split_bounds().unwrap();
    assert_eq!((bounds.min_after_column, bounds.max_after_column), (56, 56));
    state.plan_tab_split(56).unwrap();
}

#[test]
fn repeatedly_split_more_than_112_columns_makes_progress_and_exports() {
    let raw = tab(180);
    let mut state = state(&raw);
    let origin = state.lines[0].tab_provenance().unwrap().origin();
    let original_instance = state.lines[0].tab_provenance().unwrap().instance();
    let mut splits = 0;
    while state.tab_split_bounds().unwrap().max_row_columns > MAX_COLUMNS {
        let before = state.tab_split_bounds().unwrap().max_row_columns;
        let plan = state.plan_tab_split(56).unwrap();
        let preview = plan.preview();
        assert!(preview
            .rows
            .iter()
            .all(|row| row.right_columns == before - 54));
        assert_eq!(
            preview
                .warnings
                .iter()
                .any(|warning| warning == SPLIT_RIGHT_WARNING),
            before - 54 > 56
        );
        state.apply_tab_split(plan).unwrap();
        splits += 1;
    }
    assert_eq!(splits, 3);
    assert_eq!(state.lines.len(), 24);
    let song = state.finish();
    assert_eq!(song.tab_groups.len(), 4);
    let mut instances = Vec::new();
    for group in &song.tab_groups {
        assert_eq!(group.origin, origin);
        assert_eq!(group.source_line, 1);
        assert_eq!(group.original_rows, 6);
        assert_ne!(group.instance, original_instance);
        assert_eq!(group.end - group.start, 6);
        instances.push(group.instance);
    }
    instances.sort_unstable();
    instances.dedup();
    assert_eq!(instances.len(), 4);
    for chunk in state.lines.chunks_exact(6) {
        for (index, line) in chunk.iter().enumerate() {
            let provenance = line.tab_provenance().unwrap();
            assert_eq!(line.source_line(), index + 1);
            assert_eq!(provenance.original_row(), index + 1);
            assert_eq!(provenance.original_rows(), 6);
        }
    }
    validate(&song).unwrap();
    assert!(warnings(&song).is_empty());
    let tex = render(&song, false).unwrap();
    assert_eq!(tex.matches("\\textnote{\\vbox{").count(), 4);
}

#[test]
fn nested_left_and_right_splits_never_rechunk_adjacent_descendants() {
    let mut state = state(&tab(100));
    let first = state.plan_tab_split(40).unwrap();
    state.apply_tab_split(first).unwrap();
    state.position = 1; // any row selects its own instance, not the whole origin
    let plan = state.plan_tab_split(20).unwrap();
    assert_eq!(
        (plan.preview().bounds.start, plan.preview().bounds.end),
        (0, 6)
    );
    state.apply_tab_split(plan).unwrap();
    assert_eq!(state.position, 6);
    state.position = 13;
    let plan = state.plan_tab_split(40).unwrap();
    assert_eq!(
        (plan.preview().bounds.start, plan.preview().bounds.end),
        (12, 18)
    );
    state.apply_tab_split(plan).unwrap();
    assert_eq!(state.finish().tab_groups.len(), 4);
    validate(&state.finish()).unwrap();
}

#[test]
fn selection_and_mixed_source_order_remain_literal_with_stable_source_lines() {
    let raw = format!(
        "Song: Mixed\n[Verse]\n[C]Before\n{}\n[G]Between\n{}[Outro]\n[F]After\n",
        tab(30),
        tab(30)
    );
    let mut state = state(&raw);
    state.position = 7;
    let plan = state.plan_tab_split(20).unwrap();
    assert_eq!(
        (plan.preview().bounds.start, plan.preview().bounds.end),
        (3, 9)
    );
    state.apply_tab_split(plan).unwrap();
    assert_eq!(state.position, 9);
    assert_eq!(
        state
            .lines
            .iter()
            .map(|line| line.source_line())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
    );
    let song = state.finish();
    assert_eq!(
        song.tab_groups
            .iter()
            .map(|group| group.source_line)
            .collect::<Vec<_>>(),
        [4, 4, 12]
    );
    let preview = build(&song, &state.metadata);
    let tab_preview: Vec<_> = preview
        .iter()
        .filter(|row| row.style == RowStyle::Tablature)
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(tab_preview, tab_rows(&song));
    let tex = render(&song, false).unwrap();
    assert!(tex.find("Before").unwrap() < tex.find("\\textnote{\\vbox{").unwrap());
    assert!(tex.find("Between").unwrap() < tex.rfind("\\textnote{\\vbox{").unwrap());
    assert!(tex.rfind("\\textnote{\\vbox{").unwrap() < tex.find("After").unwrap());
}

#[test]
fn unsupported_partial_reclassified_noncontiguous_and_duplicate_rows_are_ineligible() {
    for count in [4, 5, 7, 13] {
        let raw = "e|--------|\n".repeat(count);
        let mut state = state(&raw);
        assert!(state.tab_split_bounds().is_err());
        if count == 7 {
            state.handle(Key::Discard); // six retained rows must not evade original count
            assert!(state.plan_tab_split(5).is_err());
        }
    }
    for verdict in [
        Verdict::Discard,
        Verdict::Noise,
        Verdict::Lyric,
        Verdict::ChordOnly,
        Verdict::SectionHeader,
    ] {
        let mut state = state(&tab(30));
        state.lines[2].verdict = verdict;
        state.position = 5;
        assert!(state
            .plan_tab_split(20)
            .unwrap_err()
            .to_string()
            .contains("partial or reclassified"));
    }
    let mut state = state(&tab(30));
    state.lines[1].preamble = true;
    assert!(state.tab_split_bounds().is_err());
    let mut state = self::state(&tab(30));
    state.lines.remove(2);
    assert!(state.tab_split_bounds().is_err());
    let mut state = self::state(&tab(30));
    state.lines[2] = state.lines[1].clone();
    assert!(state
        .tab_split_bounds()
        .unwrap_err()
        .to_string()
        .contains("unique original row"));
    let mut state = self::state(&format!("{}[Verse]\n[C]After", tab(30)));
    state.lines.swap(2, 7);
    assert!(state
        .tab_split_bounds()
        .unwrap_err()
        .to_string()
        .contains("noncontiguous"));
}

#[test]
fn printable_ascii_and_first_pipe_are_required_before_slicing() {
    for bad in ["\t", "é", "\u{0001}", "\u{007f}"] {
        let mut state = state(&tab(30));
        state.lines[2].edited = Some(format!("G|---{bad}---"));
        assert!(state
            .plan_tab_split(20)
            .unwrap_err()
            .to_string()
            .contains("only printable ASCII"));
    }
    let mut state = state(&tab(30));
    state.lines[2].edited = Some("no pipe here".into());
    assert!(state
        .plan_tab_split(20)
        .unwrap_err()
        .to_string()
        .contains("label and first pipe"));
    let mut state = self::state(&tab(30));
    state.phase = Phase::Preview;
    assert!(state.tab_split_bounds().is_err());
    state.phase = Phase::Triage;
    state.handle(Key::Edit);
    assert!(state.tab_split_bounds().is_err());
}

#[test]
fn stale_plans_reject_navigation_edit_decision_undo_and_direct_changes_atomically() {
    for keys in [
        vec![Key::Next],
        vec![Key::Next, Key::Prev],
        vec![Key::Confirm],
        vec![Key::Discard, Key::Undo],
        vec![Key::Edit, Key::Char('x'), Key::Confirm],
        vec![Key::Edit, Key::Escape],
        vec![Key::Undo],
    ] {
        let mut state = state(&tab(30));
        let plan = state.plan_tab_split(20).unwrap();
        for key in keys {
            state.handle(key);
        }
        let lines = state.lines.clone();
        let flags = state.flags.clone();
        let position = state.position;
        assert!(state
            .apply_tab_split(plan)
            .unwrap_err()
            .to_string()
            .contains("stale"));
        assert_eq!(state.lines, lines);
        assert_eq!(state.flags, flags);
        assert_eq!(state.position, position);
    }
    for mutation in 0..4 {
        let mut state = state(&tab(30));
        let plan = state.plan_tab_split(20).unwrap();
        match mutation {
            0 => state.position = 1,
            1 => state.lines[4].verdict = Verdict::Discard,
            2 => state.lines[3].edited = Some("D|--edited--".into()),
            _ => state.metadata.title.push('!'),
        }
        assert!(state.apply_tab_split(plan).is_err());
    }
    let mut state = state(&tab(30));
    let plan = state.plan_tab_split(20).unwrap();
    let copy = plan.clone();
    state.apply_tab_split(plan).unwrap();
    assert!(state.apply_tab_split(copy.clone()).is_err());
    state.handle(Key::Undo);
    assert!(state.apply_tab_split(copy).is_err());
}

#[test]
fn cancellation_and_failed_bounds_do_not_add_undo_steps() {
    let mut state = state(&tab(30));
    let original = state.lines.clone();
    let preview = state.plan_tab_split(20).unwrap().preview().clone();
    assert_eq!(preview.rows.len(), 6);
    assert!(state.plan_tab_split(0).is_err());
    state.handle(Key::Undo);
    assert_eq!(state.message.as_deref(), Some("Nothing to undo."));
    assert_eq!(state.lines, original);
    state.handle(Key::Confirm);
    let position = state.position;
    let plan = state.plan_tab_split(20).unwrap();
    drop(plan);
    state.handle(Key::Undo);
    assert_eq!(state.position, position - 1);
    assert_eq!(state.lines, original);
}

#[test]
fn split_undo_restores_flags_position_verdict_edits_view_and_keeps_transpose() {
    let raw = format!("Song: Undo\n[Verse]\n{}[C]After\n", tab(30));
    let mut state = state(&raw);
    state.handle(Key::Confirm);
    state.lines[3].edited = Some("B|--12h3----------|  ".into());
    state.position = 5;
    state.horizontal_scroll = 24;
    state.preview_scroll = 4;
    let lines = state.lines.clone();
    let flags = state.flags.clone();
    let baseline = state.finish();
    let plan = state.plan_tab_split(20).unwrap();
    state.handle(Key::TransposeUp); // offset alone must not invalidate the plan
    state.apply_tab_split(plan).unwrap();
    assert_eq!(state.position, 8);
    assert_eq!(state.horizontal_scroll, 0);
    assert_eq!(state.pending_flags(), 10);
    state.handle(Key::Undo);
    assert_eq!(state.lines, lines);
    assert_eq!(state.flags, flags);
    assert_eq!(state.pending_flags(), 5);
    assert_eq!(state.position, 5);
    assert_eq!(state.horizontal_scroll, 24);
    assert_eq!(state.preview_scroll, 4);
    assert_eq!(state.phase, Phase::Triage);
    assert_eq!(state.transpose, 1);
    assert_eq!(state.finish().tab_groups, baseline.tab_groups);
    state.handle(Key::TransposeReset);
    assert_eq!(
        render(&state.finish(), false).unwrap(),
        render(&baseline, false).unwrap()
    );
    state.handle(Key::Undo); // the single decision before split, not six row edits
    assert_eq!(state.lines[2].verdict, Verdict::Pending);
    assert_eq!(state.position, 2);
}

#[test]
fn later_discard_reclassify_and_undo_preserve_origin_and_child_boundaries() {
    let mut state = state(&tab(30));
    let plan = state.plan_tab_split(20).unwrap();
    state.apply_tab_split(plan).unwrap();
    let complete = state.finish().tab_groups;
    state.handle(Key::Discard); // first right row
    let partial = state.finish();
    assert_eq!(
        partial
            .tab_groups
            .iter()
            .map(|group| group.end - group.start)
            .collect::<Vec<_>>(),
        [6, 5]
    );
    assert_eq!(partial.tab_groups[1].origin, complete[1].origin);
    assert_eq!(partial.tab_groups[1].instance, complete[1].instance);
    assert_eq!(partial.tab_groups[1].source_line, 1);
    assert!(warnings(&partial)
        .iter()
        .any(|warning| warning.contains("5 retained rows (6 originally)")));
    assert!(state.tab_split_bounds().is_err());
    state.handle(Key::Undo);
    assert_eq!(state.finish().tab_groups, complete);
    state.position = 8;
    state.handle(Key::Lyric);
    let fragmented = state.finish();
    assert_eq!(
        fragmented
            .tab_groups
            .iter()
            .map(|group| group.end - group.start)
            .collect::<Vec<_>>(),
        [6, 2, 3]
    );
    assert_eq!(
        fragmented.tab_groups[1].instance,
        fragmented.tab_groups[2].instance
    );
    assert_eq!(fragmented.tab_groups[1].origin, complete[1].origin);
    assert!(state.tab_split_bounds().is_err());
    state.handle(Key::Undo);
    assert_eq!(state.finish().tab_groups, complete);
    state.handle(Key::Undo);
    assert_eq!(state.lines.len(), 6);
    assert_eq!(state.finish().tab_groups, parse(&tab(30)).tab_groups);
}

#[test]
fn tex_looking_bytes_remain_literal_numeric_tokens_after_arbitrary_cut() {
    let hostile = r"e|--\input{x}%#$&_^{\end{songs}}~--|  ";
    let raw = format!("{hostile}\n{}", "e|-----|  \n".repeat(5));
    let mut state = state(&raw);
    let plan = state.plan_tab_split(10).unwrap();
    let preview = plan.preview().clone();
    state.apply_tab_split(plan).unwrap();
    let tex = render(&state.finish(), false).unwrap();
    assert!(!tex.contains("\\input{x}"));
    assert!(!tex.contains("\\end{songs}"));
    assert_eq!(
        preview.rows[0].left.clone() + &preview.rows[0].right[2..],
        hostile
    );
    for fragment in [&preview.rows[0].left, &preview.rows[0].right] {
        let encoded: String = fragment
            .bytes()
            .map(|byte| {
                if byte == b' ' {
                    "\\hskip\\dimen0\\relax ".into()
                } else {
                    format!("\\char{byte}\\relax ")
                }
            })
            .collect();
        assert!(tex.contains(&encoded));
    }
}

#[test]
fn unsplit_parse_review_preview_and_render_remain_equivalent() {
    let raw = format!("Song: Uncut\nArtist: Synthetic\n[Verse]\nC   G\nnew synthetic lyric\n{}\n{}[Chorus]\n[F]After\n", tab(30), tab(30));
    let parsed = parse(&raw);
    let mut state = state(&raw);
    let tex = render(&parsed, false).unwrap();
    let groups = parsed.tab_groups.clone();
    assert_eq!(state.finish().tab_groups, groups);
    assert_eq!(render(&state.finish(), false).unwrap(), tex);
    let original_preview = build(&state.finish(), &state.metadata);
    let plan = state.plan_tab_split(20).unwrap();
    assert_eq!(render(&state.finish(), false).unwrap(), tex);
    state.apply_tab_split(plan).unwrap();
    state.handle(Key::Undo);
    assert_eq!(state.finish().tab_groups, groups);
    assert_eq!(build(&state.finish(), &state.metadata), original_preview);
    assert_eq!(render(&state.finish(), false).unwrap(), tex);
}
