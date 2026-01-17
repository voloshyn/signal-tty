use crate::app::App;
use crate::storage::ConversationType;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

pub fn render(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::DarkGray };

    let items: Vec<ListItem> = app
        .conversations
        .iter()
        .enumerate()
        .map(|(i, conv_view)| {
            let conv = &conv_view.conversation;
            let name = conv.display_name();
            
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

            let unread_indicator = if has_unread(conv_view) { "● " } else { "  " };

            ListItem::new(Line::from(vec![
                Span::styled(unread_indicator, Style::default().fg(Color::Green)),
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(name, style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Conversations ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(app.selected));

    frame.render_stateful_widget(list, area, &mut state);
}

fn has_unread(conv_view: &crate::app::ConversationView) -> bool {
    if let Some(ref messages) = conv_view.messages {
        messages.iter().any(|m| !m.is_read && !m.is_outgoing)
    } else {
        false
    }
}
