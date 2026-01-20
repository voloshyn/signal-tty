use crate::app::{App, ConversationView, Focus};
use crate::avatar::AvatarManager;
use crate::storage::{ConversationType, MessageContent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui_image::StatefulImage;
use ratatui_image::protocol::StatefulProtocol;

const ITEM_HEIGHT: u16 = 4;
const AVATAR_WIDTH: u16 = 8;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
    avatar_manager: &mut Option<AvatarManager>,
) {
    let in_filter_mode = app.focus == Focus::ConversationFilter;
    let has_filter = !app.filter_input.text.is_empty();
    let border_color = if focused || in_filter_mode {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(" Conversations ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (list_inner, filter_area) = if in_filter_mode || has_filter {
        let [list_area, filter_area] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
        (list_area, Some(filter_area))
    } else {
        (inner, None)
    };

    if app.conversations.is_empty() {
        if let Some(filter_area) = filter_area {
            render_filter_input(frame, filter_area, app);
        }
        return;
    }

    let has_avatars = avatar_manager.is_some();
    let filtered_indices = app.filtered_conversation_indices();

    let [avatar_area, list_area] = if has_avatars {
        Layout::horizontal([Constraint::Length(AVATAR_WIDTH), Constraint::Min(10)])
            .areas(list_inner)
    } else {
        [Rect::default(), list_inner]
    };

    let selected_in_filtered = filtered_indices
        .iter()
        .position(|&i| i == app.selected)
        .unwrap_or(0);

    let preview_width = list_area.width.saturating_sub(2) as usize;

    let items: Vec<ListItem> = filtered_indices
        .iter()
        .map(|&i| {
            let conv_view = &app.conversations[i];
            let conv = &conv_view.conversation;
            let is_note_to_self = app.my_number.as_ref().is_some_and(|my_num| {
                conv.recipient_number.as_ref() == Some(my_num)
                    || conv.recipient_uuid.as_ref() == app.my_uuid.as_ref()
            });
            let name = if is_note_to_self {
                "Note to Self ✅".to_string()
            } else {
                conv.display_name()
            };

            let prefix = match conv.conversation_type {
                ConversationType::Direct => " ",
                ConversationType::Group => "# ",
            };

            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            };

            let unread_indicator = if has_unread(conv_view) { " ●" } else { "" };

            let (line1, line2) = format_message_preview(conv_view, preview_width);
            let preview_style = Style::default().fg(Color::Gray);

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(name, style),
                    Span::styled(unread_indicator, Style::default().fg(Color::Green)),
                ]),
                Line::from(Span::styled(format!(" {}", line1), preview_style)),
                Line::from(Span::styled(format!(" {}", line2), preview_style)),
                Line::default(),
            ])
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(Some(selected_in_filtered));

    frame.render_stateful_widget(list, list_area, &mut state);

    if let Some(mgr) = avatar_manager {
        render_avatars_filtered(
            frame,
            avatar_area,
            app,
            mgr,
            &filtered_indices,
            state.offset(),
        );
    }

    if let Some(filter_area) = filter_area {
        render_filter_input(frame, filter_area, app);
    }
}

fn render_placeholder(frame: &mut Frame, area: Rect, name: &str, conv_type: ConversationType) {
    use ratatui::widgets::{Block, Paragraph};

    let first_char = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?');
    let color = match conv_type {
        ConversationType::Direct => Color::Blue,
        ConversationType::Group => Color::Magenta,
    };

    let bg_block = Block::default().style(Style::default().bg(color));
    frame.render_widget(bg_block, area);

    let placeholder = Paragraph::new(first_char.to_string())
        .style(Style::default().fg(Color::White).bg(color))
        .alignment(ratatui::layout::Alignment::Center);

    let text_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1) / 2,
        width: area.width,
        height: 1,
    };

    frame.render_widget(placeholder, text_area);
}

fn has_unread(conv_view: &ConversationView) -> bool {
    if let Some(ref messages) = conv_view.messages {
        messages.iter().any(|m| !m.is_read && !m.is_outgoing)
    } else {
        false
    }
}

