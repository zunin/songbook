//! Literal ASCII notation; no fret, tuning, or musical interpretation.
use crate::parser::song::{LineKind, Song};
use anyhow::{bail, Result};

pub const UNCHANGED_WARNING: &str =
    "Chord transposition is active; ASCII tabs are unchanged and may no longer match.";
/// Provisional export limit at 10pt cmtt10. Physical fit, alignment and page
/// breaks still require native/browser PDF and owner visual verification.
pub const MAX_COLUMNS: usize = 56;
pub(crate) const LAYOUT_MARKER: &str =
    "% songbook: literal ASCII tablature; one song column required\n";

/// Source-row role. String and annotation ordinals are independent, so an
/// annotation never consumes one of the original six string-row numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabRowRole {
    String,
    Annotation,
}

/// Immutable source identity, copied to every descendant of a split row.
/// Source lines and original row ordinals are one-based; IDs are opaque and
/// local to one parsed/reviewed song, not persistent identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabProvenance {
    origin: usize,
    instance: usize,
    source_line: usize,
    role: TabRowRole,
    original_row: usize,
    /// Original string rows in the source block, always excluding annotations.
    original_rows: usize,
}

impl TabProvenance {
    pub fn origin(&self) -> usize {
        self.origin
    }
    pub fn instance(&self) -> usize {
        self.instance
    }
    pub fn source_line(&self) -> usize {
        self.source_line
    }
    pub fn role(&self) -> TabRowRole {
        self.role
    }
    pub fn original_row(&self) -> usize {
        self.original_row
    }
    pub fn original_rows(&self) -> usize {
        self.original_rows
    }

    pub(crate) fn in_instance(self, instance: usize) -> Self {
        Self { instance, ..self }
    }

    pub(crate) fn annotation(self, source_line: usize, original_row: usize) -> Self {
        Self {
            source_line,
            role: TabRowRole::Annotation,
            original_row,
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabGroup {
    pub section: usize,
    /// First annotation row, equal to `start` when there are no annotations.
    pub annotation_start: usize,
    /// First retained string row. An annotation-only group has `start == end`.
    pub start: usize,
    /// Exclusive end of retained string rows; annotations are not counted.
    pub end: usize,
    /// First source line of the original block, unchanged by review/splitting.
    pub source_line: usize,
    pub original_rows: usize,
    pub origin: usize,
    pub instance: usize,
}

/// Numeric raw-column bounds for the current complete block. Review indices
/// are zero-based, `end` exclusive; source_line is one-based. A cut includes
/// indentation, label and first pipe, not just notation after the pipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSplitBounds {
    /// Start of the whole replacement range, including retained annotations.
    pub start: usize,
    pub end: usize,
    /// Number of annotation rows immediately before the six string rows.
    pub annotation_count: usize,
    pub source_line: usize,
    pub origin: usize,
    pub instance: usize,
    pub prefix_columns: usize,
    pub max_row_columns: usize,
    pub min_after_column: usize,
    pub max_after_column: usize,
}

/// One original row's two literal fragments, plus explicit added-prefix/padding
/// disclosure. String-row `prefix` includes indentation, the label and first
/// pipe. Annotation-row `prefix` is deliberately empty: right annotations use
/// alignment spaces rather than repeating heading/fingering text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSplitRow {
    pub source_line: usize,
    pub original_row: usize,
    pub original_columns: usize,
    pub prefix: String,
    pub padding_columns: usize,
    pub left: String,
    pub left_columns: usize,
    pub right: String,
    pub right_columns: usize,
    /// No source suffix remains: right is only the repeated prefix and padding.
    pub right_label_only: bool,
}

/// Read-only preview: apply places the complete annotated left child before
/// the complete annotated right child, with six string rows in each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSplitPreview {
    pub bounds: TabSplitBounds,
    pub after_column: usize,
    /// Associated rows placed above the six string rows in each child. Their
    /// right fragments begin with alignment spaces, never repeated note text.
    pub annotations: Vec<TabSplitRow>,
    pub rows: [TabSplitRow; 6],
    pub warnings: Vec<String>,
}

pub const SPLIT_NOTATION_WARNING: &str =
    "An arbitrary cut may divide a fret number or technique; no musical alignment is guaranteed.";
pub const SPLIT_PREFIX_WARNING: &str =
    "Right rows repeat each exact label/first-pipe prefix and add the disclosed alignment spaces; no tails, dashes or notes are padded or invented.";
pub const SPLIT_RIGHT_WARNING: &str = "Right block still exceeds56; split again";
pub const SPLIT_LABEL_ONLY_WARNING: &str =
    "Some right rows contain only their repeated label/first-pipe prefix and disclosed padding; no missing notes are invented.";
