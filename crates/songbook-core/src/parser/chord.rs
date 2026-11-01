use regex::Regex;
use std::sync::OnceLock;

static CHORD_RE: OnceLock<Regex> = OnceLock::new();
static INLINE_CHORD_RE: OnceLock<Regex> = OnceLock::new();

fn chord_re() -> &'static Regex {
    CHORD_RE.get_or_init(|| {
        Regex::new(concat!(
            r"^(N\.?C\.?|X|",
            r"[A-G][#b]?",
            r"(?:(?:maj|min|mi|m|M|dim|aug|sus|add|°|ø|\+|-)?\d{0,2}",
            r"(?:(?:sus|add|maj|min|mi|m|dim|aug|b|#)\d{0,2})*",
            r"(?:/(?:[A-G][#b]?|\d{1,2}))?",
            r")?)$",
        ))
        .unwrap()
    })
}

/// Matches `[Am]`-style inline chord markers embedded in a lyric line.
fn inline_chord_re() -> &'static Regex {
    INLINE_CHORD_RE.get_or_init(|| Regex::new(r"\[([^\[\]]+)\]").unwrap())
}

/// A chord token found in a line, with its zero-based character column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordToken {
    pub col: usize,
    pub text: String,
}

/// A [`ChordToken`] plus the byte span of its bare text inside the scanned
/// line, so callers can rewrite tokens in place while keeping every other
/// column (used by transposition; `text_start`/`text_end` are byte offsets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordTokenSpan {
    pub token: ChordToken,
    /// Byte offset of the first byte of `token.text` in the scanned line.
    pub text_start: usize,
    /// Byte offset one past the last byte of `token.text`.
    pub text_end: usize,
}

/// Returns true if the token is a valid chord name.
pub fn is_chord(token: &str) -> bool {
    chord_re().is_match(token)
}

/// Extracts inline `[chord]` markers from a line.
pub fn inline_chords(line: &str) -> Vec<String> {
    inline_chord_re()
        .captures_iter(line)
        .map(|c| c[1].to_string())
        .collect()
}

/// Rewrites every inline `[marker]` through `map`, keeping the brackets and
/// all surrounding text verbatim. A `None` from `map` leaves that marker
/// unchanged (non-chord brackets are not the mapper's business).
pub fn map_inline_chords(line: &str, mut map: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last = 0;
    for captures in inline_chord_re().captures_iter(line) {
        let whole = captures.get(0).unwrap();
        out.push_str(&line[last..whole.start()]);
        match map(&captures[1]) {
            Some(replacement) => {
                out.push('[');
                out.push_str(&replacement);
                out.push(']');
            }
            None => out.push_str(whole.as_str()),
        }
        last = whole.end();
    }
    out.push_str(&line[last..]);
    out
}

/// Splits a line into whitespace/bar/comma separated tokens, keeping the
/// character column of each token. Purely punctuational tokens are dropped.
pub fn chord_tokens(line: &str) -> Vec<ChordToken> {
    chord_token_spans(line)
        .into_iter()
        .map(|span| span.token)
        .collect()
}

/// [`chord_tokens`] with the byte span of each token's bare text, for callers
/// that rewrite a line while preserving token columns.
pub fn chord_token_spans(line: &str) -> Vec<ChordTokenSpan> {
    let mut spans = Vec::new();
    let mut start: Option<(usize, usize)> = None;

    for (col, (byte, ch)) in line.char_indices().enumerate() {
        let delimiter = ch.is_whitespace() || matches!(ch, '|' | ',' | ';');
        if delimiter {
            if let Some((start_byte, start_col)) = start.take() {
                push_span(&mut spans, line, start_byte, start_col, byte);
            }
        } else if start.is_none() {
            start = Some((byte, col));
        }
    }
    if let Some((start_byte, start_col)) = start {
        push_span(&mut spans, line, start_byte, start_col, line.len());
    }
    spans
}

fn push_span(
    spans: &mut Vec<ChordTokenSpan>,
    line: &str,
    start_byte: usize,
    start_col: usize,
    end_byte: usize,
) {
    let raw = &line[start_byte..end_byte];
    let leading = raw.chars().take_while(|c| is_wrapping_punct(*c)).count();
    let trimmed = raw.trim_matches(is_wrapping_punct);
    if trimmed.is_empty() || trimmed.chars().all(|c| !c.is_alphanumeric()) {
        return;
    }
    // Wrapping punctuation is ASCII, so the leading char count is the byte
    // offset of the trimmed text.
    let text_start = start_byte + leading;
    spans.push(ChordTokenSpan {
        token: ChordToken {
            col: start_col + leading,
            text: trimmed.to_string(),
        },
        text_start,
        text_end: text_start + trimmed.len(),
    });
}

fn is_wrapping_punct(c: char) -> bool {
    matches!(
        c,
        '(' | ')' | '[' | ']' | '{' | '}' | ',' | '.' | ';' | ':' | '"' | '\'' | '*' | '_' | '|'
    )
}