fn format_message_preview(conv_view: &ConversationView, max_width: usize) -> (String, String) {
    let last_msg = conv_view
        .last_message_preview
        .as_ref()
        .or_else(|| conv_view.messages.as_ref().and_then(|m| m.last()));

    let Some(last_msg) = last_msg else {
        return (String::new(), String::new());
    };

    let prefix = if last_msg.is_outgoing { "You: " } else { "" };

    let content = match &last_msg.content {
        MessageContent::Text { body } => body.replace('\n', " "),
        MessageContent::Attachment { attachments } => {
            if attachments.len() == 1 {
                format!(
                    "📎 {}",
                    attachments[0].filename.as_deref().unwrap_or("Attachment")
                )
            } else {
                format!("📎 {} attachments", attachments.len())
            }
        }
        MessageContent::Sticker { .. } => "🖼 Sticker".to_string(),
        MessageContent::RemoteDeleted => "Message deleted".to_string(),
    };

    let full = format!("{}{}", prefix, content);
    split_into_lines(&full, max_width)
}

fn split_into_lines(s: &str, max_width: usize) -> (String, String) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_width {
        return (s.to_string(), String::new());
    }

    let line1: String = chars[..max_width].iter().collect();
    let remaining: Vec<char> = chars[max_width..].iter().copied().collect();

    let line2 = if remaining.len() <= max_width {
        remaining.iter().collect()
    } else if max_width > 1 {
        remaining[..max_width - 1].iter().collect::<String>() + "…"
    } else {
        String::new()
    };

    (line1, line2)
}

fn render_avatars_filtered(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    mgr: &mut AvatarManager,
    filtered_indices: &[usize],
    scroll_offset: usize,
) {
    let visible_count = (area.height / ITEM_HEIGHT) as usize;

    for (i, &conv_idx) in filtered_indices
        .iter()
        .skip(scroll_offset)
        .take(visible_count)
        .enumerate()
    {
        let conv = &app.conversations[conv_idx].conversation;

        let y = area.y + (i as u16) * ITEM_HEIGHT;
        if y + ITEM_HEIGHT > area.y + area.height {
            break;
        }

        let avatar_rect = Rect {
            x: area.x,
            y,
            width: area.width,
            height: ITEM_HEIGHT,
        };

        if let Some(protocol) = mgr.get_conversation_avatar(
            conv.recipient_uuid.as_deref(),
            conv.recipient_number.as_deref(),
        ) {
            let image: StatefulImage<StatefulProtocol> = StatefulImage::default();
            frame.render_stateful_widget(image, avatar_rect, protocol);
        } else {
            render_placeholder(
                frame,
                avatar_rect,
                &conv.display_name(),
                conv.conversation_type,
            );
        }
    }
}

fn render_filter_input(frame: &mut Frame, area: Rect, app: &App) {
    use ratatui::widgets::Paragraph;

    let filter_text = &app.filter_input.text;
    let editing = app.focus == Focus::ConversationFilter;

    let mut spans = vec![Span::styled("/", Style::default().fg(Color::Yellow))];

    if editing {
        let cursor_pos = app.filter_input.cursor;
        if filter_text.is_empty() {
            spans.push(Span::styled(
                " ",
                Style::default().bg(Color::White).fg(Color::Black),
            ));
        } else {
            let before_cursor = &filter_text[..cursor_pos];
            let cursor_char = filter_text[cursor_pos..].chars().next();
            let after_cursor = cursor_char
                .map(|c| &filter_text[cursor_pos + c.len_utf8()..])
                .unwrap_or("");

            spans.push(Span::raw(before_cursor));
            spans.push(Span::styled(
                cursor_char
                    .map(|c| c.to_string())
                    .unwrap_or(" ".to_string()),
                Style::default().bg(Color::White).fg(Color::Black),
            ));
            spans.push(Span::raw(after_cursor));
        }
    } else {
        spans.push(Span::styled(
            filter_text,
            Style::default().fg(Color::DarkGray),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}