pub const SPLIT_ANNOTATION_WARNING: &str =
    "Tab annotations appear above both children; right annotation fragments retain only their suffix after the shared cut, preceded by disclosed alignment spaces.";

/// Assign original runs before review: blank/non-tab lines are boundaries.
/// Adjacent complete six-row blocks are split every six; other run lengths
/// remain explicitly incomplete/unsupported, never silently reinterpreted.
/// Recognized annotations share their following block's identity but use a
/// separate original-row ordinal sequence and never affect its string count.
pub(crate) fn source_provenance(kinds: &[LineKind]) -> Vec<Option<TabProvenance>> {
    let mut out = vec![None; kinds.len()];
    let mut start = 0;
    while start < kinds.len() {
        if matches!(kinds[start], LineKind::TabAnnotation(_)) {
            let annotation_start = start;
            while start < kinds.len() && matches!(kinds[start], LineKind::TabAnnotation(_)) {
                start += 1;
            }
            let string_start = start;
            while start < kinds.len() && matches!(kinds[start], LineKind::Fretboard(_)) {
                start += 1;
            }
            // The classifier only creates annotations anchored to exactly six
            // strings. Keep this invariant defensive rather than laundering an
            // unsupported run through provenance if that classifier changes.
            if start - string_start != 6 {
                continue;
            }
            for (row, slot) in out
                .iter_mut()
                .skip(annotation_start)
                .take(string_start - annotation_start)
                .enumerate()
            {
                *slot = Some(TabProvenance {
                    origin: annotation_start,
                    instance: annotation_start,
                    source_line: annotation_start + row + 1,
                    role: TabRowRole::Annotation,
                    original_row: row + 1,
                    original_rows: 6,
                });
            }
            for (row, slot) in out.iter_mut().skip(string_start).take(6).enumerate() {
                *slot = Some(TabProvenance {
                    origin: annotation_start,
                    instance: annotation_start,
                    source_line: string_start + row + 1,
                    role: TabRowRole::String,
                    original_row: row + 1,
                    original_rows: 6,
                });
            }
            continue;
        }
        if !matches!(kinds[start], LineKind::Fretboard(_)) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < kinds.len() && matches!(kinds[end], LineKind::Fretboard(_)) {
            end += 1;
        }
        let chunk = if (end - start).is_multiple_of(6) {
            6
        } else {
            end - start
        };
        for block in (start..end).step_by(chunk) {
            for (row, slot) in out.iter_mut().skip(block).take(chunk).enumerate() {
                *slot = Some(TabProvenance {
                    origin: block,
                    instance: block,
                    source_line: block + row + 1,
                    role: TabRowRole::String,
                    original_row: row + 1,
                    original_rows: chunk,
                });
            }
        }
        start = end;
    }
    out
}

/// Attach locations after chord merging using explicit, stable row provenance
/// in retained song order. Never derive origins from mutable review indices or
/// re-chunk split descendants. Non-tab output/section boundaries break groups.
/// `start..end` always counts strings only; the adjacent annotation prefix is
/// represented separately, including when every string has been discarded.
pub(crate) fn attach(song: &mut Song, retained: &[Option<TabProvenance>]) {
    let mut retained = retained.iter();
    let mut previous_identity = None;
    song.tab_groups.clear();
    for (section, value) in song.sections.iter().enumerate() {
        for (line_index, line) in value.lines.iter().enumerate() {
            let annotation = matches!(line, LineKind::TabAnnotation(_));
            if !annotation && !matches!(line, LineKind::Fretboard(_)) {
                previous_identity = None;
                continue;
            }
            let Some(Some(source)) = retained.next() else {
                previous_identity = None;
                continue;
            };
            debug_assert_eq!(
                annotation,
                source.role == TabRowRole::Annotation,
                "source role and rebuilt line kind must agree"
            );
            let identity = (source.origin, source.instance);
            if previous_identity == Some(identity) {
                if let Some(last) = song.tab_groups.last_mut() {
                    if last.section == section && last.end == line_index {
                        if !annotation {
                            last.end += 1;
                        } else {
                            // Annotation-only groups keep their empty string
                            // range immediately after the annotation prefix.
                            last.start = line_index + 1;
                            last.end = line_index + 1;
                        }
                        continue;
                    }
                }
            }
            song.tab_groups.push(TabGroup {
                section,
                annotation_start: line_index,
                start: line_index + usize::from(annotation),
                end: line_index + 1,
                source_line: source.origin + 1,
                original_rows: source.original_rows,
                origin: source.origin,
                instance: source.instance,
            });
            previous_identity = Some(identity);
        }
    }
}

