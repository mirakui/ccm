use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::app::App;

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
