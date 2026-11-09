//! Terminal-local plan/preview/apply flow. Core alone generates literal rows.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use songbook_core::{
    tablature::{TabSplitBounds, MAX_COLUMNS},
    triage::{TabSplitPlan, TriageState},
};

#[cfg(test)]
mod tests;

const PROMPT_TITLE: &str = "Split tab block after column";
const CUT_WARNING: &str = "Warning: no musically safe cut is guaranteed.";
const PREVIEW_KEYS: &str =
    "Enter Apply · Esc cancel · Up/Down scroll · PgUp/PgDn page · Left/Right pan · Ctrl+C quit";

#[derive(Debug)]
enum Stage {
    Prompt {
        bounds: TabSplitBounds,
        input: String,
        error: Option<String>,
    },
    Preview(Box<TabSplitPlan>),
    Notice {
        title: &'static str,
        message: String,
    },
}

#[derive(Debug)]
pub(super) struct SplitModal {
    stage: Stage,
    scroll: usize,
    pan: u16,
}

impl SplitModal {
    pub(super) fn open(state: &TriageState) -> Self {
        let stage = match state.tab_split_bounds() {
            Ok(bounds) => Stage::Prompt {
                input: bounds.max_after_column.to_string(),
                bounds,
                error: None,
            },
            Err(error) => Stage::Notice {
                title: "Split tab block unavailable",
                message: format!(
                    "{error} Select a complete six-string block; use e Edit or u Undo to restore rows, or d Discard to omit unsupported material."
                ),
            },
        };
        Self {
            stage,
            scroll: 0,
            pan: 0,
        }
    }

