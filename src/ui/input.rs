use crate::app::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(" Message ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

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

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, inner_area);
}
