//! Chord transposition for parsed songs (roadmap item 3).
//!
//! Transposition is a pure rewrite of chord-name text: chord-only lines keep
//! their column layout (see [Column preservation](#column-preservation-for-chord-only-lines))
//! and inline `[Chord]` markers are substituted inside their brackets, so the
//! renderer (`latex::render`) keeps consuming the same `Song` shape unchanged
//! and is never re-implemented here.
//!
//! # Spelling policy: the song's key, else fixed ladders
//!
//! When the song's metadata carries a parseable key (`Key: C`, `Key: Am`,
//! `Key: Eb`, …), every transposed root — chord roots and slash basses alike —
//! is spelled from that key's table:
//!
//! - the seven degrees of the key's major scale keep the scale's own spelling
//!   (minor keys spell through their relative major: `Am` → C-major spelling,
//!   `Dm` → F-major spelling on the Bb side);
//! - the four lowered chromatic degrees (♭II, ♭III, ♭VI, ♭VII) spell through
//!   the flat ladder in every key: a chromatic root on a lowered degree is
//!   written flat in practice, and `Bb` — never `A#` — is a root name in G
//!   major as much as in C major;
//! - the remaining chromatic degree, the tritone ♯IV, follows the key's
//!   accidental side: sharp-side keys (C G D A E B F#) spell it sharp,
//!   flat-side keys (F Bb Eb Ab Db Gb) spell it flat.
//!
//! With a key, the request's direction stops mattering — the spelling is a
//! function of the target pitch class alone:
//!
//! | token | key | semitones | result |
//! |-------|-----|-----------|--------|
//! | `F`   | C   | +5        | `Bb`   |
//! | `C`   | G   | +1        | `C#`   |
//! | `C`   | F   | +5        | `F`    |
//! | `Bb`  | Eb  | +2        | `C`    |
//! | `C`   | C   | +6        | `F#`   |
//! | `Am`  | C   | +6        | `F#m`  |
//!
//! Extreme tonics whose scales would need a double accidental (G# major's
//! `F##`) degrade to the ladder for those degrees, which the
//! single-accidental chord grammar can carry.
//!
//! Without a parseable key — a song whose metadata has none, or whose `Key:`
//! value is not a tonic — the **fixed ladders** remain exactly as they were:
//! the sharp ladder for upward and zero steps, the flat ladder for downward
//! steps.
//!
//! | token | semitones | result |
//! |-------|-----------|--------|
//! | `Am`  | +2        | `Bm`   |
//! | `C`   | +1        | `C#`   |
//! | `F`   | +5        | `A#`   |
//! | `Bb`  | −1        | `A`    |
//! | `G/C` | −2        | `F/Bb` |
//!
//! Interval-based respelling (letter step plus computed accidental) was
//! rejected on purpose: it is musically stricter but produces double
//! accidentals for edge inputs (`Cb` +1 → `Dbb`), and the root grammar the
//! rest of this crate relies on (`is_chord`: one optional `#`/`b`) then
//! re-reads `Dbb` as `Db` with an opaque `b` suffix — a second transposition
//! would move the wrong pitch. The ladders always emit a single accidental, so
//! every transposed token round-trips through the same machinery that renders
//! it, and the result depends only on the target pitch class and the sign of
//! the step (deterministic). The cost is enharmonic plainness (`F` +5 → `A#`,
//! where the key-aware table above writes `Bb`); a parseable key removes it.
//!
//! In the ladder fallback a downward request keeps its direction at the
//! tritone: `C` −6 → `Gb` (flat ladder) while `C` +6 → `F#` (sharp ladder).
//! The step is `semitones mod 12` with the sign of the request kept, so `+8`
//! spells sharp (`G#`) and `−8` spells flat (`Ab`) at the same target pitch
//! class. Under a key the tritone follows the key's side instead: `C` −6 is
//! `F#` under `Key: C` and `B` under `Key: F`.
//!
//! Everything after the root is
//! opaque quality text (`m7`, `sus4`, `add9`, `°`, `ø`, …) and is carried over
//! verbatim; a `/bass` is transposed only when it is itself a letter name,
//! otherwise the whole token is left unchanged (`Am/G` transposes, `C/3` does
//! not). The `6/9` compound counts as quality, not as a numeric bass, so
//! `C6/9` transposes whole (`C6/9` +2 → `D6/9`). Tokens without a letter-name
//! root (`X`, `N.C.`, anything `is_chord` accepts only through its non-root
//! alternatives) return `None` and stay verbatim.
//!
//! The `Key:` metadata itself is transposed and spelled the same way —
//! `Key: C` +2 → `Key: D`, `Key: Eb` +2 → `Key: F`, `Key: Am` +2 → `Key: Bm`,
//! `Key: C` +1 → `Key: Db` — with the minor marker kept verbatim (`Amin` +2 →
//! `Bmin`). Only values that parse as a tonic (letter, optional `#`/`b`,
//! optional `m`/`min`/`minor` marker) are rewritten; anything else
//! (`Key: C major`) is left untouched, and its chords fall back to the
//! ladders.
//!
//! # Column preservation for chord-only lines
//!
//! Chord-only lines are positional (`ChordToken.col`), so a transposed token
//! that changes length must not move any *other* chord. The rewrite rule:
//!
//! - each transposed token keeps its original start column;
//! - its length delta (in characters) is absorbed by the gap that follows it,
//!   adjusted at the gap's **start** (immediately after the changed token), so
//!   bars, commas, and later chords all keep their original columns:
//!   - a **shorter** name inserts the missing spaces there (the final token on
//!     a line gets no padding — trailing text is kept verbatim);
//!   - a **longer** name gives up leading whitespace of the gap, but the gap
//!     is never emptied — at least one separator character (space, bar, comma)
//!     always remains between the two tokens, which for the common
//!     all-whitespace gap is exactly "never below one space";
//!   - when the gap cannot give up enough whitespace (a single space, or a gap
//!     whose leading characters are structural like `|`), the remainder pushes
//!     the rest of the line rightward: the following chords shift right. This
//!     is the documented no-slack behaviour — the alternative (shifting the
//!     grown token left, or eating structural characters) would move a chord
//!     off its lyric position or corrupt the line's bar layout.
//! - every non-token character (bars, `2x`, punctuation, indentation) is
//!   copied verbatim.
//!
//! Inline markers need no such care: the renderer extracts whatever sits
//! inside `[…]` (`render_lyric`) and emits `\[Name]` inline, so substituting
//! the chord name inside the brackets preserves semantics for any length.

