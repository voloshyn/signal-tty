use crate::app::{App, Focus, MessageSelection};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

const ITEM_HEIGHT: u16 = 4;

pub fn handle_mouse_event(app: &mut App, event: MouseEvent) {
    let x = event.column;
    let y = event.row;

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_click(app, x, y);
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            handle_left_drag(app, x, y);
        }
        MouseEventKind::ScrollUp => {
            if is_in_rect(x, y, app.layout_areas.messages) {
                app.scroll_messages_up();
            } else if is_in_rect(x, y, app.layout_areas.conversations) {
                app.select_prev();
            }
        }
        MouseEventKind::ScrollDown => {
            if is_in_rect(x, y, app.layout_areas.messages) {
                app.scroll_messages_down();
            } else if is_in_rect(x, y, app.layout_areas.conversations) {
                app.select_next();
            }
        }
        _ => {}
    }
}

fn handle_left_click(app: &mut App, x: u16, y: u16) {
    if is_in_rect(x, y, app.layout_areas.conversations_list) {
        app.focus = Focus::Conversations;
        if let Some(conv) = app.selected_conversation_mut() {
            conv.exit_selection_mode();
        }

        let list_area = app.layout_areas.conversations_list;
        let scroll_offset = app.layout_areas.conversations_scroll_offset;
        let relative_y = y.saturating_sub(list_area.y);
        let item_index = (relative_y / ITEM_HEIGHT) as usize + scroll_offset;

        let filtered_indices = app.filtered_conversation_indices();
        if item_index < filtered_indices.len() {
            let new_selected = filtered_indices[item_index];
            if new_selected != app.selected {
                app.selected = new_selected;
                if app.conversations[app.selected].load_messages(&app.storage.clone()) {
                    app.needs_image_preload = true;
                }
            }
        }
    } else if is_in_rect(x, y, app.layout_areas.messages) {
        app.focus = Focus::Messages;
        handle_message_click(app, x, y, false);
    } else if is_in_rect(x, y, app.layout_areas.input) {
        app.focus = Focus::Input;
    }
}

fn handle_left_drag(app: &mut App, x: u16, y: u16) {
    if is_in_rect(x, y, app.layout_areas.messages) {
        handle_message_click(app, x, y, true);
    }
}

fn handle_message_click(app: &mut App, _x: u16, y: u16, extend: bool) {
    let msg_area = app.layout_areas.messages;
    let inner_y = y.saturating_sub(msg_area.y + 1);

    let msg_positions = app.message_y_positions.clone();
    if msg_positions.is_empty() {
        return;
    }

    let mut clicked_idx = None;
    for &(msg_idx, start_y, end_y) in &msg_positions {
        if inner_y >= start_y && inner_y < end_y {
            clicked_idx = Some(msg_idx);
            break;
        }
    }

    if clicked_idx.is_none() {
        if let Some((last_idx, _, _)) = msg_positions.last() {
            clicked_idx = Some(*last_idx);
        }
    }

    let Some(idx) = clicked_idx else {
        return;
    };

    if let Some(conv) = app.selected_conversation_mut() {
        if extend {
            if let Some(ref mut sel) = conv.selection {
                sel.cursor = idx;
            } else {
                conv.selection = Some(MessageSelection {
                    anchor: idx,
                    cursor: idx,
                });
            }
        } else {
            conv.selection = Some(MessageSelection {
                anchor: idx,
                cursor: idx,
            });
        }
    }
}

fn is_in_rect(x: u16, y: u16, rect: ratatui::layout::Rect) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}
