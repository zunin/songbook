//! Synthetic host-level coverage of the browser adapter, with no JS row input.
use songbook_wasm::{Session, SessionError, SessionSnapshot};

const RAGGED: &str = "  d|--0--12--|  \nA|--1--| \n F#|----2---|  \n  C|------3--|  \nG|--4------| \n D|--------5|  ";

fn review(raw: &str) -> Session {
    let mut session = Session::new(raw).unwrap();
    session.handle_action("confirm").unwrap();
    session
}

fn tab(width: usize) -> String {
    ["e", "B", "G", "D", "A", "E"]
        .map(|label| format!("{label}|{}\n", "-".repeat(width - 2)))
        .concat()
}

fn select(session: &mut Session, index: usize) {
    while session.snapshot().position < index {
        session.handle_action("next").unwrap();
    }
    while session.snapshot().position > index {
        session.handle_action("prev").unwrap();
    }
}

fn assert_review_restored(actual: &SessionSnapshot, before: &SessionSnapshot) {
    assert_eq!(actual.phase, before.phase);
    assert_eq!(actual.position, before.position);
    assert_eq!(actual.line_count, before.line_count);
    assert_eq!(actual.flag_count, before.flag_count);
    assert_eq!(actual.pending_flags, before.pending_flags);
    assert_eq!(actual.flags, before.flags);
    assert_eq!(actual.editing, before.editing);
    assert_eq!(actual.edit_buffer, before.edit_buffer);
    assert_eq!(actual.metadata, before.metadata);
    assert_eq!(actual.metadata_field, before.metadata_field);
    assert_eq!(actual.preview_scroll, before.preview_scroll);
    assert_eq!(actual.transpose, before.transpose);
    assert_eq!(actual.warnings, before.warnings);
    assert_eq!(actual.has_tablature, before.has_tablature);
    assert_eq!(actual.tab_split_bounds, before.tab_split_bounds);
    assert_eq!(actual.lines, before.lines);
}

#[test]
fn bounds_follow_core_eligibility_and_source_locations() {
    let raw = format!("Song: Synthetic\n[Verse]\n{RAGGED}\n[C]After");
    let mut session = Session::new(&raw).unwrap();
    assert!(session.snapshot().tab_split_bounds.is_none());
    assert!(session.plan_tab_split(10).is_err());
    session.handle_action("confirm").unwrap();
    let snapshot = session.snapshot();
    let bounds = snapshot.tab_split_bounds.unwrap();
    assert_eq!((bounds.start, bounds.end, bounds.source_line), (2, 8, 3));
    assert_eq!(bounds.origin, 2);
    assert_eq!(bounds.instance, 2);
    assert_eq!(bounds.prefix_columns, 4);
    assert_eq!(bounds.max_row_columns, 16);
    assert_eq!((bounds.min_after_column, bounds.max_after_column), (5, 15));
    for (index, line) in snapshot.lines.iter().enumerate() {
        assert_eq!(line.source_line, index + 1);
    }
    select(&mut session, 7);
    assert_eq!(session.snapshot().tab_split_bounds, Some(bounds.clone()));
    session.handle_action("edit").unwrap();
    assert!(session.snapshot().tab_split_bounds.is_none());
    session.handle_key("Escape").unwrap();
    assert_eq!(session.snapshot().tab_split_bounds, Some(bounds));
    select(&mut session, 8);
    assert!(session.snapshot().tab_split_bounds.is_none());
    select(&mut session, 2);
    session.handle_action("skip").unwrap();
    assert!(session.snapshot().tab_split_bounds.is_none());
    session.handle_action("back").unwrap();
    assert!(session.snapshot().tab_split_bounds.is_some());
}

#[test]
fn unsupported_partial_and_reclassified_blocks_have_no_bounds() {
    for count in [4, 5, 7, 13] {
        let mut session = review(&"e|------|\n".repeat(count));
        assert!(session.snapshot().tab_split_bounds.is_none());
        assert!(session.plan_tab_split(5).is_err());
        if count == 7 {
            session.handle_action("discard").unwrap();
            assert!(session.snapshot().tab_split_bounds.is_none());
            assert!(session.plan_tab_split(5).is_err());
        }
    }
    for action in ["discard", "lyric", "chord_only", "section_header", "noise"] {
        let mut session = review(RAGGED);
        session.handle_action(action).unwrap();
        assert!(session.snapshot().tab_split_bounds.is_none(), "{action}");
        assert!(session.plan_tab_split(10).is_err(), "{action}");
        session.handle_action("undo").unwrap();
        assert!(session.snapshot().tab_split_bounds.is_some(), "{action}");
    }
}

