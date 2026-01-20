use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let has_attachments = !app.pending_attachments.is_empty();
    let title = if has_attachments {
        format!(" Message [{} file(s)] ", app.pending_attachments.len())
    } else {
        " Message ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let [attachment_area, input_area] = if has_attachments {
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner_area)
    } else {
        [Rect::default(), inner_area]
    };

    if has_attachments {
        let names: Vec<String> = app
            .pending_attachments
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        let attachment_text = format!("📎 {} (Ctrl+x to clear)", names.join(", "));
        let attachment_line = Paragraph::new(Span::styled(
            attachment_text,
            Style::default().fg(Color::Green),
        ));
        frame.render_widget(attachment_line, attachment_area);
    }

    let text = &app.input.text;
    let cursor = app.input.cursor;

    let (before, after) = text.split_at(cursor.min(text.len()));

    let line = if focused {
        let cursor_char = after.chars().next().unwrap_or(' ');
        let after_cursor = if after.is_empty() {
            ""
        } else {
            &after[cursor_char.len_utf8()..]
        };

        Line::from(vec![
            Span::raw(before),
            Span::styled(
                cursor_char.to_string(),
                Style::default().bg(Color::White).fg(Color::Black),
            ),
            Span::raw(after_cursor),
        ])
    } else {
        if text.is_empty() {
            Line::from(Span::styled(
                "Type a message...",
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            Line::from(text.as_str())
        }
    };

    let paragraph = Paragraph::new(line).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, input_area);
}