use crate::parser::chord::{chord_token_spans, is_chord, map_inline_chords};
use crate::parser::song::{LineKind, Song};

/// Chromatic spellings used for upward and zero steps.
const SHARP_LADDER: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
/// Chromatic spellings used for downward steps.
const FLAT_LADDER: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

/// The letters `C D E F G A B`, indexed by letter.
const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
/// Natural pitch classes of [`LETTERS`], indexed by letter.
const LETTER_NATURALS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Key-aware spelling built from one parsed `Key:` metadata value.
///
/// [`Self::table`] spells every pitch class the way the key's (relative)
/// major would: scale degrees keep their scale spelling, the four lowered
/// chromatic degrees take the flat ladder, and the tritone degree follows the
/// key's accidental side (module docs). An unparseable value yields `None`
/// from [`Self::parse`] and leaves the fixed ladders in charge.
#[derive(Debug, Clone)]
struct KeySpellings {
    /// Pitch class of the metadata tonic itself (the minor tonic for minor
    /// keys; the table is built from its relative major).
    tonic_pc: i32,
    /// Byte length of the tonic inside the metadata value (letter plus
    /// optional accidental), so the `Key:` rewrite can keep the marker.
    tonic_len: usize,
    /// Spelling of every pitch class, indexed by pitch class.
    table: [String; 12],
}

impl KeySpellings {
    /// Parses a tonic: a letter `A`–`G` (case-insensitive), an optional
    /// `#`/`b`, and an optional minor marker `m`/`min`/`minor`
    /// (case-insensitive) that must end the value. `None` for anything else
    /// (`C major`, `Cm7`, `H`), which keeps the ladders and the metadata
    /// untouched.
    fn parse(value: &str) -> Option<KeySpellings> {
        let bytes = value.as_bytes();
        let letter = match bytes.first().map(|byte| byte.to_ascii_uppercase()) {
            Some(b'C') => 0,
            Some(b'D') => 1,
            Some(b'E') => 2,
            Some(b'F') => 3,
            Some(b'G') => 4,
            Some(b'A') => 5,
            Some(b'B') => 6,
            _ => return None,
        };
        let mut tonic_pc = LETTER_NATURALS[letter];
        let mut tonic_len = 1;
        if bytes.len() > tonic_len {
            match bytes[tonic_len] {
                b'#' => {
                    tonic_pc += 1;
                    tonic_len += 1;
                }
                b'b' => {
                    tonic_pc -= 1;
                    tonic_len += 1;
                }
                _ => {}
            }
        }
        let minor = match value[tonic_len..].to_ascii_lowercase().as_str() {
            "" => false,
            "m" | "min" | "minor" => true,
            _ => return None,
        };
        // Minor keys spell through their relative major, a minor third up:
        // two letter steps, three semitones.
        let (major_letter, major_pc) = if minor {
            ((letter + 2) % 7, tonic_pc + 3)
        } else {
            (letter, tonic_pc)
        };
        let major_pc = major_pc.rem_euclid(12);
        // The tritone follows the major tonic's own accidental side: a
        // spelled sharp or one of the natural letters C G D A E B is
        // sharp-side, a spelled flat or the natural F is flat-side.
        let sharp_side = match (major_pc - LETTER_NATURALS[major_letter]).rem_euclid(12) {
            1 => true,
            11 => false,
            _ => major_letter != 3,
        };
        let table = std::array::from_fn(|pc| {
            let pc = pc as i32;
            match (pc - major_pc).rem_euclid(12) {
                // Scale degrees keep the scale's own spelling.
                position @ (0 | 2 | 4 | 5 | 7 | 9 | 11) => {
                    let degree = match position {
                        0 => 0,
                        2 => 1,
                        4 => 2,
                        5 => 3,
                        7 => 4,
                        9 => 5,
                        _ => 6,
                    };
                    let letter = (major_letter + degree) % 7;
                    let letter_char = LETTERS[letter];
                    match (pc - LETTER_NATURALS[letter]).rem_euclid(12) {
                        0 => letter_char.to_string(),
                        1 => format!("{letter_char}#"),
                        11 => format!("{letter_char}b"),
                        // An extreme tonic would need a double accidental
                        // here, which the chord grammar cannot carry: degrade
                        // that degree to the ladder.
                        _ => ladder(pc, sharp_side).to_string(),
                    }
                }
                // The tritone degree follows the key's side.
                6 => ladder(pc, sharp_side).to_string(),
                // The lowered degrees borrow flat in every key.
                _ => FLAT_LADDER[pc as usize].to_string(),
            }
        });
        Some(KeySpellings {
            tonic_pc,
            tonic_len,
            table,
        })
    }