    /// Every key is consumed while open. Returns true to dismiss. Escape is
    /// handled here, before quit confirmation, and never reaches core. Only
    /// Apply mutates review state; pan/scroll do not invalidate the stored plan.
    pub(super) fn handle_key(
        &mut self,
        key: KeyEvent,
        state: &mut TriageState,
        area: Rect,
    ) -> bool {
        if key.code == KeyCode::Esc {
            return true;
        }
        let unmodified = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        if let Stage::Prompt { input, error, .. } = &mut self.stage {
            match key.code {
                KeyCode::Char(ch) if unmodified && ch.is_ascii_digit() => {
                    input.push(ch);
                    *error = None;
                }
                KeyCode::Backspace => {
                    input.pop();
                    *error = None;
                }
                KeyCode::Enter => {
                    let result = input
                        .parse::<usize>()
                        .map_err(|_| {
                            anyhow::anyhow!(
                                "Enter a whole raw column number in the displayed range."
                            )
                        })
                        .and_then(|column| state.plan_tab_split(column));
                    match result {
                        Ok(plan) => {
                            self.stage = Stage::Preview(Box::new(plan));
                            self.scroll = 0;
                            self.pan = 0;
                        }
                        Err(problem) => *error = Some(problem.to_string()),
                    }
                }
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::Enter {
            // Move (not clone) the opaque plan so it can be applied only once.
            let stage = std::mem::replace(
                &mut self.stage,
                Stage::Notice {
                    title: "Split tab block not applied",
                    message: String::new(),
                },
            );
            if let Stage::Preview(plan) = stage {
                if let Err(error) = state.apply_tab_split(*plan) {
                    self.stage = Stage::Notice {
                        title: "Split tab block not applied",
                        message: format!(
                            "{error} Cancel, then press F2 to preview the current block again."
                        ),
                    };
                    self.scroll = 0;
                    self.pan = 0;
                    return false;
                }
            }
            return true;
        }
        let (_, body, _) = self.areas(area);
        let max_scroll = self
            .document(body.width)
            .len()
            .saturating_sub(body.height as usize);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if unmodified => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') if unmodified => {
                self.scroll = self.scroll.saturating_add(1).min(max_scroll);
            }
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10).min(max_scroll),
            KeyCode::Left => self.pan = self.pan.saturating_sub(8),
            KeyCode::Right => {
                let max_pan = match &self.stage {
                    Stage::Preview(plan) => plan
                        .preview()
                        .rows
                        .iter()
                        .chain(&plan.preview().annotations)
                        .map(|row| row.left_columns.max(row.right_columns))
                        .max()
                        .unwrap_or(0)
                        .saturating_sub(body.width as usize),
                    _ => 0,
                };
                self.pan = self
                    .pan
                    .saturating_add(8)
                    .min(max_pan.min(u16::MAX as usize) as u16);
            }
            _ => {}
        }
        false
    }

    fn header(&self) -> (&str, String) {
        match &self.stage {
            Stage::Prompt { .. } => (
                PROMPT_TITLE,
                "Numeric entry; Enter previews, not applies.".into(),
            ),
            Stage::Preview(plan) => (
                "Split tab block preview",
                format!(
                    "After raw column {}. {CUT_WARNING}",
                    plan.preview().after_column
                ),
            ),
            Stage::Notice { title, .. } => {
                (title, "No split applied; review state is unchanged.".into())
            }
        }
    }

    fn keys(&self) -> &str {
        match &self.stage {
            Stage::Prompt { .. } => {
                "Digits append · Backspace delete · Enter Preview · Esc cancel · Ctrl+C quit"
            }
            Stage::Preview(_) => PREVIEW_KEYS,
            Stage::Notice { .. } => {
                "Enter/Esc close · Up/Down scroll · PgUp/PgDn page · Ctrl+C quit"
            }
        }
    }

    fn areas(&self, area: Rect) -> (Rect, Rect, Rect) {
        let width = area.width.saturating_sub(2);
        let header_height = (wrap_text(&self.header().1, width).len() as u16 + 2)
            .min(area.height.saturating_sub(2));
        let footer_height = (wrap_text(self.keys(), width).len() as u16 + 2)
            .min(area.height.saturating_sub(header_height));
        let header = Rect::new(area.x, area.y, area.width, header_height);
        let body = Rect::new(
            area.x + u16::from(area.width > 0),
            area.y + header_height,
            width,
            area.height.saturating_sub(header_height + footer_height),
        );
        let footer = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(footer_height),
            area.width,
            footer_height,
        );
        (header, body, footer)
    }

    /// Prose wraps independently of raw notation. Horizontal pan affects only
    /// literal rows, never warnings, controls or the row-length disclosures.
    fn document(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        match &self.stage {
            Stage::Prompt {
                bounds,
                input,
                error,
            } => {
                prose(&mut lines, &format!("{PROMPT_TITLE}: {input}_"), width);
                prose(&mut lines, &format!(
                    "Allowed raw columns: {} through {} inclusive. Suggested default: {} (not applied automatically).",
                    bounds.min_after_column, bounds.max_after_column, bounds.max_after_column
                ), width);
                prose(&mut lines, "Count from column 1: indentation, string label and first pipe count. This is one shared raw column count for annotations and all six strings, not a musical position.", width);
                prose(
                    &mut lines,
                    &format!(
                        "Source block line {}: {} annotation(s) above six strings.",
                        bounds.source_line, bounds.annotation_count
                    ),
                    width,
                );
                prose(
                    &mut lines,
                    &format!(
                        "Longest row: {} columns; longest prefix through first pipe: {} columns.",
                        bounds.max_row_columns, bounds.prefix_columns
                    ),
                    width,
                );
                prose(&mut lines, CUT_WARNING, width);
                if let Some(error) = error {
                    prose(&mut lines, &format!("Invalid column: {error}"), width);
                }
            }
            Stage::Preview(plan) => {
                let preview = plan.preview();
                for right in [false, true] {
                    prose(
                        &mut lines,
                        &format!(
                            "{} six rows: {} annotation(s) first{}",
                            if right { "RIGHT" } else { "LEFT" },
                            preview.annotations.len(),
                            if right { " (after LEFT)" } else { "" }
                        ),
                        width,
                    );
                    for row in &preview.annotations {
                        let columns = if right {
                            row.right_columns
                        } else {
                            row.left_columns
                        };
                        let detail = if right {
                            format!(
                                "prefix {} cols + padding {} spaces{}",
                                row.prefix.len(),
                                row.padding_columns,
                                if columns > MAX_COLUMNS {
                                    "; OVERFLOW: split again"
                                } else {
                                    ""
                                }
                            )
                        } else {
                            format!("original {} cols", row.original_columns)
                        };
                        prose(
                            &mut lines,
                            &format!(
                                "Annotation {} (source {}): {columns} cols; {detail}",
                                row.original_row, row.source_line
                            ),
                            width,
                        );
                        let literal = if right { &row.right } else { &row.left };
                        if right && row.right_label_only {
                            prose(&mut lines, "Empty annotation fragment: alignment spaces only (no source suffix).", width);
                        } else if literal.is_empty() {
                            prose(
                                &mut lines,
                                "Empty annotation fragment (no source content).",
                                width,
                            );
                        }
                        lines.push(Line::from(
                            literal.chars().skip(self.pan as usize).collect::<String>(),
                        ));
                    }
                    for row in &preview.rows {
                        let columns = if right {
                            row.right_columns
                        } else {
                            row.left_columns
                        };
                        let detail = if right {
                            format!(
                                "prefix {} cols + padding {} spaces{}{}",
                                row.prefix.len(),
                                row.padding_columns,
                                if row.right_label_only {
                                    "; label-only"
                                } else {
                                    ""
                                },
                                if columns > MAX_COLUMNS {
                                    "; OVERFLOW: split again"
                                } else {
                                    ""
                                }
                            )
                        } else {
                            format!("original {} cols", row.original_columns)
                        };
                        prose(
                            &mut lines,
                            &format!(
                                "String {} (source {}): {columns} cols; {detail}",
                                row.original_row, row.source_line
                            ),
                            width,
                        );
                        let literal = if right { &row.right } else { &row.left };
                        lines.push(Line::from(
                            literal.chars().skip(self.pan as usize).collect::<String>(),
                        ));
                    }
                }
                prose(
                    &mut lines,
                    "Warnings and additions (inspect before Apply):",
                    width,
                );
                for warning in &preview.warnings {
                    prose(&mut lines, warning, width);
                }
                if preview
                    .rows
                    .iter()
                    .chain(&preview.annotations)
                    .any(|row| row.right_columns > MAX_COLUMNS)
                {
                    prose(&mut lines, &format!("Remaining overflow: a right row exceeds {MAX_COLUMNS} columns. Apply is allowed; split the right block again before export."), width);
                }
                prose(&mut lines, "Enter Apply replaces the block with LEFT annotations/six strings then RIGHT annotations/six strings as one undo step. No changes have been made yet.", width);
            }
            Stage::Notice { message, .. } => prose(&mut lines, message, width),
        }
        lines
    }

    pub(super) fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let (header, body, footer) = self.areas(area);
        frame.render_widget(Clear, area);
        let (title, explanation) = self.header();
        frame.render_widget(
            Paragraph::new(wrap_text(&explanation, header.width.saturating_sub(2)).join("\n"))
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title(title)),
            header,
        );
        let lines = self.document(body.width);
        let max_scroll = lines.len().saturating_sub(body.height as usize);
        let scroll = self.scroll.min(max_scroll);
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll.min(u16::MAX as usize) as u16, 0)),
            body,
        );
        frame.render_widget(
            Paragraph::new(wrap_text(self.keys(), footer.width.saturating_sub(2)).join("\n"))
                .block(Block::default().borders(Borders::ALL).title(format!(
                    "Split keys · scroll {scroll}/{max_scroll} · pan {}",
                    self.pan
                ))),
            footer,
        );
    }
}

fn prose(lines: &mut Vec<Line<'static>>, text: &str, width: u16) {
    lines.extend(wrap_text(text, width).into_iter().map(|text| {
        Line::from(Span::styled(
            text,
            Style::default().add_modifier(Modifier::BOLD),
        ))
    }));
}

/// Only explanatory prose is wrapped; notation never passes through this.
fn wrap_text(text: &str, width: u16) -> Vec<String> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let length = line.chars().count();
        if length > 0 && length + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
