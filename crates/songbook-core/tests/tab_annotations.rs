//! Original synthetic chord/fingering annotations and literal six-row tabs.
use songbook_core::{
    audit::audit_summary_line,
    book::{compose_book, distinct_chords, BookPart},
    latex::render::render,
    parser::{
        classify::{classify_lines, parse},
        song::{LineKind, Song},
    },
    preview::{build, RowStyle},
    tablature::{has_tablature, validate, warnings, TabRowRole, UNCHANGED_WARNING},
    transpose::transpose_song,
    triage::{Key, TriageState},
};

fn tab(width: usize) -> String {
    ["e", "B", "G", "D", "A", "E"]
        .map(|label| format!("{label}|{}\n", "-".repeat(width - 2)))
        .concat()
}

fn state(raw: &str) -> TriageState {
    let mut state = TriageState::new(parse(raw), raw);
    state.handle(Key::Confirm);
    state
}

fn line_index(state: &TriageState, text: &str) -> usize {
    state
        .lines
        .iter()
        .position(|line| line.original == text)
        .unwrap_or_else(|| panic!("missing original line {text:?}"))
}

fn annotation_rows(song: &Song) -> Vec<&str> {
    song.sections
        .iter()
        .flat_map(|section| &section.lines)
        .filter_map(|line| match line {
            LineKind::TabAnnotation(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn string_rows(song: &Song) -> Vec<&str> {
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
fn heading_and_parenthetical_fingerings_attach_with_independent_source_ordinals() {
    let annotations = "  Em      A7/C#  \n(Em*)     (A7/C#)\n(022000)  (x02220)  ";
    let raw = format!("{annotations}\n{}", tab(20));
    let parsed = parse(&raw);
    assert_eq!(
        annotation_rows(&parsed),
        annotations.lines().collect::<Vec<_>>()
    );
    assert_eq!(string_rows(&parsed), tab(20).lines().collect::<Vec<_>>());
    assert_eq!(parsed.sections[0].lines.len(), 9);
    let group = &parsed.tab_groups[0];
    assert_eq!((group.annotation_start, group.start, group.end), (0, 3, 9));
    assert_eq!(
        (group.source_line, group.original_rows, group.origin),
        (1, 6, 0)
    );
    assert!(warnings(&parsed).is_empty(), "{:?}", warnings(&parsed));
    validate(&parsed).unwrap();

    let reviewed = state(&raw);
    assert_eq!(reviewed.finish().tab_groups, parsed.tab_groups);
    assert_eq!(
        render(&reviewed.finish(), false).unwrap(),
        render(&parsed, false).unwrap()
    );
    for (index, line) in reviewed.lines.iter().take(9).enumerate() {
        let source = line.tab_provenance().unwrap();
        assert_eq!(source.source_line(), index + 1);
        assert_eq!(source.original_rows(), 6);
        assert_eq!(source.origin(), 0);
        if index < 3 {
            assert_eq!(source.role(), TabRowRole::Annotation);
            assert_eq!(source.original_row(), index + 1);
        } else {
            assert_eq!(source.role(), TabRowRole::String);
            assert_eq!(source.original_row(), index - 2);
        }
    }
    assert_eq!(
        annotation_rows(&reviewed.finish()),
        annotation_rows(&parsed)
    );
}

#[test]
fn common_generic_headings_and_multiple_comment_forms_use_existing_chord_syntax() {
    for (heading, comments) in [
        (
            "C Csus2 Ab Gm G Am Em Bm F# Dm",
            "(x32010) (Csus2) (133211)\n(Em*) (A7/C#)",
        ),
        ("| Am | F | C | 2x", "(Am) (F) (C)"),
        ("Em*   A7/C#,", "(022000) (X02220)"),
    ] {
        let raw = format!("{heading}\n{comments}\n{}", tab(20));
        let song = parse(&raw);
        assert_eq!(
            annotation_rows(&song).len(),
            comments.lines().count() + 1,
            "{raw}"
        );
        assert_eq!(song.tab_groups[0].end - song.tab_groups[0].start, 6);
        validate(&song).unwrap();
    }
}

#[test]
fn cue_and_heading_attach_in_either_order_without_changing_literal_text() {
    for heading in ["  C    or   Csus2  ", "  Em*    A7/C#  "] {
        for cue in [" (C)   (Csus2)  ", " (x32010)   (022000)  "] {
            for prefix in [format!("{cue}\n{heading}"), format!("{heading}\n{cue}")] {
                // With and without a section, the prefix must survive title
                // inference, contextual classification and review unchanged.
                for section in ["", "[Verse]\n"] {
                    let raw = format!("{section}{prefix}\n{}", tab(20));
                    let song = parse(&raw);
                    let expected = prefix.lines().collect::<Vec<_>>();
                    assert_eq!(annotation_rows(&song), expected, "{raw:?}");
                    assert_eq!(annotation_rows(&state(&raw).finish()), expected);
                    let classified = classify_lines(&raw);
                    let offset = usize::from(!section.is_empty());
                    for (index, text) in prefix.lines().enumerate() {
                        assert!(!classified[offset + index].preamble);
                        assert_eq!(
                            classified[offset + index].kind,
                            LineKind::TabAnnotation(text.to_string())
                        );
                    }
                    validate(&song).unwrap();
                }
            }
        }
    }
}

#[test]
fn cue_scan_stops_at_blank_metadata_section_and_invalid_or_headings() {
    for barrier in ["", "Key: C", "[Verse]", "ordinary comment"] {
        for prefix in [
            format!("(022000)\n{barrier}\nEm A7"),
            format!("Em A7\n(022000)\n{barrier}"),
        ] {
            let raw = format!("[Intro]\n{prefix}\n{}", tab(20));
            assert!(annotation_rows(&parse(&raw)).is_empty(), "{raw:?}");
        }
        // A nearby cue still attaches, but cannot pull an earlier heading
        // across a barrier into the annotation prefix.
        let raw = format!("[Intro]\nEm A7\n{barrier}\n(022000)\n{}", tab(20));
        assert_eq!(annotation_rows(&parse(&raw)), ["(022000)"]);
    }
    for heading in ["or C", "C or", "C or or G", "C or lyric", "C lyric G"] {
        let raw = format!("[Verse]\n(022000)\n{heading}\n{}", tab(20));
        assert!(annotation_rows(&parse(&raw)).is_empty(), "{raw:?}");
        let raw = format!("[Verse]\n{heading}\n(022000)\n{}", tab(20));
        assert_eq!(annotation_rows(&parse(&raw)), ["(022000)"]);
    }
    for prefix in ["C or Csus2", "C\n(this is a lyric)", "(this is a lyric)\nC"] {
        let raw = format!("[Verse]\n{prefix}\n{}", tab(20));
        assert!(annotation_rows(&parse(&raw)).is_empty(), "{raw:?}");
    }
}

#[test]
fn parenthetical_evidence_is_required_and_never_crosses_boundaries() {
    let cases = [
        "Em A7/C#",
        "(this is a parenthetical lyric)",
        "(x0222)",
        "(x02220-xxxxx)",
        "(022000)\n\nRoman numerals are prose",
        "(022000)\nTabbed by: Someone",
        "ordinary performance comment",
    ];
    for prefix in cases {
        let raw = format!("{prefix}\n{}", tab(20));
        let parsed = parse(&raw);
        assert!(annotation_rows(&parsed).is_empty(), "absorbed {prefix:?}");
        for count in [5, 7, 12] {
            let rows = format!(
                "{}\n",
                tab(20)
                    .repeat(2)
                    .lines()
                    .take(count)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            let partial = format!("(Em*)\n{rows}");
            assert!(
                annotation_rows(&parse(&partial)).is_empty(),
                "count {count}"
            );
        }
    }

    // Backward scanning stops at an unsupported comment: only the nearest
    // parenthetical comment is evidence, never the chord heading above prose.
    let raw = format!("Em A7\nunrelated comment\n(022000)\n{}", tab(20));
    assert_eq!(annotation_rows(&parse(&raw)), ["(022000)"]);
    assert!(!annotation_rows(&parse(&raw)).contains(&"Em A7"));
}

#[test]
fn title_artist_inference_still_stops_before_the_annotation_prefix() {
    let raw = format!(
        "Inferred Title\nInferred Artist\nEm A7/C#\n(022000) (x02220)\n{}",
        tab(20)
    );
    let song = parse(&raw);
    assert_eq!(song.title.as_deref(), Some("Inferred Title"));
    assert_eq!(song.artist.as_deref(), Some("Inferred Artist"));
    assert_eq!(annotation_rows(&song), ["Em A7/C#", "(022000) (x02220)"]);
    assert!(classify_lines(&raw)
        .iter()
        .take(2)
        .all(|line| line.preamble));
    assert!(classify_lines(&raw)
        .iter()
        .skip(2)
        .take(2)
        .all(|line| !line.preamble && matches!(line.kind, LineKind::TabAnnotation(_))));
}

#[test]
fn annotations_render_literally_preview_in_order_and_stay_out_of_chord_indexes() {
    let raw = format!(
        "[Verse]\n[D]Before\nC Em A7/C#\n(x32010) (Em*) (A7/C#)\n{}[G]After",
        tab(20)
    );
    let mut song = parse(&raw);
    assert_eq!(distinct_chords(&song), ["D", "G"]);
    transpose_song(&mut song, 2);
    assert_eq!(
        annotation_rows(&song),
        ["C Em A7/C#", "(x32010) (Em*) (A7/C#)"]
    );
    assert_eq!(string_rows(&song), tab(20).lines().collect::<Vec<_>>());
    assert!(warnings(&song).contains(&UNCHANGED_WARNING.to_string()));
    assert_eq!(distinct_chords(&song), ["E", "A"]);

    let tex = render(&song, false).unwrap();
    assert_eq!(tex.matches("\\textnote{\\vbox{").count(), 1);
    // Two annotation hboxes plus six string hboxes; source TeX never appears.
    assert_eq!(tex.matches("\\hbox{").count(), 9); // includes \\setbox0=\\hbox
    assert!(!tex.contains("A7/C#"));
    assert!(tex.contains(
        "\\char65\\relax \\char55\\relax \\char47\\relax \\char67\\relax \\char35\\relax"
    ));

    let metadata = songbook_core::triage::Metadata::default();
    let preview = build(&song, &metadata);
    let literal: Vec<_> = preview
        .iter()
        .filter(|row| row.style == RowStyle::Tablature)
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(&literal[..2], annotation_rows(&song));
    assert_eq!(&literal[2..], string_rows(&song));

    let book = compose_book(
        None,
        None,
        &[BookPart {
            body: tex.clone(),
            chords: distinct_chords(&song),
        }],
        false,
    )
    .unwrap();
    assert!(book.contains(&tex));
    assert!(!book.contains("A7/C#{"));
    let indexed_book = compose_book(
        None,
        None,
        &[BookPart {
            body: tex.clone(),
            chords: distinct_chords(&song),
        }],
        true,
    )
    .unwrap();
    assert!(indexed_book.contains("\\indexentry[chordidx]{E}"));
}

#[test]
fn differing_annotation_content_prevents_chorus_deduplication() {
    let differing = format!("[Chorus]\n(Em*)\n{}[Chorus]\n(A7/C#)\n{}", tab(20), tab(20));
    let song = parse(&differing);
    assert_eq!(
        render(&song, false)
            .unwrap()
            .matches("\\beginchorus")
            .count(),
        2
    );
    assert!(!audit_summary_line(&song, &differing).contains("collapsed"));

    let identical = format!("[Chorus]\n(Em*)\n{}[Chorus]\n(Em*)\n{}", tab(20), tab(20));
    let song = parse(&identical);
    assert_eq!(
        render(&song, false)
            .unwrap()
            .matches("\\beginchorus")
            .count(),
        1
    );
    assert!(audit_summary_line(&song, &identical).contains("collapsed"));
}

#[test]
fn discarding_strings_leaves_printable_orphan_annotations_not_partial_six_strings() {
    let raw = format!("Em A7/C#\n(022000) (x02220)\n{}", tab(20));
    let mut reviewed = state(&raw);
    for line in reviewed.lines.iter_mut().skip(2) {
        line.verdict = songbook_core::triage::Verdict::Discard;
    }
    let song = reviewed.finish();
    assert_eq!(annotation_rows(&song).len(), 2);
    assert!(string_rows(&song).is_empty());
    assert!(has_tablature(&song));
    assert_eq!(
        (
            song.tab_groups[0].annotation_start,
            song.tab_groups[0].start,
            song.tab_groups[0].end
        ),
        (0, 2, 2)
    );
    let report = warnings(&song);
    assert_eq!(report.len(), 1, "{report:?}");
    assert!(report[0].contains("no retained string rows"), "{report:?}");
    assert!(!report[0].contains("Partial six-string"), "{report:?}");
    validate(&song).unwrap();
    let tex = render(&song, false).unwrap();
    assert_eq!(tex.matches("\\textnote{\\vbox{").count(), 1);
    assert!(!tex.contains("Partial six-string"));
}

#[test]
fn annotation_width_is_checked_independently_instead_of_merged_phantom_width() {
    let annotation = format!("(022000){}", " ".repeat(49)); // 57 bytes, not a merged chord/lyric row.
    assert_eq!(annotation.len(), 57);
    let raw = format!("{annotation}\n{}", tab(20));
    let song = parse(&raw);
    assert_eq!(annotation_rows(&song), [annotation.as_str()]);
    let error = validate(&song).unwrap_err().to_string();
    assert!(error.contains("annotation row 1 has 57 columns"), "{error}");
    assert!(!error.contains("69 columns"), "{error}");
    assert!(render(&song, false).is_err());
}

#[test]
fn manual_action_attaches_only_selected_adjacent_comment_and_undo_restores_it() {
    let raw = format!(
        "Song: Manual\n[Verse]\nfirst ordinary comment\nsecond ordinary comment\n{}",
        tab(20)
    );
    let mut reviewed = state(&raw);
    let second = line_index(&reviewed, "second ordinary comment");
    let first = line_index(&reviewed, "first ordinary comment");

    reviewed.position = first;
    let untouched = reviewed.lines.clone();
    reviewed.handle(Key::TabAnnotation);
    assert_eq!(
        reviewed.lines, untouched,
        "must not jump across the second comment"
    );
    assert!(reviewed
        .message
        .as_deref()
        .unwrap()
        .contains("immediately before"));

    reviewed.position = second;
    let original = reviewed.lines.clone();
    let string_origin = reviewed.lines[second + 1]
        .tab_provenance()
        .unwrap()
        .origin();
    reviewed.handle(Key::TabAnnotation);
    assert!(matches!(
        reviewed.lines[second].kind,
        LineKind::TabAnnotation(_)
    ));
    assert_eq!(reviewed.lines[second].effective_label(), "tab annotation");
    assert_eq!(
        annotation_rows(&reviewed.finish()),
        ["second ordinary comment"]
    );
    assert!(annotation_rows(&reviewed.finish())
        .iter()
        .all(|text| !text.contains("first")));
    assert_eq!(reviewed.finish().tab_groups[0].origin, string_origin);
    assert_eq!(
        reviewed.finish().tab_groups[0].end - reviewed.finish().tab_groups[0].start,
        6
    );

    // Once the lower row is explicitly associated, the adjacent upper row can
    // be associated as well. Nothing else is absorbed implicitly.
    reviewed.position = first;
    reviewed.handle(Key::TabAnnotation);
    assert_eq!(
        annotation_rows(&reviewed.finish()),
        ["first ordinary comment", "second ordinary comment"]
    );
    let annotations: Vec<_> = reviewed.lines[first..=second]
        .iter()
        .map(|line| line.tab_provenance().unwrap())
        .collect();
    assert_eq!(annotations[0].original_row(), 1);
    assert_eq!(annotations[1].original_row(), 2);
    assert!(annotations
        .iter()
        .all(|source| source.role() == TabRowRole::Annotation));

    reviewed.handle(Key::Undo);
    assert_eq!(
        annotation_rows(&reviewed.finish()),
        ["second ordinary comment"]
    );
    reviewed.handle(Key::Undo);
    assert_eq!(reviewed.lines, original);
    assert!(annotation_rows(&reviewed.finish()).is_empty());
}

#[test]
fn manual_action_rejects_unsupported_partial_nonadjacent_and_string_rows() {
    for raw in [
        format!("comment\n\n{}", tab(20)),
        "comment\ne|---|\nB|---|\nG|---|\nD|---|\nA|---|\n".to_string(),
    ] {
        let mut reviewed = state(&raw);
        reviewed.position = line_index(&reviewed, "comment");
        let before = reviewed.lines.clone();
        reviewed.handle(Key::TabAnnotation);
        assert_eq!(reviewed.lines, before);
        assert!(annotation_rows(&reviewed.finish()).is_empty());
    }

    let mut reviewed = state(&format!("comment\n{}", tab(20)));
    reviewed.lines[6].verdict = songbook_core::triage::Verdict::Discard;
    reviewed.position = 0;
    let before = reviewed.lines.clone();
    reviewed.handle(Key::TabAnnotation);
    assert_eq!(reviewed.lines, before);

    let mut reviewed = state(&tab(20));
    reviewed.position = 0;
    let before = reviewed.lines.clone();
    reviewed.handle(Key::TabAnnotation);
    assert_eq!(reviewed.lines, before);
}

#[test]
fn manual_action_can_restore_a_preamble_comment_without_changing_inferred_metadata() {
    let raw = format!("Inferred comment title\n{}", tab(20));
    let mut reviewed = state(&raw);
    assert!(reviewed.lines[0].preamble);
    assert_eq!(reviewed.metadata.title, "Inferred comment title");
    reviewed.position = 0;
    reviewed.handle(Key::TabAnnotation);
    assert_eq!(
        annotation_rows(&reviewed.finish()),
        ["Inferred comment title"]
    );
    assert_eq!(
        reviewed.finish().title.as_deref(),
        Some("Inferred comment title")
    );
    reviewed.position = 0;
    assert!(reviewed.verdict_label().contains("literal tab annotation"));
    reviewed.handle(Key::Undo);
    assert!(reviewed.lines[0].preamble);
    assert!(annotation_rows(&reviewed.finish()).is_empty());
    assert_eq!(
        reviewed.finish().title.as_deref(),
        Some("Inferred comment title")
    );
}

#[test]
fn split_preview_and_apply_reproduce_annotations_above_each_six_row_child() {
    let source_annotations = ["  Em A7  ", " (022000)  "];
    let source_strings = [
        "  d|--0--12--|  ",
        "A|--1--| ",
        " F#|----2---|  ",
        "  C|------3--|  ",
        "G|--4------| ",
        " D|--------5|  ",
    ];
    let raw = format!(
        "{}\n{}\n{}",
        source_annotations.join("\n"),
        "(Em*)",
        source_strings.join("\n")
    );
    let mut reviewed = state(&raw);
    let original_lines = reviewed.lines.clone();
    let original_flags = reviewed.flags.clone();
    let original_position = reviewed.position;
    let origin = reviewed.lines[0].tab_provenance().unwrap().origin();
    let plan = reviewed.plan_tab_split(10).unwrap();
    let preview = plan.preview().clone();
    assert_eq!(preview.bounds.annotation_count, 3);
    assert_eq!((preview.bounds.start, preview.bounds.end), (0, 9));
    assert_eq!(preview.annotations.len(), 3);
    assert_eq!(reviewed.lines, original_lines);
    assert_eq!(reviewed.flags, original_flags);
    assert!(preview
        .warnings
        .iter()
        .any(|warning| warning.contains("annotations appear above both")));
    for (index, row) in preview.annotations.iter().enumerate() {
        let text = original_lines[index].text();
        assert_eq!(row.source_line, index + 1);
        assert_eq!(row.original_row, index + 1);
        assert_eq!(row.left, text[..10.min(text.len())]);
        assert_eq!(
            row.right,
            format!("{}{}", " ".repeat(4), text.get(10..).unwrap_or(""))
        );
        assert!(row.prefix.is_empty());
        assert_eq!(row.padding_columns, 4);
        assert!(!row.right.starts_with(text));
    }
    reviewed.apply_tab_split(plan).unwrap();
    assert_eq!(reviewed.lines.len(), 18);
    assert_eq!(reviewed.position, 9);
    assert_eq!(reviewed.flags.len(), 12);
    for (index, line) in reviewed.lines.iter().enumerate() {
        let right = index >= 9;
        let child_index = if right { index - 9 } else { index };
        let source_index = child_index;
        let expected_row = if child_index < 3 {
            &preview.annotations[child_index]
        } else {
            &preview.rows[child_index - 3]
        };
        assert_eq!(line.original, original_lines[source_index].original);
        assert_eq!(
            line.source_line(),
            original_lines[source_index].source_line()
        );
        assert_eq!(
            line.text(),
            if right {
                &expected_row.right
            } else {
                &expected_row.left
            }
        );
        let source = line.tab_provenance().unwrap();
        assert_eq!(source.origin(), origin);
        assert_eq!(
            source.role(),
            original_lines[source_index]
                .tab_provenance()
                .unwrap()
                .role()
        );
        assert_eq!(
            source.original_row(),
            original_lines[source_index]
                .tab_provenance()
                .unwrap()
                .original_row()
        );
        assert_eq!(source.original_rows(), 6);
    }
    let song = reviewed.finish();
    assert_eq!(song.tab_groups.len(), 2);
    for group in &song.tab_groups {
        assert_eq!(group.origin, origin);
        assert_eq!(group.end - group.start, 6);
        assert_eq!(group.start - group.annotation_start, 3);
        assert_ne!(
            group.instance,
            original_lines[0].tab_provenance().unwrap().instance()
        );
    }
    assert_ne!(song.tab_groups[0].instance, song.tab_groups[1].instance);
    assert!(warnings(&song).is_empty(), "{:?}", warnings(&song));
    validate(&song).unwrap();
    assert_eq!(annotation_rows(&song).len(), 6);
    assert_eq!(
        &annotation_rows(&song)[..3],
        preview
            .annotations
            .iter()
            .map(|row| row.left.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &annotation_rows(&song)[3..],
        preview
            .annotations
            .iter()
            .map(|row| row.right.as_str())
            .collect::<Vec<_>>()
    );
    let tex = render(&song, false).unwrap();
    assert_eq!(tex.matches("\\textnote{\\vbox{").count(), 2);

    reviewed.handle(Key::Undo);
    assert_eq!(reviewed.lines, original_lines);
    assert_eq!(reviewed.flags, original_flags);
    assert_eq!(reviewed.position, original_position);
    assert_eq!(reviewed.finish().tab_groups, parse(&raw).tab_groups);
}

#[test]
fn annotated_right_children_split_again_without_rechunking_or_partial_warnings() {
    let raw = format!("Em A7\n(022000) (x02220)\n{}", tab(180));
    let mut reviewed = state(&raw);
    let origin = reviewed.lines[0].tab_provenance().unwrap().origin();
    for expected_groups in [2, 3] {
        let plan = reviewed.plan_tab_split(56).unwrap();
        assert_eq!(plan.preview().annotations.len(), 2);
        reviewed.apply_tab_split(plan).unwrap();
        let song = reviewed.finish();
        assert_eq!(song.tab_groups.len(), expected_groups);
        assert!(song.tab_groups.iter().all(|group| group.origin == origin));
        assert!(song
            .tab_groups
            .iter()
            .all(|group| group.end - group.start == 6));
        assert_eq!(annotation_rows(&song).len(), expected_groups * 2);
        assert!(warnings(&song)
            .iter()
            .all(|warning| !warning.contains("Partial six-string")));
    }
    reviewed.handle(Key::Undo);
    assert_eq!(reviewed.finish().tab_groups.len(), 2);
}

#[test]
fn split_rejects_missing_annotations_and_right_suffixes_never_repeat_whole_notes() {
    let raw = format!("Em A7\n(022000) (x02220)\n{}", tab(40));
    let mut reviewed = state(&raw);
    let plan = reviewed.plan_tab_split(20).unwrap();
    let preview = plan.preview().clone();
    assert_eq!(preview.annotations[0].right, "  ");
    assert_eq!(preview.annotations[1].right, "  ");
    drop(plan);
    reviewed.lines[0].verdict = songbook_core::triage::Verdict::Discard;
    assert!(reviewed.tab_split_bounds().is_err());
    assert!(reviewed.plan_tab_split(20).is_err());
}
