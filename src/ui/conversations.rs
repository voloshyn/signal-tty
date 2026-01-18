use crate::app::App;
use crate::avatar::AvatarManager;
use crate::storage::ConversationType;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui_image::StatefulImage;
use ratatui_image::protocol::StatefulProtocol;

const ITEM_HEIGHT: u16 = 2;
const AVATAR_WIDTH: u16 = 5;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    focused: bool,
    avatar_manager: &mut Option<AvatarManager>,
) {
    let border_color = if focused {
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

    if app.conversations.is_empty() {
        return;
    }

    let has_avatars = avatar_manager.is_some();

    let [avatar_area, list_area] = if has_avatars {
        Layout::horizontal([Constraint::Length(AVATAR_WIDTH), Constraint::Min(10)]).areas(inner)
    } else {
        [Rect::default(), inner]
    };

    let items: Vec<ListItem> = app
        .conversations
        .iter()
        .enumerate()
        .map(|(i, conv_view)| {
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
                Style::default().fg(Color::White)
            };

            let unread_indicator = if has_unread(conv_view) { " ●" } else { "" };

            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(name, style),
                    Span::styled(unread_indicator, Style::default().fg(Color::Green)),
                ]),
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
    state.select(Some(app.selected));

    frame.render_stateful_widget(list, list_area, &mut state);

    if let Some(mgr) = avatar_manager {
        render_avatars(frame, avatar_area, app, mgr, state.offset());
    }
}

fn render_avatars(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    mgr: &mut AvatarManager,
    scroll_offset: usize,
) {
    let visible_count = (area.height / ITEM_HEIGHT) as usize;

    for (i, conv_view) in app
        .conversations
        .iter()
        .skip(scroll_offset)
        .take(visible_count)
        .enumerate()
    {
        let conv = &conv_view.conversation;

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

fn render_placeholder(frame: &mut Frame, area: Rect, name: &str, conv_type: ConversationType) {
    use ratatui::widgets::Paragraph;

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

    let placeholder = Paragraph::new(first_char.to_string())
        .style(Style::default().fg(Color::White).bg(color))
        .alignment(ratatui::layout::Alignment::Center);

    let centered = Rect {
        x: area.x + area.width.saturating_sub(2) / 2,
        y: area.y,
        width: 2.min(area.width),
        height: 1.min(area.height),
    };

    frame.render_widget(placeholder, centered);
}

fn has_unread(conv_view: &crate::app::ConversationView) -> bool {
    if let Some(ref messages) = conv_view.messages {
        messages.iter().any(|m| !m.is_read && !m.is_outgoing)
    } else {
        false
    }
}
