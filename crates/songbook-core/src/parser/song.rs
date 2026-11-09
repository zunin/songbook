#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    Intro,
    Verse,
    PreChorus,
    Chorus,
    Bridge,
    Solo,
    Interlude,
    Instrumental,
    Outro,
    Other(String),
}

/// Classification of a single line of raw tab text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineKind {
    /// Lyric line, possibly containing inline `[Chord]` markers.
    Lyric(String),
    /// A line consisting only of chord names (e.g. `Am  F  G  2x`).
    ChordOnly(String),
    SectionHeader(SectionKind, String),
    /// Fretboard tablature (e|---3---| etc.).
    Fretboard(String),
    /// Literal chord/fingering annotation attached to an adjacent tab block.
    /// Kept separate from the six string rows so alignment and source counts
    /// survive review, splitting and export.
    TabAnnotation(String),
    /// Website noise, credits, ads, tuning notes, etc.
    Noise(String),
    Blank,
    /// Classifier was unsure; queued for TUI triage.
    Ambiguous(String),
}

#[derive(Debug, Clone)]
pub struct Section {
    pub kind: SectionKind,
    pub lines: Vec<LineKind>,
}

#[derive(Debug, Clone, Default)]
pub struct Song {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub key: Option<String>,
    pub capo: Option<String>,
    #[allow(dead_code)]
    pub tuning: Option<String>,
    pub sections: Vec<Section>,
    /// Retained tab locations and original source grouping (ADR0021).
    pub tab_groups: Vec<crate::tablature::TabGroup>,
    /// Net requested chord offset while tablature is retained; payload is never
    /// changed. Shared state for conditional unchanged-tab warnings.
    pub tablature_transpose: i32,
}
