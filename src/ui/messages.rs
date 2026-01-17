use crate::app::App;
use crate::image_cache::ImageCache;
use crate::storage::MessageContent;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ratatui_image::StatefulImage;

const IMAGE_HEIGHT: u16 = 8;

struct MessageRenderInfo {
    lines: Vec<Line<'static>>,
    image_key: Option<String>,
    image_height: u16,
}

fn calculate_message_height(msg: &crate::storage::Message) -> u16 {
    match &msg.content {
        MessageContent::Attachment { attachments } => {
            let mut h = 0u16;
            for att in attachments {
                h += 1;
                if ImageCache::is_image(att.content_type.as_deref()) && att.local_path.is_some() {
                    h += IMAGE_HEIGHT;
                }
            }
            h.max(1)
        }
        _ => 1,
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App, focused: bool, image_cache: &mut Option<ImageCache>) {
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
    app.messages_height = inner_area.height as usize;
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
    
    // scroll_offset is the index of the bottom-most visible message
    let scroll_offset = conv_view.scroll_offset.min(total_messages.saturating_sub(1));
    
    // Calculate which messages to show by working backwards from scroll_offset
    let mut total_height = 0usize;
    let mut start_idx = scroll_offset + 1;
    while start_idx > 0 && total_height < visible_height {
        start_idx -= 1;
        total_height += calculate_message_height(&messages[start_idx]) as usize;
    }
    
    // Calculate how many lines to skip at the start if first message is partially visible
    let skip_lines_at_start = total_height.saturating_sub(visible_height);
    
    let mut y_offset: i16 = -(skip_lines_at_start as i16);

    // Render messages from start_idx to scroll_offset (inclusive)
    for msg in messages.iter().skip(start_idx).take(scroll_offset + 1 - start_idx) {
        if y_offset >= inner_area.height as i16 {
            break;
        }

        let sender = if msg.is_outgoing {
            "You"
        } else {
            msg.sender_name.as_deref().unwrap_or("Unknown")
        };
        let timestamp = format_timestamp(msg.timestamp);
        let sender_style = if msg.is_outgoing {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        };

        match &msg.content {
            MessageContent::Attachment { attachments } => {
                for attachment in attachments {
                    if y_offset >= inner_area.height as i16 {
                        break;
                    }
                    
                    let is_image = ImageCache::is_image(attachment.content_type.as_deref());
                    let name = attachment.filename.as_deref()
                        .or(attachment.id.as_deref())
                        .unwrap_or("file");
                    
                    let header = Line::from(vec![
                        Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{}: ", sender), sender_style.clone()),
                        Span::styled(format!("📎 {}", name), Style::default().fg(Color::Yellow)),
                    ]);
                    
                    // Only render if y_offset is visible
                    if y_offset >= 0 {
                        let header_rect = Rect {
                            x: inner_area.x,
                            y: inner_area.y + y_offset as u16,
                            width: inner_area.width,
                            height: 1,
                        };
                        frame.render_widget(Paragraph::new(header), header_rect);
                    }
                    y_offset += 1;

                    if is_image {
                        if let (Some(cache), Some(local_path)) = (image_cache.as_mut(), &attachment.local_path) {
                            if let Some(protocol) = cache.get_image(local_path) {
                                // Calculate visible portion of image
                                let img_start = y_offset.max(0) as u16;
                                let img_end = (y_offset + IMAGE_HEIGHT as i16).min(inner_area.height as i16) as u16;
                                
                                if img_end > img_start {
                                    let image_rect = Rect {
                                        x: inner_area.x + 2,
                                        y: inner_area.y + img_start,
                                        width: inner_area.width.saturating_sub(4).min(60),
                                        height: img_end - img_start,
                                    };
                                    frame.render_stateful_widget(StatefulImage::new(), image_rect, protocol);
                                }
                            }
                        }
                        y_offset += IMAGE_HEIGHT as i16;
                    }
                }
            }
            _ => {
                let text = match &msg.content {
                    MessageContent::Text { body } => body.clone(),
                    MessageContent::Sticker { pack_id, sticker_id } => {
                        format!("[Sticker: {}#{}]", pack_id, sticker_id)
                    }
                    MessageContent::RemoteDeleted => "[Message deleted]".to_string(),
                    MessageContent::Attachment { .. } => unreachable!(),
                };

                let line = Line::from(vec![
                    Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{}: ", sender), sender_style),
                    Span::raw(text),
                ]);

                if y_offset >= 0 && y_offset < inner_area.height as i16 {
                    let msg_rect = Rect {
                        x: inner_area.x,
                        y: inner_area.y + y_offset as u16,
                        width: inner_area.width,
                        height: 1,
                    };
                    frame.render_widget(Paragraph::new(line), msg_rect);
                }
                y_offset += 1;
            }
        }
    }

    // Show scroll position indicator
    if total_messages > 1 {
        let scroll_pct = ((scroll_offset + 1) as f64 / total_messages as f64 * 100.0) as u16;
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
