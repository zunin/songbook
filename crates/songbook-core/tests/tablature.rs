//! Original synthetic notation only; opaque strings, not musical validation.
use songbook_core::{
    audit::{apply_noninteractive_defaults, audit_report, audit_summary_line},
    book::{compose_book, distinct_chords, BookPart},
    latex::render::render,
    parser::{
        classify::{classify_lines, parse},
        song::{LineKind, Song},
    },
    preview::{build, RowStyle},
    tablature::{has_tablature, validate, warnings, MAX_COLUMNS, UNCHANGED_WARNING},
    transpose::transpose_song,
    triage::{Key, TriageState, Verdict},
};

const TAB: &str = "  d|--0--12--|  \n  A|--1------|  \n  F#|----2---|  \n  C|------3--|  \n  G|--4------|  \n  D|--------5|  \n";
fn rows(song: &Song) -> Vec<&str> {
    song.sections
        .iter()
        .flat_map(|s| &s.lines)
        .filter_map(|l| match l {
            LineKind::Fretboard(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}
fn state(raw: &str) -> TriageState {
    let mut s = TriageState::new(parse(raw), raw);
    s.handle(Key::Confirm);
    s
}

#[test]
fn literal_blocks_survive_chord_definition_preamble_and_noise_like_payload() {
    for area in ["Chords used:\n", "Chords used:\nAm: x02210\nC: x32010\n"] {
        let tab = TAB.replacen("--0--12--", "--www.example.org--", 1);
        let raw = format!("Song: Synthetic\n{area}{tab}[Verse]\n[C]After\n");
        let parsed = parse(&raw);
        let reviewed = state(&raw).finish();
        assert!(has_tablature(&parsed));
        assert_eq!(rows(&parsed), tab.lines().collect::<Vec<_>>());
        assert_eq!(rows(&reviewed), rows(&parsed));
        assert_eq!(reviewed.tab_groups, parsed.tab_groups);
        assert_eq!(parsed.tab_groups[0].original_rows, 6);
        assert_eq!(
            render(&parsed, false).unwrap(),
            render(&reviewed, false).unwrap()
        );
        for line in classify_lines(&raw).iter().filter(|l| matches!(&l.kind, LineKind::Noise(text) if text.contains("x02210") || text.contains("x32010"))) {
            assert!(line.preamble);
        }
        assert!(classify_lines(&raw)
            .iter()
            .filter(|l| matches!(l.kind, LineKind::Fretboard(_)))
            .all(|l| !l.preamble));
        assert!(!render(&parsed, false).unwrap().contains("x02210"));
    }
}

#[test]
fn digit_and_delimiter_leading_harmonics_are_opaque_tabs_not_lyrics() {
    for body in ["0--<12>--|", "12--<7>--|", "|0--<12>--|"] {
        let tab = ["e", "B", "G", "D", "A", "E"]
            .map(|label| format!("  {label}|{body}  \n"))
            .concat();
        let raw = format!("Song: Harmonics\nChords used:\n{tab}[Verse]\n[C]After\n");
        let parsed = parse(&raw);
        let mut reviewed = state(&raw);
        assert_eq!(rows(&parsed), tab.lines().collect::<Vec<_>>());
        assert_eq!(rows(&reviewed.finish()), rows(&parsed));
        assert_eq!(reviewed.finish().tab_groups, parsed.tab_groups);
        validate(&parsed).unwrap();
        reviewed.handle(Key::TransposeUp);
        assert_eq!(rows(&reviewed.finish()), rows(&parsed));
        assert!(warnings(&reviewed.finish()).contains(&UNCHANGED_WARNING.to_string()));
    }
    for count in [4, 5, 7] {
        let tab = "e|0--<12>--|\n".repeat(count);
        let song = parse(&tab);
        assert_eq!(rows(&song).len(), count);
        assert!(validate(&song)
            .unwrap_err()
            .to_string()
            .contains("six-string guitar only"));
    }
}

#[test]
fn wide_digit_leading_tabs_are_blocked_but_ordinary_prose_is_not_tablature() {
    let tab = format!("e|12{}|\n", "-".repeat(52)).repeat(6);
    let parsed = parse(&tab);
    assert_eq!(rows(&parsed).len(), 6);
    assert!(render(&parsed, false)
        .unwrap_err()
        .to_string()
        .contains("57 columns, maximum 56"));
    let reviewed = state(&tab).finish();
    assert!(validate(&reviewed).is_err());
    let prose = "Song: Prose\n[Verse]\nI have 12 bright balloons\nA|12 words about daylight\nB|0 ordinary words\nC|12 more ordinary words\nItem|12--description (one isolated table row)\nPlain words after the table\n";
    assert!(!has_tablature(&parse(prose)));
    assert!(!has_tablature(&state(prose).finish()));
}

#[test]
fn six_twelve_and_blank_boundaries_have_source_provenance() {
    for (raw, starts) in [
        (TAB.to_string(), vec![1]),
        (format!("{TAB}{TAB}"), vec![1, 7]),
        (format!("{TAB}\n{TAB}"), vec![1, 8]),
    ] {
        let song = parse(&raw);
        assert_eq!(
            song.tab_groups
                .iter()
                .map(|g| g.source_line)
                .collect::<Vec<_>>(),
            starts
        );
        assert!(song
            .tab_groups
            .iter()
            .all(|g| g.original_rows == 6 && g.end - g.start == 6));
        validate(&song).unwrap();
        assert!(warnings(&song).is_empty());
    }
}

#[test]
fn source_and_unresolved_keep_bulk_accept_rebuild_agree() {
    let raw = format!("Song: Synthetic\n[Intro]\n{TAB}\n{TAB}[Verse]\nC   G\nnew lyric\n{TAB}");
    let mut parsed = parse(&raw);
    apply_noninteractive_defaults(&mut parsed);
    let mut s = state(&raw);
    assert_eq!(s.finish().tab_groups, parsed.tab_groups);
    assert_eq!(
        render(&parsed, false).unwrap(),
        render(&s.finish(), false).unwrap()
    );
    s.handle(Key::Confirm);
    s.handle(Key::AutoAccept);
    assert_eq!(
        render(&parsed, false).unwrap(),
        render(&s.finish(), false).unwrap()
    );
    assert_eq!(rows(&parsed)[0], TAB.lines().next().unwrap());
    let classified = classify_lines(&raw);
    assert_eq!(
        classified
            .iter()
            .filter(|l| matches!(l.kind, LineKind::Fretboard(_)))
            .count(),
        18
    );
}

#[test]
fn discard_from_original_six_warns_without_merging_next_identical_block() {
    let raw = format!("{TAB}{TAB}");
    let mut s = state(&raw);
    s.handle(Key::Discard);
    let song = s.finish();
    assert_eq!(
        song.tab_groups
            .iter()
            .map(|g| g.end - g.start)
            .collect::<Vec<_>>(),
        [5, 6]
    );
    assert_eq!(song.tab_groups[1].source_line, 7);
    assert_eq!(warnings(&song).len(), 1);
    assert!(warnings(&song)[0].contains("5 retained rows (6 originally)"));
    validate(&song).unwrap();
    s.handle(Key::Undo);
    assert!(warnings(&s.finish()).is_empty());
}

#[test]
fn discarded_boundary_and_middle_rows_do_not_merge_source_groups() {
    let raw = format!("{}\n{}", "e|--0--|\n".repeat(6), "e|--0--|\n".repeat(6));
    let mut s = state(&raw);
    for i in [1, 3, 6, 7, 8, 9, 10, 11] {
        s.lines[i].verdict = Verdict::Discard;
    }
    let song = s.finish();
    assert_eq!(
        song.tab_groups
            .iter()
            .map(|g| (g.source_line, g.end - g.start))
            .collect::<Vec<_>>(),
        [(1, 4), (8, 1)]
    );
    assert_eq!(rows(&song), vec!["e|--0--|"; 5]);
    assert_eq!(warnings(&song).len(), 2);
    validate(&song).unwrap();
}

#[test]
fn reclassification_splits_original_group_fragments() {
    let mut s = state(TAB);
    s.lines[2].verdict = Verdict::Lyric;
    let song = s.finish();
    assert_eq!(
        song.tab_groups
            .iter()
            .map(|g| g.end - g.start)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(warnings(&song).len(), 2);
    validate(&song).unwrap();
}

#[test]
fn original_unsupported_runs_stay_unsupported_after_discards() {
    for count in [1, 4, 5, 7, 13] {
        let raw = "string1|--0--|\n".repeat(count);
        let mut s = state(&raw);
        let song = s.finish();
        assert_eq!(rows(&song).len(), count);
        assert_eq!(song.tab_groups[0].original_rows, count);
        assert!(render(&song, false)
            .unwrap_err()
            .to_string()
            .contains("six-string guitar only"));
        assert!(!warnings(&song).iter().any(|w| w.starts_with("Partial")));
        s.handle(Key::Discard);
        if count > 1 {
            assert!(validate(&s.finish()).is_err());
        }
    }
}

#[test]
fn width_and_glyph_errors_are_actionable_and_never_wrap() {
    let exact = format!("e|{}\n", "-".repeat(MAX_COLUMNS - 2)).repeat(6);
    validate(&parse(&exact)).unwrap();
    let wide = exact.replacen("e|", " e|", 1);
    let err = render(&parse(&wide), false).unwrap_err().to_string();
    assert!(err.contains("57 columns, maximum 56"), "{err}");
    assert!(err.contains("split the notation"));
    for bad in ["\t", "é", "\u{0001}"] {
        let raw = TAB.replacen("--0", &format!("--{bad}0"), 1);
        assert!(validate(&parse(&raw))
            .unwrap_err()
            .to_string()
            .contains("only printable ASCII"));
    }
}

#[test]
fn tab_only_preview_retains_alignment_and_mixed_export_order() {
    let raw = format!("Song: Literal\n[Verse]\n[C]Before\n{TAB}[G]After\n");
    let s = state(&raw);
    let song = s.finish();
    let preview = build(&song, &s.metadata);
    assert_eq!(
        preview
            .iter()
            .filter(|r| r.style == RowStyle::Tablature)
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
        TAB.lines().collect::<Vec<_>>()
    );
    let tex = render(&song, false).unwrap();
    assert!(tex.find("Before").unwrap() < tex.find("\\hbox{").unwrap());
    assert!(tex.find("\\hbox{").unwrap() < tex.find("After").unwrap());
    assert!(render(&parse(TAB), true)
        .unwrap()
        .contains("\\songcolumns{1}"));
}

#[test]
fn spaces_and_straight_quotes_use_literal_native_verified_font_mapping() {
    let raw = format!("{}\n{}", "  e|--'`--|  ", "e|-----|\n".repeat(5));
    let tex = render(&parse(&raw), false).unwrap();
    assert!(tex.contains("\\setbox0=\\hbox{\\char48\\relax}\\dimen0=\\wd0\\relax"));
    assert!(tex.contains("\\hbox{\\hskip\\dimen0\\relax \\hskip\\dimen0\\relax \\char101"));
    assert!(tex.contains("\\char13\\relax \\char18\\relax"));
    assert!(tex.contains("\\char124\\relax \\hskip\\dimen0\\relax \\hskip\\dimen0\\relax }"));
    for nonliteral in ["\\char32\\relax", "\\char39\\relax", "\\char96\\relax"] {
        assert!(!tex.contains(nonliteral));
    }
}

#[test]
fn hostile_tex_looking_payload_is_only_numeric_characters() {
    let hostile = r"e|--\input{x}%#$&_^{\end{songs}}~--|";
    let raw = format!("{hostile}\n{}", "e|-----|\n".repeat(5));
    let song = parse(&raw);
    let tex = render(&song, false).unwrap();
    let encoded: String = hostile
        .bytes()
        .map(|b| format!("\\char{b}\\relax "))
        .collect();
    assert!(tex.contains(&encoded));
    assert!(!tex.contains(hostile));
    assert!(!tex.contains("\\input{x}"));
}

#[test]
fn transposition_never_changes_frets_and_warns_only_at_nonzero_retained_offset() {
    let mut song = parse(&format!("[Verse]\n[C]Before\n{TAB}"));
    let original = rows(&song)
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    assert!(warnings(&song).is_empty());
    transpose_song(&mut song, 2);
    assert_eq!(rows(&song), original);
    assert_eq!(warnings(&song), [UNCHANGED_WARNING]);
    assert!(render(&song, false).unwrap().contains(UNCHANGED_WARNING));
    transpose_song(&mut song, -2);
    assert!(warnings(&song).is_empty());
    assert!(!render(&song, false).unwrap().contains(UNCHANGED_WARNING));
    transpose_song(&mut song, 12);
    assert_eq!(warnings(&song), [UNCHANGED_WARNING]);
    let mut s = state(TAB);
    s.transpose = 3;
    for line in &mut s.lines {
        line.verdict = Verdict::Discard;
    }
    assert!(!has_tablature(&s.finish()));
    assert!(warnings(&s.finish()).is_empty());
    let mut plain = parse("[C]Words");
    transpose_song(&mut plain, 2);
    assert!(warnings(&plain).is_empty());
}

#[test]
fn differing_tab_text_whitespace_and_group_boundaries_prevent_chorus_collapse() {
    for second in [TAB.replace("12", "13"), TAB.replace("  d|", " d|")] {
        let raw = format!("[Chorus]\n{TAB}[Chorus]\n{second}");
        let s = state(&raw);
        let song = s.finish();
        assert_eq!(
            render(&song, false)
                .unwrap()
                .matches("\\beginchorus")
                .count(),
            2
        );
        assert_eq!(
            build(&song, &s.metadata)
                .iter()
                .filter(|r| r.text == "[Chorus]")
                .count(),
            2
        );
        assert!(!audit_summary_line(&song, &raw).contains("collapsed"));
    }
    let identical = "e|--0--|\n".repeat(6);
    let raw = format!("[Chorus]\n{identical}[Chorus]\n{identical}\n{identical}");
    let mut s = state(&raw);
    // First chorus has one six-row group; second keeps 3+3 from two groups.
    for i in [11, 12, 13, 16, 17, 18] {
        s.lines[i].verdict = Verdict::Discard;
    }
    let song = s.finish();
    assert_eq!(rows(&song).len(), 12);
    assert_eq!(
        render(&song, false)
            .unwrap()
            .matches("\\beginchorus")
            .count(),
        2
    );
}

#[test]
fn identical_tab_choruses_collapse_and_book_body_remains_exact() {
    let raw = format!("[Chorus]\n{TAB}[Chorus]\n{TAB}");
    let song = parse(&raw);
    let body = render(&song, false).unwrap();
    assert_eq!(body.matches("\\beginchorus").count(), 1);
    assert!(audit_report(&song, &raw).contains("1 repeated chorus collapsed"));
    let book = compose_book(
        None,
        None,
        &[BookPart {
            body: body.clone(),
            chords: distinct_chords(&song),
        }],
        false,
    )
    .unwrap();
    assert!(book.contains(&body));
    assert!(book.contains("\\songcolumns{1}"));
}
