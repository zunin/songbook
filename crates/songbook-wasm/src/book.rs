//! Book ferry (ADR0019): hands triaged song parts to core's book composer.
//!
//! JavaScript never assembles LaTeX (P1/P2): `Session::book_parts` produces
//! each triaged song's `{ body, chords }` part, and [`compose_book`] forwards
//! the collected parts to `songbook_core::book::compose_book`, the one
//! composition implementation, returning the finished standalone document.

use crate::error::SessionError;
use songbook_core::book::{self, BookPart};

/// Composes pre-rendered book parts into one standalone LaTeX document,
/// delegating entirely to core's parts-based composer. The title index is
/// always included; `chord_index` adds the chord index. Empty titles and
/// authors count as absent, like the manifest's headers.
pub fn compose_book(
    title: Option<&str>,
    author: Option<&str>,
    parts: &[BookPart],
    chord_index: bool,
) -> Result<String, SessionError> {
    book::compose_book(title, author, parts, chord_index)
        .map_err(|error| SessionError::Compose(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    #[test]
    fn composes_parts_from_sessions_into_one_document() {
        let one = Session::new("Song: One\n[Verse]\n[Am]Hello\n")
            .unwrap()
            .book_parts()
            .unwrap();
        let two = Session::new("[Chorus]\n[F]Hey\n")
            .unwrap()
            .book_parts()
            .unwrap();

        let parts = [
            BookPart {
                body: one.body,
                chords: one.chords,
            },
            BookPart {
                body: two.body,
                chords: two.chords,
            },
        ];
        let tex = compose_book(Some("My Book"), None, &parts, true).unwrap();
        assert!(tex.starts_with("\\documentclass[12pt]{article}\n"), "{tex}");
        assert!(tex.contains("\\title{My Book}"), "{tex}");
        assert!(tex.contains("\\beginsong{One}"), "{tex}");
        assert!(tex.contains("\\indexentry[chordidx]{Am}%"), "{tex}");
        assert!(
            tex.contains("\\beginsong{Untitled}\n\\indexentry[chordidx]{F}%"),
            "{tex}"
        );
        assert!(
            tex.contains("\\end{songs}\n\\showindex{Index of Chords}{chordidx}\n\\end{document}\n"),
            "{tex}"
        );
    }

    #[test]
    fn empty_parts_and_no_metadata_compose_an_empty_book() {
        let tex = compose_book(None, None, &[], false).unwrap();
        assert!(
            tex.contains("\\begin{songs}{titleidx}\n\\end{songs}"),
            "{tex}"
        );
    }

    #[test]
    fn compose_errors_carry_the_compose_variant() {
        // The core composer cannot fail on plain parts, so the mapping is
        // defensive; assert the variant's display shape the JS side sees.
        let error = SessionError::Compose("boom".to_string());
        assert_eq!(error.to_string(), "book composition failed: boom");
    }
}
