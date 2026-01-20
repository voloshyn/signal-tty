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
            (total_len as u16).div_ceil(width.max(1)).max(1)
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

    let (messages, mut scroll_offset, selection_range, sel_cursor) = {
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

        let sel_range = conv_view.selection.as_ref().map(|s| s.range());
        let sel_cursor = conv_view.selection.as_ref().map(|s| s.cursor);
        (msgs.clone(), conv_view.scroll_offset, sel_range, sel_cursor)
    };

    let visible_height = inner_area.height as usize;
    let max_img_width = inner_area.width.saturating_sub(4);

    let mut msg_heights: Vec<usize> = Vec::with_capacity(messages.len());
    let mut total_content_height = 0usize;
    for msg in messages.iter() {
        let prefix_len = get_sender_prefix_len(msg);
        let h = calculate_message_height(msg, image_cache, inner_area.width, prefix_len) as usize;
        msg_heights.push(h);
        total_content_height += h;
    }

    let max_scroll = total_content_height.saturating_sub(visible_height);

    if let Some(cursor_idx) = sel_cursor {
        let mut cursor_top = 0usize;
        for h in msg_heights.iter().take(cursor_idx) {
            cursor_top += h;
        }
        let cursor_height = msg_heights.get(cursor_idx).copied().unwrap_or(1);
        let cursor_bottom = cursor_top + cursor_height;

        let view_bottom = total_content_height.saturating_sub(scroll_offset);
        let view_top = view_bottom.saturating_sub(visible_height);

        if cursor_top < view_top {
            scroll_offset = total_content_height.saturating_sub(cursor_top + visible_height);
        } else if cursor_bottom > view_bottom {
            scroll_offset = total_content_height.saturating_sub(cursor_bottom);
        }
    }

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
    let mut end_idx = start_idx;
    for (msg_idx, msg) in messages.iter().enumerate().skip(start_idx) {
        if y_offset >= inner_area.height as i16 {
            break;
        }
        end_idx = msg_idx;

        let is_selected = selection_range
            .as_ref()
            .is_some_and(|r| r.contains(&msg_idx));
        let selection_style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

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
                .patch(selection_style)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
                .patch(selection_style)
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
                            Style::default().fg(Color::DarkGray).patch(selection_style),
                        ),
                        Span::styled(format!("{}: ", sender), sender_style),
                        Span::styled(
                            format!("📎 {}", name),
                            Style::default().fg(Color::Yellow).patch(selection_style),
                        ),
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

                let edited_suffix = if msg.is_edited { " (edited)" } else { "" };

                let prefix = format!("[{}] {}: ", timestamp, sender);
                let prefix_len = prefix.len();
                let msg_height =
                    calculate_message_height(msg, image_cache, inner_area.width, prefix_len) as i16;

                let line = Line::from(vec![
                    Span::styled(
                        format!("[{}] ", timestamp),
                        Style::default().fg(Color::DarkGray).patch(selection_style),
                    ),
                    Span::styled(format!("{}: ", sender), sender_style),
                    Span::styled(text, selection_style),
                    Span::styled(
                        edited_suffix,
                        Style::default().fg(Color::DarkGray).patch(selection_style),
                    ),
                ]);

                let render_start = y_offset.max(0) as u16;
                let render_end = (y_offset + msg_height).min(inner_area.height as i16) as u16;

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

    if let Some(conv) = app.selected_conversation_mut() {
        conv.visible_range = Some((start_idx, end_idx));
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
