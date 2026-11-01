use anyhow::{bail, Context, Result};
use songbook_core::audit::{apply_noninteractive_defaults, audit_summary_line};
use songbook_core::latex::render::render;
use songbook_core::parser::classify::parse;
use std::path::{Path, PathBuf};

pub struct BatchSummary {
    pub converted: usize,
    pub skipped: usize,
}

/// Converts every `.txt`/`.crd` file found under `input` (a directory) to a
/// `.tex` file beside each input, or into `out_dir` when given. Per-file
/// errors are reported to stderr and skipped, never fatal; the caller prints
/// the summary and decides the exit status. Batch mode is inherently
/// non-interactive: each converted file gets a one-line audit summary, like
/// `--yes` (which prints the full report).
pub fn run(
    input: &Path,
    out_dir: Option<&Path>,
    recursive: bool,
    standalone: bool,
) -> Result<BatchSummary> {
    let files = discover(input, recursive)?;
    if files.is_empty() {
        println!("No .txt or .crd files found in {}.", input.display());
        return Ok(BatchSummary {
            converted: 0,
            skipped: 0,
        });
    }

    if let Some(out_dir) = out_dir {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("creating output directory {}", out_dir.display()))?;
    }

    let mut converted = 0;
    let mut skipped = 0;
    for file in &files {
        let Some(out_path) = output_path_for(file, out_dir) else {
            eprintln!(
                "Skipped {}: cannot determine output file name",
                file.display()
            );
            skipped += 1;
            continue;
        };
        match convert_file(file, &out_path, standalone) {
            Ok(summary) => {
                println!("Wrote {} — {summary}", out_path.display());
                converted += 1;
            }
            Err(error) => {
                eprintln!("Skipped {}: {error:#}", file.display());
                skipped += 1;
            }
        }
    }

    Ok(BatchSummary { converted, skipped })
}

/// Reads, parses, renders, and writes one file, returning its one-line audit
/// summary.
fn convert_file(file: &Path, out_path: &Path, standalone: bool) -> Result<String> {
    let raw =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    if raw.trim().is_empty() {
        bail!("file is empty");
    }
    let song = parse(&raw);
    if song.sections.is_empty() {
        bail!("no song sections found (input may be only noise or metadata)");
    }
    let summary = audit_summary_line(&song, &raw);
    let mut song = song;
    apply_noninteractive_defaults(&mut song);
    let tex = render(&song, standalone).with_context(|| format!("rendering {}", file.display()))?;
    std::fs::write(out_path, tex).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(summary)
}

/// Deterministic, sorted list of tab files under `dir`. Non-recursive by
/// default: only files directly inside `dir`; `recursive` walks subdirectories
/// (symlinks are never followed).
pub fn discover(dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    discover_into(dir, recursive, &mut files)?;
    files.sort();
    Ok(files)
}

fn discover_into(dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("listing directory {}", dir.display()))?;
    let mut subdirs = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("reading a directory entry in {}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspecting {}", path.display()))?;
        if file_type.is_dir() {
            if recursive {
                subdirs.push(path);
            }
        } else if path.is_file() && is_tab_file(&path) {
            files.push(path);
        }
    }
    subdirs.sort();
    for subdir in subdirs {
        discover_into(&subdir, recursive, files)?;
    }
    Ok(())
}

fn is_tab_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension)
            if extension.eq_ignore_ascii_case("txt") || extension.eq_ignore_ascii_case("crd")
    )
}

fn output_path_for(file: &Path, out_dir: Option<&Path>) -> Option<PathBuf> {
    let stem = file.file_stem()?;
    Some(match out_dir {
        Some(dir) => dir.join(Path::new(stem).with_extension("tex")),
        None => file.with_extension("tex"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const GOOD: &str = "Song: Good Song\nArtist: Someone\n\n[Verse]\n[Am]Hello there\n";

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("songbook-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discovers_txt_and_crd_files_non_recursively() {
        let dir = tempdir("discover");
        fs::write(dir.join("a.txt"), GOOD).unwrap();
        fs::write(dir.join("b.crd"), GOOD).unwrap();
        fs::write(dir.join("c.md"), "nope").unwrap();
        fs::write(dir.join("E.TXT"), GOOD).unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub").join("d.txt"), GOOD).unwrap();

        let flat = discover(&dir, false).unwrap();
        assert_eq!(
            flat,
            vec![dir.join("E.TXT"), dir.join("a.txt"), dir.join("b.crd")]
        );

        let recursive = discover(&dir, true).unwrap();
        assert_eq!(
            recursive,
            vec![
                dir.join("E.TXT"),
                dir.join("a.txt"),
                dir.join("b.crd"),
                dir.join("sub").join("d.txt")
            ]
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn batch_overwrites_existing_tex_and_skips_bad_files() {
        let dir = tempdir("batch");
        fs::write(dir.join("good.txt"), GOOD).unwrap();
        fs::write(dir.join("empty.txt"), "").unwrap();
        fs::write(dir.join("noise.txt"), "Tabbed by: x\n").unwrap();
        fs::write(dir.join("good.tex"), "stale content").unwrap();

        let summary = run(&dir, None, false, false).unwrap();
        assert_eq!(summary.converted, 1);
        assert_eq!(summary.skipped, 2);

        let tex = fs::read_to_string(dir.join("good.tex")).unwrap();
        assert!(tex.contains("\\beginsong{Good Song}"), "{tex}");
        assert!(!tex.contains("stale content"));
        assert!(!dir.join("empty.tex").exists());
        assert!(!dir.join("noise.tex").exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn batch_writes_into_out_dir() {
        let dir = tempdir("outdir");
        fs::write(dir.join("good.txt"), GOOD).unwrap();
        let out = dir.join("out");

        let summary = run(&dir, Some(&out), false, true).unwrap();
        assert_eq!(summary.converted, 1);
        assert_eq!(summary.skipped, 0);

        let tex = fs::read_to_string(out.join("good.tex")).unwrap();
        assert!(tex.starts_with("\\documentclass"), "{tex}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn empty_directory_is_a_clean_no_op() {
        let dir = tempdir("emptydir");
        let summary = run(&dir, None, false, false).unwrap();
        assert_eq!(summary.converted, 0);
        assert_eq!(summary.skipped, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recursive_batch_converts_nested_files() {
        let dir = tempdir("recursive");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("top.txt"), GOOD).unwrap();
        fs::write(dir.join("sub").join("nested.txt"), GOOD).unwrap();

        let summary = run(&dir, None, true, false).unwrap();
        assert_eq!(summary.converted, 2);
        assert!(dir.join("top.tex").exists());
        assert!(dir.join("sub").join("nested.tex").exists());

        fs::remove_dir_all(&dir).unwrap();
    }
}
