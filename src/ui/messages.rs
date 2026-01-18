use crate::app::App;
use crate::image_cache::ImageCache;
use crate::storage::MessageContent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::Image;

const DEFAULT_IMAGE_HEIGHT: u16 = 8;

fn calculate_message_height(
    msg: &crate::storage::Message,
    image_cache: &Option<ImageCache>,
    width: u16,
    sender_prefix_len: usize,
) -> u16 {
    match &msg.content {
        MessageContent::Attachment { attachments } => {
            let mut h = 0u16;
            for att in attachments {
                h += 1;
                if ImageCache::is_image(att.content_type.as_deref())
                    && let Some(local_path) = &att.local_path
                {
                    if let Some(cache) = image_cache.as_ref() {
                        h += cache.get_image_height(local_path);
                    } else {
                        h += DEFAULT_IMAGE_HEIGHT;
                    }
                }
            }
            h.max(1)
        }
        _ => {
            let text = match &msg.content {
                MessageContent::Text { body } => body.as_str(),
                MessageContent::Sticker { .. } => "[Sticker]",
                MessageContent::RemoteDeleted => "[Message deleted]",
                MessageContent::Attachment { .. } => "",
            };
            let total_len = sender_prefix_len + text.len();
            if width == 0 {
                1
            } else {
                ((total_len as u16 + width - 1) / width).max(1)
            }
        }
    }
}

fn get_sender_prefix_len(msg: &crate::storage::Message) -> usize {
    let timestamp_len = 8;
    let sender = if msg.is_outgoing {
        "You"
    } else {
        msg.sender_name.as_deref().unwrap_or("Unknown")
    };
    timestamp_len + sender.len() + 4
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    focused: bool,
    image_cache: &mut Option<ImageCache>,
) {
    let border_color = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

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

    let (messages, mut scroll_offset) = {
        let Some(conv_view) = app.selected_conversation() else {
            let empty = Paragraph::new("No conversation selected")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(empty, inner_area);
            return;
        };

        let Some(ref msgs) = conv_view.messages else {
            let loading = Paragraph::new("Loading...").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(loading, inner_area);
            return;
        };

        if msgs.is_empty() {
            let empty =
                Paragraph::new("No messages yet").style(Style::default().fg(Color::DarkGray));
            frame.render_widget(empty, inner_area);
            return;
        }

        (msgs.clone(), conv_view.scroll_offset)
    };

    let visible_height = inner_area.height as usize;
    let max_img_width = inner_area.width.saturating_sub(4);

    let mut total_content_height = 0usize;
    for msg in messages.iter() {
        let prefix_len = get_sender_prefix_len(msg);
        total_content_height +=
            calculate_message_height(msg, image_cache, inner_area.width, prefix_len) as usize;
    }

    let max_scroll = total_content_height.saturating_sub(visible_height);
    scroll_offset = scroll_offset.min(max_scroll);
    if let Some(conv) = app.selected_conversation_mut() {
        conv.scroll_offset = scroll_offset;
    }

    let target_bottom = total_content_height.saturating_sub(scroll_offset);
    let target_top = target_bottom.saturating_sub(visible_height);

    let mut cumulative_height = 0usize;
    let mut start_idx = 0;
    let mut skip_lines_at_start = 0usize;

    for (i, msg) in messages.iter().enumerate() {
        let prefix_len = get_sender_prefix_len(msg);
        let msg_height =
            calculate_message_height(msg, image_cache, inner_area.width, prefix_len) as usize;
        if cumulative_height + msg_height > target_top {
            start_idx = i;
            skip_lines_at_start = target_top.saturating_sub(cumulative_height);
            break;
        }
        cumulative_height += msg_height;
    }

    let mut y_offset: i16 = -(skip_lines_at_start as i16);
    for msg in messages.iter().skip(start_idx) {
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
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };

        match &msg.content {
            MessageContent::Attachment { attachments } => {
                for attachment in attachments {
                    if y_offset >= inner_area.height as i16 {
                        break;
                    }

                    let is_image = ImageCache::is_image(attachment.content_type.as_deref());
                    let name = attachment
                        .filename
                        .as_deref()
                        .or(attachment.id.as_deref())
                        .unwrap_or("file");

                    let header = Line::from(vec![
                        Span::styled(
                            format!("[{}] ", timestamp),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{}: ", sender), sender_style),
                        Span::styled(format!("📎 {}", name), Style::default().fg(Color::Yellow)),
                    ]);

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

                    if is_image
                        && let Some(local_path) = &attachment.local_path
                        && let Some(cache) = image_cache.as_mut()
                    {
                        let img_height = cache.get_image_height(local_path);

                        let img_start = y_offset.max(0) as u16;
                        let img_end =
                            (y_offset + img_height as i16).min(inner_area.height as i16) as u16;

                        if img_end > img_start {
                            if let Some((protocol, img_width, _)) =
                                cache.get_image_with_size(local_path, max_img_width)
                            {
                                let image_rect = Rect {
                                    x: inner_area.x + 2,
                                    y: inner_area.y + img_start,
                                    width: img_width.min(max_img_width),
                                    height: img_end - img_start,
                                };
                                frame.render_widget(Image::new(protocol), image_rect);
                            } else if cache.is_loading(local_path) {
                                let placeholder_rect = Rect {
                                    x: inner_area.x + 2,
                                    y: inner_area.y + img_start,
                                    width: max_img_width,
                                    height: 1,
                                };
                                frame.render_widget(
                                    Paragraph::new("⏳ Loading image...")
                                        .style(Style::default().fg(Color::DarkGray)),
                                    placeholder_rect,
                                );
                            }
                        }
                        y_offset += img_height as i16;
                    }
                }
            }
            _ => {
                let text = match &msg.content {
                    MessageContent::Text { body } => body.clone(),
                    MessageContent::Sticker {
                        pack_id,
                        sticker_id,
                    } => {
                        format!("[Sticker: {}#{}]", pack_id, sticker_id)
                    }
                    MessageContent::RemoteDeleted => "[Message deleted]".to_string(),
                    MessageContent::Attachment { .. } => unreachable!(),
                };

                let prefix = format!("[{}] {}: ", timestamp, sender);
                let prefix_len = prefix.len();
                let msg_height =
                    calculate_message_height(msg, image_cache, inner_area.width, prefix_len)
                        as i16;

                let line = Line::from(vec![
                    Span::styled(
                        format!("[{}] ", timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("{}: ", sender), sender_style),
                    Span::raw(text),
                ]);

                let render_start = y_offset.max(0) as u16;
                let render_end =
                    (y_offset + msg_height).min(inner_area.height as i16) as u16;

                if render_end > render_start {
                    let msg_rect = Rect {
                        x: inner_area.x,
                        y: inner_area.y + render_start,
                        width: inner_area.width,
                        height: render_end - render_start,
                    };
                    frame.render_widget(
                        Paragraph::new(line).wrap(ratatui::widgets::Wrap { trim: false }),
                        msg_rect,
                    );
                }
                y_offset += msg_height;
            }
        }
    }

    if total_content_height > visible_height {
        let scroll_pct = if max_scroll > 0 {
            100 - (scroll_offset as f64 / max_scroll as f64 * 100.0) as u16
        } else {
            100
        };
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