    /// Spelling of a pitch class under this key.
    fn spell(&self, pc: i32) -> &str {
        &self.table[pc.rem_euclid(12) as usize]
    }

    /// The transposed `Key:` metadata value: the tonic moves by `semitones`
    /// and is spelled through this key's own table, while the minor marker
    /// and anything after the tonic is carried over verbatim.
    fn transposed_value(&self, value: &str, semitones: i32) -> String {
        format!(
            "{}{}",
            self.spell(self.tonic_pc + semitones),
            &value[self.tonic_len..]
        )
    }
}

/// Transposes one chord-name token by `semitones` (negative = down).
///
/// Returns the respelled token, or `None` when the token has no letter-name
/// root or carries a non-letter bass (`C/3`); `None` means "leave unchanged".
/// A step of zero respells through the sharp ladder (`Db` +0 → `C#`), which is
/// why [`transpose_song`] short-circuits multiples of twelve instead.
///
/// ```
/// # use songbook_core::transpose::transpose_chord;
/// assert_eq!(transpose_chord("Am", 2).as_deref(), Some("Bm"));
/// assert_eq!(transpose_chord("F#m7/C#", -2).as_deref(), Some("Em7/B"));
/// assert_eq!(transpose_chord("N.C.", 3), None);
/// ```
pub fn transpose_chord(token: &str, semitones: i32) -> Option<String> {
    transpose_chord_in_key(token, semitones, None)
}

/// [`transpose_chord`] under an optional key-aware spelling: `Some(key)` spells
/// every root from the key's table (module docs), `None` keeps the fixed
/// ladders.
fn transpose_chord_in_key(
    token: &str,
    semitones: i32,
    key: Option<&KeySpellings>,
) -> Option<String> {
    if !is_chord(token) {
        return None;
    }
    let delta = signed_delta(semitones);
    let (main, bass) = match token.find('/') {
        Some(slash) => (&token[..slash], Some(&token[slash + 1..])),
        None => (token, None),
    };
    let (suffix, root_pc) = split_root(main)?;
    // `C6/9` splits into root, quality `6`, and a bass that merely looks
    // numeric; the compound is one opaque quality and rides along verbatim
    // instead of failing the letter-bass check below.
    let six_nine = bass == Some("9") && suffix.ends_with('6');
    let mut transposed = spell_root(root_pc + delta, delta, key);
    transposed.push_str(suffix);
    if six_nine {
        transposed.push_str("/9");
    } else if let Some(bass) = bass {
        let bass_pc = letter_bass_pc(bass)?;
        transposed.push('/');
        transposed.push_str(&spell_root(bass_pc + delta, delta, key));
    }
    Some(transposed)
}

/// Normalizes `semitones` to the net step whose sign decides the ladder:
/// non-negative requests step up through `semitones mod 12` (sharp ladder),
/// negative requests step down through the same residue (flat ladder). A
/// negative request keeps its direction at the tritone, so `C` +6 → `F#`
/// while `C` −6 → `Gb`.
fn signed_delta(semitones: i32) -> i32 {
    let normalized = semitones.rem_euclid(12);
    if semitones >= 0 || normalized == 0 {
        normalized
    } else {
        normalized - 12
    }
}

/// Splits the chord root `[A-G][#b]?` off a slash-free token, returning the
/// verbatim quality suffix and the root's pitch class. `None` for tokens whose
/// first symbol is not a letter-name root (`N.C.`, `X`, `NC`, …).
fn split_root(main: &str) -> Option<(&str, i32)> {
    let mut chars = main.chars();
    let natural = match chars.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    match rest.chars().next() {
        Some('#') => Some((&rest[1..], natural + 1)),
        Some('b') => Some((&rest[1..], natural - 1)),
        _ => Some((rest, natural)),
    }
}

/// Pitch class of a plain letter-name bass `[A-G][#b]?`; `None` for numeric or
/// otherwise decorated basses, which transposition leaves untouched.
fn letter_bass_pc(bass: &str) -> Option<i32> {
    let (suffix, pc) = split_root(bass)?;
    suffix.is_empty().then_some(pc.rem_euclid(12))
}

/// The fixed-ladder spelling of a pitch class: the sharp ladder on the sharp
/// side (upward and zero steps, or the tritone under a sharp-side key), the
/// flat ladder on the flat side.
fn ladder(pc: i32, sharp: bool) -> &'static str {
    let index = pc.rem_euclid(12) as usize;
    if sharp {
        SHARP_LADDER[index]
    } else {
        FLAT_LADDER[index]
    }
}

fn spell(pc: i32, delta: i32) -> &'static str {
    ladder(pc, delta >= 0)
}

/// Spells one transposed root pitch class: through the key's table when a key
/// is known (the key decides, not the request's direction), else through the
/// fixed ladder chosen by the request's sign.
fn spell_root(pc: i32, delta: i32, key: Option<&KeySpellings>) -> String {
    match key {
        Some(key) => key.spell(pc).to_string(),
        None => spell(pc, delta).to_string(),
    }
}