/// Projects a chord-only line onto the lyric line below it, producing a lyric
/// line with inline `[Chord]` markers at the original chord columns.
pub fn merge_chord_line(chord_line: &str, lyric_line: &str) -> String {
    let mut chords: Vec<(usize, String)> = chord_tokens(chord_line)
        .into_iter()
        .filter(|token| is_chord(&token.text))
        .map(|token| (token.col, token.text))
        .collect();
    if chords.is_empty() {
        return lyric_line.to_string();
    }
    chords.sort_by_key(|(col, _)| *col);

    let lyric_chars: Vec<char> = lyric_line.chars().collect();
    let mut out = String::new();
    let mut chord_index = 0;

    for (index, ch) in lyric_chars.iter().enumerate() {
        while chord_index < chords.len() && chords[chord_index].0 <= index {
            out.push('[');
            out.push_str(&chords[chord_index].1);
            out.push(']');
            chord_index += 1;
        }
        out.push(*ch);
    }
    // Chords past the end of the lyric line are appended at the end.
    while chord_index < chords.len() {
        out.push('[');
        out.push_str(&chords[chord_index].1);
        out.push(']');
        chord_index += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_chords() {
        for c in [
            "A", "Am", "F#m7", "C/G", "Csus4", "Bbdim", "N.C.", "NC", "Gmaj7", "Dadd9", "C6/9",
            "E5", "A7sus4", "F#m7b5", "Cadd11", "G7b9", "Cmaj7#11", "D/F#", "A/C#", "C°", "Cø7",
            "C+", "C-", "CM7", "Amin", "Am7/G", "E/G#", "Bb7", "Asus2", "X",
        ] {
            assert!(is_chord(c), "expected {c} to be a chord");
        }
    }

    #[test]
    fn rejects_non_chords() {
        for c in [
            "Verse",
            "Chorus",
            "x2",
            "Intro",
            "hello",
            "2x",
            "Song:",
            "Standard",
            "Am,",
            "difficulty",
            "",
            "H",
        ] {
            assert!(!is_chord(c), "expected {c} NOT to be a chord");
        }
    }

    #[test]
    fn extracts_inline_chords() {
        let got = inline_chords("[Am]Let it [F]be, let it [C]be");
        assert_eq!(got, vec!["Am", "F", "C"]);
    }

    #[test]
    fn tokenizes_chords_with_columns_and_bars() {
        let got = chord_tokens("| Am | F  C |");
        assert_eq!(
            got,
            vec![
                ChordToken {
                    col: 2,
                    text: "Am".into()
                },
                ChordToken {
                    col: 7,
                    text: "F".into()
                },
                ChordToken {
                    col: 10,
                    text: "C".into()
                },
            ]
        );
    }

    #[test]
    fn tokenizes_repeat_and_punctuation() {
        let got = chord_tokens("(Am)  F, 2x  ...");
        assert_eq!(
            got,
            vec![
                ChordToken {
                    col: 1,
                    text: "Am".into()
                },
                ChordToken {
                    col: 6,
                    text: "F".into()
                },
                ChordToken {
                    col: 9,
                    text: "2x".into()
                },
            ]
        );
    }

    #[test]
    fn spans_point_at_the_bare_token_text() {
        let source = "| (Am)  F#m7 |";
        let spans = chord_token_spans(source);
        assert_eq!(
            spans,
            vec![
                ChordTokenSpan {
                    token: ChordToken {
                        col: 3,
                        text: "Am".into()
                    },
                    text_start: 3,
                    text_end: 5,
                },
                ChordTokenSpan {
                    token: ChordToken {
                        col: 8,
                        text: "F#m7".into()
                    },
                    text_start: 8,
                    text_end: 12,
                },
            ]
        );
        for span in &spans {
            assert_eq!(&source[span.text_start..span.text_end], span.token.text);
        }
    }

    #[test]
    fn spans_handle_multibyte_token_text() {
        let source = "C°7  F";
        let spans = chord_token_spans(source);
        assert_eq!(spans[0].token.text, "C°7");
        assert_eq!(&source[spans[0].text_start..spans[0].text_end], "C°7");
        assert_eq!(spans[1].token.col, 5);
    }

    #[test]
    fn maps_inline_chords_and_leaves_the_rest_verbatim() {
        let mapped = map_inline_chords("A [Am]B [Hx]C [F]D", |chord| {
            (chord == "Am").then(|| "Bm".to_string())
        });
        assert_eq!(mapped, "A [Bm]B [Hx]C [F]D");
    }

    #[test]
    fn merges_chords_at_columns() {
        let got = merge_chord_line("Am        F", "When I find myself");
        assert_eq!(got, "[Am]When I fin[F]d myself");
    }

    #[test]
    fn merges_multiple_chords_on_same_line() {
        let got = merge_chord_line("Am  F   C    G", "one two three four");
        assert_eq!(got, "[Am]one [F]two [C]three[G] four");
    }

    #[test]
    fn merges_chords_over_spaces() {
        let got = merge_chord_line(" Am", "  hello");
        assert_eq!(got, " [Am] hello");
    }

    #[test]
    fn merges_chords_beyond_end_of_lyric() {
        let got = merge_chord_line("Am             G", "Hello");
        assert_eq!(got, "[Am]Hello[G]");
    }

    #[test]
    fn merges_chord_at_end_of_lyric() {
        let got = merge_chord_line("Am  F", "Hello");
        assert_eq!(got, "[Am]Hell[F]o");
    }

    #[test]
    fn merge_without_chords_keeps_lyric() {
        assert_eq!(merge_chord_line("2x", "Hello"), "Hello");
    }

    #[test]
    fn merges_with_leading_indentation_aligned() {
        let got = merge_chord_line("  Am   F", "  When I find");
        assert_eq!(got, "  [Am]When [F]I find");
    }

    #[test]
    fn merges_bars_and_repeats() {
        let got = merge_chord_line("| Am | F | 2x", "Hello there");
        assert_eq!(got, "He[Am]llo t[F]here");
    }
}