#[test]
fn preview_is_literal_and_cancel_changes_neither_review_nor_undo() {
    let mut session = review(RAGGED);
    // Existing review history must survive both planning and cancellation.
    session.handle_action("keep").unwrap();
    let before = session.snapshot();
    let body = session.export_tex(false).unwrap();
    let rows = session.preview_rows();
    let preview = session.plan_tab_split(10).unwrap();
    assert_eq!(session.snapshot(), before);
    assert_eq!(preview.after_column, 10);
    assert_eq!(preview.bounds, before.tab_split_bounds.clone().unwrap());
    assert_eq!(preview.rows.len(), 6);
    assert_eq!(preview.rows[0].left, "  d|--0--1");
    assert_eq!(preview.rows[0].right, "  d|2--|  ");
    assert_eq!(preview.rows[1].left, "A|--1--| ");
    assert_eq!(preview.rows[1].right, "A|  ");
    assert_eq!(preview.rows[1].prefix, "A|");
    assert_eq!(preview.rows[1].padding_columns, 2);
    assert!(preview.rows[1].right_label_only);
    for (index, row) in preview.rows.iter().enumerate() {
        assert_eq!(row.source_line, index + 1);
        assert_eq!(row.original_row, index + 1);
        assert_eq!(row.original_columns, before.lines[index].text.len());
        assert_eq!(row.left_columns, row.left.len());
        assert_eq!(row.right_columns, row.right.len());
    }
    assert!(preview.warnings.iter().any(|w| w.contains("fret number")));
    assert!(preview
        .warnings
        .iter()
        .any(|w| w.contains("Some right rows")));
    assert_eq!(session.cancel_tab_split(), before);
    assert_eq!(session.cancel_tab_split(), before);
    assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
    assert_eq!(session.export_tex(false).unwrap(), body);
    assert_eq!(session.preview_rows(), rows);
    session.handle_action("undo").unwrap();
    let undone = session.snapshot();
    assert_eq!(undone.position, 0);
    assert_eq!(undone.pending_flags, 6);
    assert_eq!(undone.lines[0].verdict, "pending");
}

#[test]
fn replacement_plans_use_the_latest_cut_and_failed_requests_drop_old_plans() {
    let mut session = review(RAGGED);
    let before = session.snapshot();
    session.plan_tab_split(8).unwrap();
    let latest = session.plan_tab_split(10).unwrap();
    assert_eq!(session.snapshot(), before);
    let applied = session.apply_tab_split().unwrap();
    assert_eq!(applied.lines[0].text, latest.rows[0].left);
    assert_eq!(applied.lines[6].text, latest.rows[0].right);
    assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
    session.handle_action("undo").unwrap();
    for invalid in [0, 4, 16, 57, usize::MAX] {
        session.plan_tab_split(10).unwrap();
        let before = session.snapshot();
        assert!(session.plan_tab_split(invalid).is_err());
        assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
        assert_eq!(session.snapshot(), before);
    }
}

#[test]
fn apply_is_one_atomic_undo_step_with_flags_verdicts_edits_and_source_rows() {
    let mut session = review(RAGGED);
    session.handle_action("keep").unwrap();
    session.handle_action("edit").unwrap();
    session.insert_text(" ").unwrap();
    session.handle_action("confirm").unwrap();
    select(&mut session, 4);
    let before = session.snapshot();
    let body = session.export_tex(false).unwrap();
    let rows = session.preview_rows();
    let preview = session.plan_tab_split(10).unwrap();
    let applied = session.apply_tab_split().unwrap();
    assert_eq!(applied, session.snapshot());
    assert_eq!(applied.line_count, 12);
    assert_eq!(applied.position, 6);
    assert_eq!(applied.flag_count, 12);
    assert_eq!(applied.pending_flags, 8);
    assert_eq!(applied.flags, (0..12).collect::<Vec<_>>());
    for (index, line) in applied.lines.iter().enumerate() {
        let original = &before.lines[index % 6];
        let fragments = &preview.rows[index % 6];
        assert_eq!(line.index, index);
        assert_eq!(line.source_line, original.source_line);
        assert_eq!(line.original, original.original);
        assert_eq!(line.classifier, original.classifier);
        assert_eq!(line.verdict, original.verdict);
        assert_eq!(line.preamble, original.preamble);
        assert_eq!(line.flag, original.flag);
        assert!(line.edited);
        assert_eq!(
            line.status,
            if line.verdict == "pending" {
                "pending"
            } else {
                "edited"
            }
        );
        assert_eq!(line.effective_label, "fretboard");
        assert_eq!(
            line.text,
            if index < 6 {
                &fragments.left
            } else {
                &fragments.right
            }
            .as_str()
        );
    }
    session.handle_action("undo").unwrap();
    assert_review_restored(&session.snapshot(), &before);
    assert_eq!(session.export_tex(false).unwrap(), body);
    assert_eq!(session.preview_rows(), rows);
    session.handle_action("undo").unwrap(); // The edit preceding the split.
    assert!(!session.snapshot().lines[1].edited);
    assert_eq!(session.snapshot().lines[1].verdict, "pending");
    session.handle_action("undo").unwrap(); // The original Keep decision.
    assert_eq!(session.snapshot().pending_flags, 6);
}

