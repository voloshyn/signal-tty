use crate::app::App;
use crate::storage::MessageContent;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::DarkGray };

    let title = app
        .selected_conversation()
        .map(|c| format!(" {} ", c.conversation.display_name()))
        .unwrap_or_else(|| " Messages ".to_string());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let Some(conv_view) = app.selected_conversation() else {
        let empty = Paragraph::new("No conversation selected")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner_area);
        return;
    };

    let Some(ref messages) = conv_view.messages else {
        let loading = Paragraph::new("Loading...")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(loading, inner_area);
        return;
    };

    if messages.is_empty() {
        let empty = Paragraph::new("No messages yet")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner_area);
        return;
    }

    let visible_height = inner_area.height as usize;
    let total_messages = messages.len();
    
    // Calculate visible range based on scroll offset
    let start = conv_view.scroll_offset.saturating_sub(visible_height.saturating_sub(1));
    let end = (start + visible_height).min(total_messages);

    let lines: Vec<Line> = messages[start..end]
        .iter()
        .map(|msg| {
            let sender = if msg.is_outgoing {
                "You"
            } else {
                msg.sender_name.as_deref().unwrap_or("Unknown")
            };

            let text = match &msg.content {
                MessageContent::Text { body } => body.clone(),
                MessageContent::Attachment { attachments } => {
                    let name = attachments.first()
                        .and_then(|a| a.filename.clone())
                        .unwrap_or_else(|| "file".to_string());
                    format!("[Attachment: {}]", name)
                }
                MessageContent::Sticker { pack_id, sticker_id } => {
                    format!("[Sticker: {}#{}]", pack_id, sticker_id)
                }
                MessageContent::RemoteDeleted => "[Message deleted]".to_string(),
            };

            let timestamp = format_timestamp(msg.timestamp);

            let sender_style = if msg.is_outgoing {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            };

            Line::from(vec![
                Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{}: ", sender), sender_style),
                Span::raw(text),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, inner_area);

    // Scroll indicator
    if total_messages > visible_height {
        let scroll_pct = (conv_view.scroll_offset as f64 / total_messages as f64 * 100.0) as u16;
        let indicator = format!(" {}% ", scroll_pct.min(100));
        let indicator_area = Rect {
            x: area.x + area.width - indicator.len() as u16 - 2,
            y: area.y,
            width: indicator.len() as u16,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(indicator).style(Style::default().fg(Color::DarkGray)),
            indicator_area,
        );
    }
}

fn format_timestamp(timestamp: i64) -> String {
    use chrono::{Local, TimeZone};
    
    if let Some(dt) = Local.timestamp_millis_opt(timestamp).single() {
        let now = Local::now();
        if dt.date_naive() == now.date_naive() {
            dt.format("%H:%M").to_string()
        } else {
            dt.format("%m/%d %H:%M").to_string()
        }
    } else {
        "??:??".to_string()
    }
}