/// Transposes every chord token in `song` by `semitones` (negative = down) in
/// place and returns how many chord tokens were transposed. Multiples of
/// twelve leave chord/key text untouched (`0` returned), so a zero offset keeps
/// existing byte-exact output. The net requested offset is recorded separately
/// for unchanged-tab warnings, including nonzero full-octave requests.
///
/// A parseable `Key:` metadata value spells every root through that key and is
/// itself rewritten (`Key: C` +2 → `Key: D`); without one, the fixed ladders
/// apply and the metadata is left untouched (module docs).
///
/// Chord-only lines are rewritten with column preservation (module docs);
/// inline `[Chord]` markers in lyric lines are substituted inside their
/// brackets. Unreviewed (`Ambiguous`) lines are deliberately untouched: after
/// the non-interactive defaults they are lyric lines and transpose like any
/// other.
pub fn transpose_song(song: &mut Song, semitones: i32) -> usize {
    if crate::tablature::has_tablature(song) {
        song.tablature_transpose = song.tablature_transpose.saturating_add(semitones);
    }
    if semitones.rem_euclid(12) == 0 {
        return 0;
    }
    // The key is parsed once and its table spells every root below; a missing
    // or unparseable key leaves the fixed ladders in charge.
    let key = song.key.as_deref().and_then(KeySpellings::parse);
    let mut new_key = None;
    if let (Some(key), Some(value)) = (&key, &song.key) {
        new_key = Some(key.transposed_value(value, semitones));
    }
    let mut count = 0;
    for section in &mut song.sections {
        for line in &mut section.lines {
            match line {
                LineKind::ChordOnly(text) => {
                    let (rebuilt, transposed) = transpose_chord_line(text, semitones, key.as_ref());
                    if transposed > 0 {
                        *text = rebuilt;
                        count += transposed;
                    }
                }
                LineKind::Lyric(text) => {
                    let mut transposed = 0;
                    let rebuilt = map_inline_chords(text, |chord| {
                        transpose_chord_in_key(chord, semitones, key.as_ref())
                            .inspect(|_| transposed += 1)
                    });
                    if transposed > 0 {
                        *text = rebuilt;
                        count += transposed;
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(new_key) = new_key {
        song.key = Some(new_key);
    }
    count
}

/// Rewrites one chord-only line with column preservation, returning the
/// rebuilt line and the number of transposed tokens (module docs for the
/// exact whitespace rules).
fn transpose_chord_line(line: &str, semitones: i32, key: Option<&KeySpellings>) -> (String, usize) {
    let spans = chord_token_spans(line);
    let mut out = String::with_capacity(line.len() + 2);
    let mut cursor = 0usize;
    let mut count = 0usize;

    for (index, span) in spans.iter().enumerate() {
        out.push_str(&line[cursor..span.text_start]);
        let Some(new) = transpose_chord_in_key(&span.token.text, semitones, key) else {
            out.push_str(&line[span.text_start..span.text_end]);
            cursor = span.text_end;
            continue;
        };
        count += 1;
        out.push_str(&new);
        let delta = new.chars().count() as i64 - span.token.text.chars().count() as i64;
        let next_start = spans
            .get(index + 1)
            .map_or(line.len(), |next| next.text_start);
        let gap = &line[span.text_end..next_start];
        if index + 1 == spans.len() || delta == 0 {
            // The trailing text has no column semantics; an unchanged length
            // needs no adjustment at all.
            out.push_str(gap);
        } else if delta < 0 {
            // Pad right after the (now shorter) name: bars and later chords
            // keep their columns.
            for _ in 0..-delta {
                out.push(' ');
            }
            out.push_str(gap);
        } else {
            // Give up leading whitespace, never emptying the gap; whatever
            // cannot be absorbed pushes the rest of the line rightward.
            let mut remainder = gap;
            let mut absorbed = 0i64;
            while absorbed < delta {
                match remainder.chars().next() {
                    Some(space) if space.is_whitespace() && remainder.len() > space.len_utf8() => {
                        remainder = &remainder[space.len_utf8()..];
                        absorbed += 1;
                    }
                    _ => break,
                }
            }
            out.push_str(remainder);
        }
        cursor = next_start;
    }
    out.push_str(&line[cursor..]);
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::latex::render::render;
    use crate::parser::classify::parse;
    use crate::parser::song::{LineKind, Section, SectionKind};

    /// Pitch class of a spelled note, computed here independently of the
    /// implementation under test.
    fn pc(note: &str) -> i32 {
        let mut chars = note.chars();
        let natural: i32 = match chars.next() {
            Some('C') => 0,
            Some('D') => 2,
            Some('E') => 4,
            Some('F') => 5,
            Some('G') => 7,
            Some('A') => 9,
            Some('B') => 11,
            other => panic!("not a note letter: {other:?}"),
        };
        let accidental: i32 = match chars.next() {
            Some('#') => 1,
            Some('b') => -1,
            None => 0,
            other => panic!("not an accidental: {other:?}"),
        };
        (natural + accidental).rem_euclid(12)
    }

    #[test]
    fn every_root_transposes_through_every_step() {
        const SHARP_ROOTS: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        for (root_index, root) in SHARP_ROOTS.iter().enumerate() {
            let base = root_index as i32;
            for step in 1..=11i32 {
                let up = transpose_chord(&format!("{root}m7"), step).unwrap();
                let expected = SHARP_LADDER[(base + step).rem_euclid(12) as usize];
                assert_eq!(up, format!("{expected}m7"), "{root}m7 +{step}");
                assert_eq!(pc(expected), (base + step).rem_euclid(12));

                let down = transpose_chord(&format!("{root}m7"), -step).unwrap();
                let expected = FLAT_LADDER[(base - step).rem_euclid(12) as usize];
                assert_eq!(down, format!("{expected}m7"), "{root}m7 -{step}");
                assert_eq!(pc(expected), (base - step).rem_euclid(12));
            }
        }
    }

    #[test]
    fn documents_the_ladder_examples() {
        assert_eq!(transpose_chord("Am", 2).as_deref(), Some("Bm"));
        assert_eq!(transpose_chord("C", 1).as_deref(), Some("C#"));
        assert_eq!(transpose_chord("F", 5).as_deref(), Some("A#"));
        assert_eq!(transpose_chord("Bb", -1).as_deref(), Some("A"));
        assert_eq!(transpose_chord("G/C", -2).as_deref(), Some("F/Bb"));
    }

    #[test]
    fn tritone_keeps_the_requested_direction() {
        assert_eq!(transpose_chord("C", 6).as_deref(), Some("F#"));
        assert_eq!(transpose_chord("C", -6).as_deref(), Some("Gb"));
        // Octave-wrapped negative requests keep their downward direction.
        assert_eq!(transpose_chord("C", -6 - 12).as_deref(), Some("Gb"));
    }

    #[test]
    fn zero_step_respells_through_the_sharp_ladder() {
        assert_eq!(transpose_chord("Db", 0).as_deref(), Some("C#"));
        assert_eq!(transpose_chord("Fb", 0).as_deref(), Some("E"));
        // Twelve semitones normalize to a zero step, which is why song-level
        // transposition short-circuits multiples of twelve instead.
        assert_eq!(transpose_chord("Am", 12).as_deref(), Some("Am"));
    }

    #[test]
    fn the_requests_sign_decides_the_ladder() {
        // Both land on pitch class 8, spelled from opposite directions.
        assert_eq!(transpose_chord("C", 8).as_deref(), Some("G#"));
        assert_eq!(transpose_chord("C", -4).as_deref(), Some("Ab"));
        assert_eq!(transpose_chord("C", -8).as_deref(), Some("E"));
    }

    #[test]
    fn quality_suffixes_survive_verbatim() {
        for suffix in [
            "m", "m7", "7sus4", "dim", "aug", "add9", "°", "ø7", "sus2", "maj7", "M7", "6", "9",
            "5", "7b9", "#11", "m7b5", "+", "-",
        ] {
            let token = format!("F{suffix}");
            let transposed = transpose_chord(&token, 2).unwrap();
            assert!(
                transposed.as_str().ends_with(suffix),
                "{token} → {transposed}"
            );
            assert!(transposed.starts_with('G'), "{token} → {transposed}");
            assert!(is_chord(&transposed), "{token} → {transposed}");
        }
    }

    #[test]
    fn slash_chords_transpose_root_and_letter_bass() {
        assert_eq!(transpose_chord("Am/G", 2).as_deref(), Some("Bm/A"));
        assert_eq!(transpose_chord("D/F#", -1).as_deref(), Some("Db/F"));
        assert_eq!(transpose_chord("C/E", 1).as_deref(), Some("C#/F"));
        assert_eq!(
            transpose_chord("Cmaj7#11/G", 2).as_deref(),
            Some("Dmaj7#11/A")
        );
    }

    #[test]
    fn numeric_and_odd_basses_leave_the_token_unchanged() {
        // `C6/9` is absent here on purpose: the 6/9 compound counts as quality
        // and transposes (see `six_nine_compounds_transpose_as_one_quality`).
        for token in ["C/3", "C/9", "Am/12", "C/E/G", "C/", "Am/g"] {
            assert_eq!(transpose_chord(token, 2), None, "{token}");
        }
    }

    #[test]
    fn six_nine_compounds_transpose_as_one_quality() {
        assert_eq!(transpose_chord("C6/9", 2).as_deref(), Some("D6/9"));
        assert_eq!(transpose_chord("C6/9", 5).as_deref(), Some("F6/9"));
        assert_eq!(transpose_chord("Am6/9", -2).as_deref(), Some("Gm6/9"));
        assert_eq!(transpose_chord("Cmaj6/9", 2).as_deref(), Some("Dmaj6/9"));
        // The general numeric-bass rule is untouched: only the 6/9 compound
        // (quality ending in `6`, slash part exactly `9`) is special-cased.
        assert_eq!(transpose_chord("C13/9", 2), None);
        assert_eq!(transpose_chord("C6/10", 2), None);
        // Key-aware spelling moves only the root.
        assert_eq!(transposed_in("C", "C6/9", 5).as_deref(), Some("F6/9"));
        assert_eq!(transposed_in("C", "Bb6/9", 2).as_deref(), Some("C6/9"));
    }

    /// Transposes `token` under the key parsed from `key`.
    fn transposed_in(key: &str, token: &str, semitones: i32) -> Option<String> {
        transpose_chord_in_key(token, semitones, Some(&KeySpellings::parse(key).unwrap()))
    }

    #[test]
    fn key_aware_spelling_under_c_writes_bb_where_the_ladder_wrote_a_sharp() {
        // The headline: the ladder's A# becomes the key's Bb.
        assert_eq!(transposed_in("C", "F", 5).as_deref(), Some("Bb"));
        // The key, not the request's sign, decides: the same pitch class from
        // below spells the same way.
        assert_eq!(transposed_in("C", "F", -7).as_deref(), Some("Bb"));
        // The lowered degrees borrow flat even though C sits on the sharp
        // side.
        assert_eq!(transposed_in("C", "C", 1).as_deref(), Some("Db"));
        assert_eq!(transposed_in("C", "E", -1).as_deref(), Some("Eb"));
        assert_eq!(transposed_in("C", "G", 1).as_deref(), Some("Ab"));
        assert_eq!(transposed_in("C", "B", -1).as_deref(), Some("Bb"));
        // The tritone degree follows the key's sharp side, in both directions.
        assert_eq!(transposed_in("C", "C", 6).as_deref(), Some("F#"));
        assert_eq!(transposed_in("C", "C", -6).as_deref(), Some("F#"));
        // In-scale degrees keep their scale spelling.
        assert_eq!(transposed_in("C", "A", 2).as_deref(), Some("B"));
        assert_eq!(transposed_in("C", "F", 7).as_deref(), Some("C"));
    }

    #[test]
    fn key_aware_spelling_under_g_spells_its_tritone_degree_sharp() {
        // C is a tritone above G's tonic, and G sits on the sharp side.
        assert_eq!(transposed_in("G", "C", 1).as_deref(), Some("C#"));
        assert_eq!(transposed_in("G", "C", -11).as_deref(), Some("C#"));
        // The lowered degrees still borrow flat in a sharp key.
        assert_eq!(transposed_in("G", "A", 1).as_deref(), Some("Bb"));
        assert_eq!(transposed_in("G", "D", 1).as_deref(), Some("Eb"));
        assert_eq!(transposed_in("G", "G", 1).as_deref(), Some("Ab"));
        // In-scale degrees keep the scale spelling.
        assert_eq!(transposed_in("G", "C", 7).as_deref(), Some("G"));
        assert_eq!(transposed_in("G", "F", 2).as_deref(), Some("G"));
    }

    #[test]
    fn key_aware_spelling_under_f_spells_flats_and_a_natural_tritone() {
        // C is F's dominant; up a fifth it lands on the tonic itself.
        assert_eq!(transposed_in("F", "C", 5).as_deref(), Some("F"));
        // F is the tritone degree of F major; the flat side spells the
        // natural B in both directions.
        assert_eq!(transposed_in("F", "F", 6).as_deref(), Some("B"));
        assert_eq!(transposed_in("F", "F", -6).as_deref(), Some("B"));
        // Lowered degrees spell flat.
        assert_eq!(transposed_in("F", "C", 1).as_deref(), Some("Db"));
        assert_eq!(transposed_in("F", "B", 7).as_deref(), Some("Gb"));
        // In-scale degrees.
        assert_eq!(transposed_in("F", "D", 5).as_deref(), Some("G"));
        assert_eq!(transposed_in("F", "Bb", 2).as_deref(), Some("C"));
    }

    #[test]
    fn key_aware_spelling_under_eb() {
        assert_eq!(transposed_in("Eb", "Bb", 2).as_deref(), Some("C"));
        assert_eq!(transposed_in("Eb", "Bb", -10).as_deref(), Some("C"));
        // Ab is the tritone degree of Eb; the flat side spells the natural A.
        assert_eq!(transposed_in("Eb", "Ab", 1).as_deref(), Some("A"));
        assert_eq!(transposed_in("Eb", "Eb", 6).as_deref(), Some("A"));
        // In-scale degrees.
        assert_eq!(transposed_in("Eb", "Ab", 2).as_deref(), Some("Bb"));
        assert_eq!(transposed_in("Eb", "G", 5).as_deref(), Some("C"));
        // The borrowed bIII spells flat even where the transposed-to key
        // would call it F# — the documented cost of spelling from the song's
        // own key.
        assert_eq!(transposed_in("Eb", "F", 1).as_deref(), Some("Gb"));
    }

    #[test]
    fn minor_keys_spell_through_their_relative_major() {
        // Am → C-major spelling: flat borrowings, sharp tritone.
        assert_eq!(transposed_in("Am", "F", 5).as_deref(), Some("Bb"));
        assert_eq!(transposed_in("Am", "C", 6).as_deref(), Some("F#"));
        assert_eq!(transposed_in("Am", "Am", 2).as_deref(), Some("Bm"));
        // Dm → F-major spelling on the Bb side: natural tritone, flat
        // borrowings.
        assert_eq!(transposed_in("Dm", "C", 5).as_deref(), Some("F"));
        assert_eq!(transposed_in("Dm", "F", 6).as_deref(), Some("B"));
        assert_eq!(transposed_in("Dm", "Bb", 2).as_deref(), Some("C"));
        assert_eq!(transposed_in("Dm", "C", 1).as_deref(), Some("Db"));
        // Gm → Bb-major spelling.
        assert_eq!(transposed_in("Gm", "C", 5).as_deref(), Some("F"));
        assert_eq!(transposed_in("Gm", "F", 6).as_deref(), Some("B"));
    }

    #[test]
    fn key_aware_spelling_covers_slash_basses() {
        assert_eq!(transposed_in("C", "Am/G", 5).as_deref(), Some("Dm/C"));
        assert_eq!(transposed_in("F", "C/E", 5).as_deref(), Some("F/A"));
        assert_eq!(transposed_in("G", "D/F#", 7).as_deref(), Some("A/C#"));
    }

    #[test]
    fn extreme_keys_spell_true_scale_degrees_or_degrade_to_the_ladder() {
        // F# major's seventh degree is E#, Gb major's fourth is Cb; both are
        // single accidentals and round-trip through the chord grammar.
        assert_eq!(transposed_in("F#", "B", -6).as_deref(), Some("E#"));
        assert!(is_chord("E#"));
        assert_eq!(transposed_in("Gb", "F", 6).as_deref(), Some("Cb"));
        assert!(is_chord("Cb"));
        // G# major would need F## for its seventh degree; that degree
        // degrades to the ladder's single accidental instead.
        assert_eq!(transposed_in("G#", "B", -4).as_deref(), Some("G"));
        assert!(is_chord("G"));
    }

    #[test]
    fn key_aware_song_transposition_spells_chords_and_the_key_header() {
        let mut song = parse("Key: C\n\n[Intro]\nF  Bb  C\n\n[Verse]\n[F]Hello [Bb]world");
        assert_eq!(transpose_song(&mut song, 5), 5);
        assert_eq!(song.key.as_deref(), Some("F"));
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("Bb Eb  F".into())
        );
        assert_eq!(
            song.sections[1].lines[0],
            LineKind::Lyric("[Bb]Hello [Eb]world".into())
        );
    }

    #[test]
    fn without_a_key_the_same_song_keeps_the_ladder_spellings() {
        let mut song = parse("[Intro]\nF  Bb  C\n\n[Verse]\n[F]Hello [Bb]world");
        assert_eq!(transpose_song(&mut song, 5), 5);
        assert_eq!(song.key, None);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("A# D#  F".into())
        );
        assert_eq!(
            song.sections[1].lines[0],
            LineKind::Lyric("[A#]Hello [D#]world".into())
        );
    }

    #[test]
    fn the_key_metadata_is_transposed_and_spelled_the_same_way() {
        let mut song = parse("Key: C\n\n[Verse]\n[Am]Hello [F]world");
        assert_eq!(transpose_song(&mut song, 2), 2);
        assert_eq!(song.key.as_deref(), Some("D"));

        let mut song = parse("Key: Eb\n\n[Verse]\n[Am]Hello");
        transpose_song(&mut song, 2);
        assert_eq!(song.key.as_deref(), Some("F"));

        let mut song = parse("Key: Am\n\n[Verse]\n[Am]Hello");
        transpose_song(&mut song, 2);
        assert_eq!(song.key.as_deref(), Some("Bm"));

        // A semitone up spells the new tonic from the old key's side.
        for (from, to) in [("C", "Db"), ("G", "Ab"), ("F", "Gb")] {
            let mut song = parse(&format!("Key: {from}\n\n[Verse]\n[Am]Hello"));
            transpose_song(&mut song, 1);
            assert_eq!(song.key.as_deref(), Some(to), "Key: {from} +1");
        }
    }

    #[test]
    fn the_key_rewrite_keeps_the_marker_and_accepts_any_case() {
        let mut song = parse("Key: Amin\n\n[Verse]\n[Am]Hello");
        transpose_song(&mut song, 2);
        assert_eq!(song.key.as_deref(), Some("Bmin"));

        let mut song = parse("key: am\n\n[Verse]\n[Am]Hello");
        transpose_song(&mut song, 2);
        assert_eq!(song.key.as_deref(), Some("Bm"));
    }

    #[test]
    fn unparseable_key_metadata_is_untouched_and_keeps_the_ladders() {
        for value in ["C major", "Cm7", "A minor", "C##", "Do", "H", "X"] {
            let mut song = parse(&format!("Key: {value}\n\n[Verse]\n[F]Hello"));
            assert_eq!(transpose_song(&mut song, 5), 1, "Key: {value}");
            assert_eq!(song.key.as_deref(), Some(value), "Key: {value}");
            assert_eq!(
                song.sections[0].lines[0],
                LineKind::Lyric("[A#]Hello".into()),
                "Key: {value}"
            );
        }
    }

    #[test]
    fn full_octave_offsets_leave_the_key_metadata_alone() {
        let mut song = parse("Key: C\n\n[Verse]\n[Am]Hello");
        assert_eq!(transpose_song(&mut song, 12), 0);
        assert_eq!(song.key.as_deref(), Some("C"));
        assert_eq!(transpose_song(&mut song, 0), 0);
        assert_eq!(song.key.as_deref(), Some("C"));
    }

    #[test]
    fn tokens_without_a_letter_root_are_unchanged() {
        for token in ["X", "N.C.", "NC", "N. C.", "", "H", "Verse", "2x", "x2"] {
            assert_eq!(transpose_chord(token, 2), None, "{token}");
        }
    }

    #[test]
    fn double_accidentals_follow_the_machinerys_single_accidental_reading() {
        // `is_chord` accepts `Gbb` (root `Gb` plus an opaque `b` suffix), so
        // transposition shifts that root and carries the suffix verbatim. The
        // result is a valid token whose pitch class matches the machinery's
        // own reading of the input.
        let transposed = transpose_chord("Gbb", 2).unwrap();
        assert_eq!(transposed, "G#b");
        assert!(is_chord(&transposed));
        assert_eq!(pc("G#"), (pc("Gb") + 2) % 12);
    }

    #[test]
    fn transposes_inline_markers_without_touching_lyrics() {
        let mut song = parse("[Verse]\n[Am]Let it [F]be, let it [C]be");
        let count = transpose_song(&mut song, 2);
        assert_eq!(count, 3);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::Lyric("[Bm]Let it [G]be, let it [D]be".into())
        );
    }

    #[test]
    fn inline_markers_with_invalid_chords_stay_partial() {
        // A parsed line mixing valid and invalid markers is `Ambiguous`, so
        // build the lyric line directly to pin the marker-level rule: valid
        // markers transpose, invalid ones stay verbatim.
        let mut song = Song::default();
        song.sections.push(Section {
            kind: SectionKind::Verse,
            lines: vec![LineKind::Lyric("[Am]Hello [Hx]weird".into())],
        });
        let count = transpose_song(&mut song, 2);
        assert_eq!(count, 1);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::Lyric("[Bm]Hello [Hx]weird".into())
        );
    }

    #[test]
    fn chord_only_columns_survive_equal_length_changes() {
        let mut song = parse("[Intro]\nAm   F   C\n\n[Verse]\n[Am]Hello");
        assert_eq!(transpose_song(&mut song, 2), 4);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("Bm   G   D".into())
        );
    }

    #[test]
    fn chord_only_columns_survive_longer_names_with_slack() {
        // F → A# grows by one; the two-space gap gives one up, so the trailing
        // C→F keeps its column (7) and Dm keeps its column untouched.
        let mut song = parse("[Intro]\nAm  F  C\n\n[Verse]");
        assert_eq!(transpose_song(&mut song, 5), 3);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("Dm  A# F".into())
        );
    }

    #[test]
    fn chord_only_columns_survive_shorter_names() {
        // A#m → Am shrinks by one; the space is inserted right after the name,
        // so the bar (6) and the F→E slot (8) keep their columns.
        let mut song = parse("[Intro]\n| A#m | F |\n\n[Verse]");
        assert_eq!(transpose_song(&mut song, -1), 2);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("| Am  | E |".into())
        );
    }

    #[test]
    fn no_slack_pushes_the_rest_of_the_line_rightward() {
        // The gap is a single space: it must never vanish, so the following
        // chord moves right by one instead.
        let mut song = parse("[Intro]\nAm F\n\n[Verse]");
        assert_eq!(transpose_song(&mut song, 1), 2);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("A#m F#".into())
        );

        // A structural-only gap (`|`) has no whitespace to give either.
        let mut song = parse("[Intro]\nAm|F\n\n[Verse]");
        assert_eq!(transpose_song(&mut song, 1), 2);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("A#m|F#".into())
        );
    }

    #[test]
    fn bars_and_repeats_are_preserved() {
        // Every bar and repeat keeps its column: Cm@2, G#@7, D#@11, A#@15,
        // bars @5/9/13/17, `2x` @19 — exactly the original layout; the three
        // grown names ate the space before their following bar.
        let mut song = parse("[Intro]\n| Am | F | C | G | 2x\n\n[Verse]\n[Am]Hello");
        assert_eq!(transpose_song(&mut song, 3), 5);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("| Cm | G#| D#| A#| 2x".into())
        );
    }

    #[test]
    fn non_chord_tokens_and_punctuation_survive() {
        let mut song = parse("[Intro]\n(Am)  F, N.C.  2x ...\n\n[Verse]");
        assert_eq!(transpose_song(&mut song, 2), 2);
        assert_eq!(
            song.sections[0].lines[0],
            LineKind::ChordOnly("(Bm)  G, N.C.  2x ...".into())
        );
    }

    #[test]
    fn zero_semitones_is_an_exact_identity() {
        let raw = concat!(
            "Song: Spaced\nArtist: Band\n\n[Intro]\n",
            "| Am7 | F#m7b5 | C6/9 | N.C. | 2x   \n\n",
            "[Verse]\n[Am]Let it [F]be, let it [C]be\n",
            "  Dm7   G7   \n[Chorus]\n[C°]Odd [Cø7]names [C+]too\n"
        );
        let song = parse(raw);
        let before = format!("{song:?}");
        let before_tex = render(&song, false).unwrap();
        let mut song = song;
        assert_eq!(transpose_song(&mut song, 0), 0);
        assert_eq!(format!("{song:?}"), before);
        assert_eq!(transpose_song(&mut song, 12), 0);
        assert_eq!(transpose_song(&mut song, -24), 0);
        assert_eq!(format!("{song:?}"), before);
        assert_eq!(render(&song, false).unwrap(), before_tex);
    }

    #[test]
    fn ambiguous_lines_are_left_to_their_verdict() {
        let mut song = parse("[Verse]\n[Am]Hello\nmystery [Am]line [Hx]junk");
        // The ambiguous line stays raw; only the lyric marker transposes.
        assert_eq!(transpose_song(&mut song, 2), 1);
        assert_eq!(
            song.sections[0].lines[1],
            LineKind::Ambiguous("mystery [Am]line [Hx]junk".into())
        );
    }

    #[test]
    fn transposed_song_renders_transposed_chord_names() {
        let mut song = parse("[Verse]\n[Am]Let it [F]be\n[Intro]\nAm  F  C  G");
        transpose_song(&mut song, 2);
        let tex = render(&song, false).unwrap();
        assert!(tex.contains("\\[Bm]Let it \\[G]be"), "{tex}");
        assert!(tex.contains("\\[Bm] \\[G] \\[D] \\[A]"), "{tex}");
        assert!(!tex.contains("\\[Am]"), "{tex}");
    }

    #[test]
    fn counts_are_the_sum_over_all_lines() {
        let mut song = parse("[Verse]\n[Am]a [Am]b\nAm F\n\n[Chorus]\n[C]c");
        assert_eq!(transpose_song(&mut song, 1), 5);
    }
}