#[test]
fn generated_blocks_appear_in_snapshots_previews_exports_and_book_parts() {
    let mut session = review(RAGGED);
    session.transpose(3).unwrap();
    session.handle_action("transpose_down").unwrap();
    let preview = session.plan_tab_split(10).unwrap();
    let applied = session.apply_tab_split().unwrap();
    assert_eq!(applied.transpose, 2);
    assert_eq!(
        applied.warnings,
        [songbook_core::tablature::UNCHANGED_WARNING]
    );
    let expected: Vec<_> = preview
        .rows
        .iter()
        .map(|r| r.left.clone())
        .chain(preview.rows.iter().map(|r| r.right.clone()))
        .collect();
    assert_eq!(
        applied
            .lines
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        session
            .preview_rows()
            .into_iter()
            .filter(|r| r.style == "tablature")
            .map(|r| r.text)
            .collect::<Vec<_>>(),
        expected
    );
    let body = session.export_tex(false).unwrap();
    assert_eq!(body.matches("\\textnote{\\vbox{").count(), 2);
    assert_eq!(body.matches("\\hbox{").count(), 14); // Six rows + measured cell per block.
    assert!(body.contains(songbook_core::tablature::UNCHANGED_WARNING));
    assert_eq!(session.book_parts().unwrap().body, body);
    assert!(session.export_tex(true).unwrap().contains(&body));
    session.handle_action("undo").unwrap();
    assert_eq!(session.snapshot().transpose, 2);
    assert_eq!(session.snapshot().line_count, 6);
}

#[test]
fn repeated_splits_share_origins_and_source_rows_but_have_distinct_instances() {
    let mut session = review(&tab(180));
    let original = session.snapshot().tab_split_bounds.unwrap();
    assert!(session.export_tex(false).is_err());
    let mut splits = 0;
    while session
        .snapshot()
        .tab_split_bounds
        .as_ref()
        .unwrap()
        .max_row_columns
        > 56
    {
        let preview = session.plan_tab_split(56).unwrap();
        let still_wide = preview.rows.iter().any(|r| r.right_columns > 56);
        assert_eq!(
            preview
                .warnings
                .iter()
                .any(|w| w == songbook_core::tablature::SPLIT_RIGHT_WARNING),
            still_wide
        );
        let applied = session.apply_tab_split().unwrap();
        splits += 1;
        assert_eq!(applied.line_count, (splits + 1) * 6);
        assert_eq!(applied.position, splits * 6);
        assert_eq!(applied.tab_split_bounds.as_ref().unwrap().source_line, 1);
        if still_wide {
            assert!(session.export_tex(false).is_err());
            assert!(session.book_parts().is_err());
        }
    }
    assert_eq!(splits, 3);
    let mut instances = Vec::new();
    for block in 0..4 {
        select(&mut session, block * 6);
        let snapshot = session.snapshot();
        let bounds = snapshot.tab_split_bounds.unwrap();
        assert_eq!((bounds.start, bounds.end), (block * 6, (block + 1) * 6));
        assert_eq!(bounds.source_line, original.source_line);
        assert_eq!(bounds.origin, original.origin);
        assert_ne!(bounds.instance, original.instance);
        instances.push(bounds.instance);
        for (row, line) in snapshot.lines[block * 6..(block + 1) * 6]
            .iter()
            .enumerate()
        {
            assert_eq!(line.source_line, row + 1);
        }
    }
    instances.sort_unstable();
    instances.dedup();
    assert_eq!(instances.len(), 4);
    assert!(session.snapshot().warnings.is_empty());
    let body = session.export_tex(false).unwrap();
    assert_eq!(body.matches("\\textnote{\\vbox{").count(), 4);
    assert_eq!(session.book_parts().unwrap().body, body);
}

#[test]
fn normal_actions_keys_edits_undo_and_transpose_invalidate_pending_plans() {
    for actions in [
        vec!["next"],
        vec!["next", "prev"],
        vec!["discard", "undo"],
        vec!["keep"],
        vec!["undo"],
        vec!["edit", "escape"],
        vec!["transpose_up"],
        vec!["skip", "back"],
    ] {
        let mut session = review(RAGGED);
        session.plan_tab_split(10).unwrap();
        for action in actions {
            session.handle_action(action).unwrap();
        }
        let before = session.snapshot();
        assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
        assert_eq!(session.snapshot(), before);
    }
    let mut session = review(RAGGED);
    session.plan_tab_split(10).unwrap();
    session.handle_key("ArrowDown").unwrap();
    assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
    session.plan_tab_split(10).unwrap();
    session.transpose(1).unwrap();
    assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
    session.plan_tab_split(10).unwrap();
    session.handle_action("edit").unwrap();
    session.insert_char(" ").unwrap();
    session.insert_text("  ").unwrap();
    session.handle_key("Enter").unwrap();
    let edited = session.snapshot();
    assert_eq!(session.apply_tab_split(), Err(SessionError::NoTabSplitPlan));
    assert_eq!(session.snapshot(), edited);
    session.plan_tab_split(10).unwrap();
    session.apply_tab_split().unwrap();
}
