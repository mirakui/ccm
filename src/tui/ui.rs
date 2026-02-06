use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use super::app::App;

/// Wrap text to fit within the given display width (in terminal columns).
/// Handles multi-byte UTF-8 and wide characters (CJK, emoji) correctly.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut result = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }
        let mut remaining = line;
        while !remaining.is_empty() {
            if UnicodeWidthStr::width(remaining) <= width {
                result.push(remaining.to_string());
                break;
            }
            // Find byte offset where display width exceeds `width`
            let mut col = 0;
            let mut byte_end = remaining.len();
            for (idx, ch) in remaining.char_indices() {
                let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                if col + ch_width > width {
                    byte_end = idx;
                    break;
                }
                col += ch_width;
            }
            // Find last space within the safe byte range for word-breaking
            let mut break_at = if byte_end < remaining.len() {
                remaining[..byte_end]
                    .rfind(' ')
                    .map(|pos| pos + 1)
                    .unwrap_or(byte_end)
            } else {
                byte_end
            };
            // Ensure forward progress: if break_at is 0 (first char wider than width),
            // advance past at least one character to avoid infinite loop
            if break_at == 0 {
                break_at = remaining
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| i)
                    .unwrap_or(remaining.len());
            }
            result.push(remaining[..break_at].trim_end().to_string());
            remaining = &remaining[break_at..];
        }
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

/// Render a title inside a Unicode box, appending Lines to the given vec.
fn render_title_box(lines: &mut Vec<Line>, title: &str, indent: usize, box_width: usize) {
    let indent_str: String = " ".repeat(indent);
    let inner_width = box_width.saturating_sub(4); // "│ " + " │"
    if inner_width == 0 {
        return;
    }

    let wrapped = wrap_text(title, inner_width);
    let style = Style::default().fg(Color::DarkGray);

    // Top border: ┌────┐
    let top = format!(
        "{indent_str}┌{}┐",
        "─".repeat(box_width.saturating_sub(2))
    );
    lines.push(Line::from(Span::styled(top, style)));

    // Content lines: │ text │
    for text_line in &wrapped {
        let display_width = UnicodeWidthStr::width(text_line.as_str());
        let padding = inner_width.saturating_sub(display_width);
        let content = format!(
            "{indent_str}│ {}{} │",
            text_line,
            " ".repeat(padding)
        );
        lines.push(Line::from(Span::styled(content, style)));
    }

    // Bottom border: └────┘
    let bottom = format!(
        "{indent_str}└{}┘",
        "─".repeat(box_width.saturating_sub(2))
    );
    lines.push(Line::from(Span::styled(bottom, style)));
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Min(1),    // session list
        Constraint::Length(1), // help line
    ])
    .split(area);

    // Session list
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        " CCM Sessions",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        " ────────────",
        Style::default().fg(Color::DarkGray),
    )));

    let indent: usize = 3;
    let box_width = (area.width as usize).saturating_sub(indent);

    if app.sessions.is_empty() {
        lines.push(Line::from(Span::styled(
            " (no sessions)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, session) in app.sessions.iter().enumerate() {
            let is_selected = i == app.selected_index;
            let is_active = app.active_session.as_deref() == Some(&session.name);

            let prefix = if is_selected { " > " } else { "   " };
            let suffix = if is_active { "  ●" } else { "" };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(Span::styled(
                format!("{prefix}{}{suffix}", session.name),
                style,
            )));

            // Render pane title box if available
            if let Some(title) = app.pane_titles.get(&session.claude_pane_id) {
                if !title.is_empty() && box_width > 4 {
                    render_title_box(&mut lines, title, indent, box_width);
                }
            }
        }
    }

    // Confirm delete overlay
    if let Some(ref name) = app.confirm_delete {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" Delete '{name}'? [y/n]"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    }

    // Status message
    if let Some(ref msg) = app.status_message {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(Color::Red),
        )));
    }

    let session_widget = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
    f.render_widget(session_widget, chunks[0]);

    // Help line
    let help = Line::from(vec![
        Span::styled(" j/k", Style::default().fg(Color::Yellow)),
        Span::styled(" nav ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(" switch ", Style::default().fg(Color::DarkGray)),
        Span::styled("d", Style::default().fg(Color::Yellow)),
        Span::styled(" del ", Style::default().fg(Color::DarkGray)),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::styled(" quit", Style::default().fg(Color::DarkGray)),
    ]);
    let help_widget = Paragraph::new(help);
    f.render_widget(help_widget, chunks[1]);
}