pub fn has_tablature(song: &Song) -> bool {
    song.sections.iter().any(|s| {
        s.lines
            .iter()
            .any(|l| matches!(l, LineKind::Fretboard(_) | LineKind::TabAnnotation(_)))
    })
}

pub fn warnings(song: &Song) -> Vec<String> {
    let mut warnings = Vec::new();
    if song.tablature_transpose != 0 && has_tablature(song) {
        warnings.push(UNCHANGED_WARNING.into());
    }
    for (index, group) in song.tab_groups.iter().enumerate() {
        let count = group.end - group.start;
        if count == 0 && group.annotation_start < group.start {
            warnings.push(format!("Retained tab annotation group {} at source line {} has no retained string rows. Associate it with a following complete six-string block or discard the annotation; no missing strings are invented.", index + 1, group.source_line));
        } else if group.original_rows == 6 && count != 6 {
            warnings.push(format!("Partial six-string tab block {} at source line {}: {count} retained rows ({} originally). Check Keep/Discard/Edit; no missing strings are invented.", index + 1, group.source_line, group.original_rows));
        }
    }
    if let Err(error) = validate(song) {
        warnings.push(error.to_string());
    }
    warnings
}

pub fn validate(song: &Song) -> Result<()> {
    for (section_index, section) in song.sections.iter().enumerate() {
        for (line_index, line) in section.lines.iter().enumerate() {
            let (text, annotation) = match line {
                LineKind::Fretboard(text) => (text, false),
                LineKind::TabAnnotation(text) => (text, true),
                _ => continue,
            };
            let group = song.tab_groups.iter().position(|g| {
                g.section == section_index && g.annotation_start <= line_index && g.end > line_index
            });
            let Some(group_index) = group else {
                bail!("Tab provenance missing in song '{}'; reparse or review the source before export.", song.title.as_deref().unwrap_or("Untitled"));
            };
            let group = &song.tab_groups[group_index];
            let location = format!(
                "song '{}', tab block {}, source line {}",
                song.title.as_deref().unwrap_or("Untitled"),
                group_index + 1,
                group.source_line
            );
            if group.original_rows != 6 {
                bail!("Cannot export {location}: {} rows are unsupported; support is six-string guitar only. Separate complete six-row blocks with blank lines, or discard/reclassify unsupported material.", group.original_rows);
            }
            if !text.bytes().all(|b| (32..=126).contains(&b)) {
                bail!("Cannot export {location}: only printable ASCII is supported (no tabs/control or Unicode characters). Edit whitespace to spaces or discard/reclassify the block.");
            }
            if text.len() > MAX_COLUMNS {
                let role = if annotation {
                    format!("annotation row {}", line_index - group.annotation_start + 1)
                } else {
                    format!("string row {}", line_index - group.start + 1)
                };
                bail!("Cannot export {location}: {role} has {} columns, maximum {MAX_COLUMNS} at readable 10pt. Shorten indentation or split the notation into aligned six-string blocks; annotations and strings are never wrapped or shrunk.", text.len());
            }
        }
    }
    Ok(())
}

pub(crate) fn render_group(song: &Song, group: &TabGroup) -> String {
    let mut out = String::new();
    let string_count = group.end - group.start;
    if group.original_rows == 6 && string_count != 6 && string_count != 0 {
        out.push_str(&format!(
            "\\textnote{{Partial six-string tab block: {string_count} retained rows; source line {}.}}\n",
            group.source_line
        ));
    }
    out.push_str(
        "\\textnote{\\vbox{\\fontencoding{OT1}\\fontfamily{cmtt}\\fontsize{10}{12}\\selectfont\n\\setbox0=\\hbox{\\char48\\relax}\\dimen0=\\wd0\\relax\n",
    );
    for line in &song.sections[group.section].lines[group.annotation_start..group.end] {
        if let LineKind::Fretboard(text) | LineKind::TabAnnotation(text) = line {
            out.push_str("\\hbox{");
            // Only fixed TeX and numeric character tokens: raw source cannot
            // execute commands, close delimiters, or turn into a comment.
            // OT1 cmtt10's slot 32 is a visible-space symbol, and slots 39/96
            // are curly quotes. Use a measured font cell for actual spaces and
            // the font's straight-quote slots (native-verified by print review).
            for byte in text.bytes() {
                match byte {
                    b' ' => out.push_str("\\hskip\\dimen0\\relax "),
                    b'\'' => out.push_str("\\char13\\relax "),
                    b'`' => out.push_str("\\char18\\relax "),
                    _ => out.push_str(&format!("\\char{byte}\\relax ")),
                }
            }
            out.push_str("}\n");
        }
    }
    out.push_str("}}\n");
    out
}
