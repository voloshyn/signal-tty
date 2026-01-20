use crate::app::{App, Focus, PendingRemoteDelete};
use crate::storage::StorageRepository;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::io::Write;
use std::process::{Command, Stdio};

fn copy_to_clipboard(text: &str) {
    if let Ok(mut child) = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        return;
    }

    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text);
    }
}

pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Global shortcuts
    match key {
        KeyEvent { code: KeyCode::Char('c'), modifiers, .. }
            if modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.should_quit = true;
            return;
        }
        KeyEvent { code: KeyCode::Char('q'), modifiers, .. }
            if modifiers.is_empty()
                && app.focus != Focus::Input
                && app.focus != Focus::ConversationFilter =>
        {
            app.should_quit = true;
            return;
        }
        KeyEvent { code: KeyCode::Tab, .. } => {
            app.cycle_focus();
            return;
        }
        KeyEvent { code: KeyCode::Esc, .. } => {
            if app
                .selected_conversation()
                .is_some_and(|c| c.selection.is_some())
            {
                if let Some(conv) = app.selected_conversation_mut() {
                    conv.exit_selection_mode();
                }
                return;
            }
            app.filter_input.clear();
            app.focus = if app.focus == Focus::Input {
                Focus::Messages
            } else {
                Focus::Conversations
            };
            return;
        }
        KeyEvent {
            code: KeyCode::Char('h'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => {
            app.filter_input.clear();
            app.focus = Focus::Conversations;
            return;
        }
        KeyEvent {
            code: KeyCode::Char('l'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) => {
            app.focus = Focus::Messages;
            return;
        }
        _ => {}
    }

    // Focus-specific handling
    match app.focus {
        Focus::Conversations => handle_conversations_key(app, key),
        Focus::ConversationFilter => handle_conversation_filter_key(app, key),
        Focus::Messages => handle_messages_key(app, key),
        Focus::Input => handle_input_key(app, key),
    }
}

fn handle_conversations_key(app: &mut App, key: KeyEvent) {
    let has_filter = !app.filter_input.text.is_empty();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if has_filter {
                app.select_filtered(-1);
            } else {
                app.select_prev();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if has_filter {
                app.select_filtered(1);
            } else {
                app.select_next();
            }
        }
        KeyCode::Char('/') => {
            app.focus = Focus::ConversationFilter;
        }
        KeyCode::Enter | KeyCode::Char('i') => {
            app.focus = Focus::Input;
        }
        _ => {}
    }
}

fn handle_conversation_filter_key(app: &mut App, key: KeyEvent) {
    match key {
        KeyEvent { code: KeyCode::Enter, .. } => {
            app.focus = Focus::Conversations;
        }
        KeyEvent { code: KeyCode::Up, .. } => {
            app.select_filtered(-1);
        }
        KeyEvent { code: KeyCode::Down, .. } => {
            app.select_filtered(1);
        }
        KeyEvent { code: KeyCode::Backspace, .. } => {
            app.filter_input.delete_back();
            app.ensure_selection_matches_filter();
        }
        KeyEvent { code: KeyCode::Delete, .. } => {
            app.filter_input.delete_forward();
            app.ensure_selection_matches_filter();
        }
        KeyEvent { code: KeyCode::Left, .. } => {
            app.filter_input.move_left();
        }
        KeyEvent { code: KeyCode::Right, .. } => {
            app.filter_input.move_right();
        }
        KeyEvent { code: KeyCode::Home, .. } => {
            app.filter_input.move_start();
        }
        KeyEvent { code: KeyCode::End, .. } => {
            app.filter_input.move_end();
        }
        KeyEvent { code: KeyCode::Char(c), modifiers, .. }
            if !modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.filter_input.insert(c);
            app.ensure_selection_matches_filter();
        }
        _ => {}
    }
}

fn handle_messages_key(app: &mut App, key: KeyEvent) {
    let in_selection = app
        .selected_conversation()
        .is_some_and(|c| c.selection.is_some());

    if in_selection {
        handle_selection_key(app, key);
        return;
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.scroll_messages_up(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_messages_down(),
        KeyCode::PageUp => {
            for _ in 0..10 {
                app.scroll_messages_up();
            }
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                app.scroll_messages_down();
            }
        }
        KeyCode::Home => {
            for _ in 0..1000 {
                app.scroll_messages_up();
            }
        }
        KeyCode::End => {
            if let Some(conv) = app.selected_conversation_mut() {
                conv.scroll_to_bottom();
            }
        }
        KeyCode::Enter | KeyCode::Char('i') => {
            app.focus = Focus::Input;
        }
        KeyCode::Char('v') => {
            if let Some(conv) = app.selected_conversation_mut() {
                conv.enter_selection_mode();
            }
        }
        _ => {}
    }
}

fn handle_selection_key(app: &mut App, key: KeyEvent) {
    match key {
        KeyEvent {
            code: KeyCode::Up | KeyCode::Char('k'),
            modifiers,
            ..
        } => {
            let extend = modifiers.contains(KeyModifiers::SHIFT);
            if let Some(conv) = app.selected_conversation_mut() {
                conv.move_selection(-1, extend);
            }
        }
        KeyEvent {
            code: KeyCode::Char('K'),
            ..
        } => {
            if let Some(conv) = app.selected_conversation_mut() {
                conv.move_selection(-1, true);
            }
        }
        KeyEvent {
            code: KeyCode::Down | KeyCode::Char('j'),
            modifiers,
            ..
        } => {
            let extend = modifiers.contains(KeyModifiers::SHIFT);
            if let Some(conv) = app.selected_conversation_mut() {
                conv.move_selection(1, extend);
            }
        }
        KeyEvent {
            code: KeyCode::Char('J'),
            ..
        } => {
            if let Some(conv) = app.selected_conversation_mut() {
                conv.move_selection(1, true);
            }
        }
        KeyEvent {
            code: KeyCode::Char('y'),
            ..
        } => {
            if let Some(text) = app.selected_conversation().and_then(|c| c.get_selected_text()) {
                copy_to_clipboard(&text);
            }
            if let Some(conv) = app.selected_conversation_mut() {
                conv.exit_selection_mode();
            }
        }
        KeyEvent {
            code: KeyCode::Char('d'),
            ..
        } => {
            if let Some(conv) = app.selected_conversation_mut() {
                let ids = conv.delete_selected_messages();
                for id in ids {
                    let _ = app.storage.delete_message(&id);
                }
            }
        }
        KeyEvent {
            code: KeyCode::Char('D'),
            ..
        } => {
            if let Some(conv) = app.selected_conversation_mut() {
                let timestamps = conv.get_selected_outgoing_timestamps();
                let target = conv.remote_delete_target();
                let ids = conv.delete_selected_messages();
                for id in &ids {
                    let _ = app.storage.delete_message(id);
                }
                if let Some(target) = target {
                    if !timestamps.is_empty() {
                        app.pending_remote_deletes.push(PendingRemoteDelete {
                            target,
                            timestamps,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

fn handle_input_key(app: &mut App, key: KeyEvent) {
    match key {
        KeyEvent { code: KeyCode::Enter, .. } => {
            let text = app.input.clear();
            if !text.is_empty() {
                app.queue_send_message(text);
            }
        }
        KeyEvent { code: KeyCode::Backspace, .. } => {
            app.input.delete_back();
        }
        KeyEvent { code: KeyCode::Delete, .. } => {
            app.input.delete_forward();
        }
        KeyEvent { code: KeyCode::Left, .. } => {
            app.input.move_left();
        }
        KeyEvent { code: KeyCode::Right, .. } => {
            app.input.move_right();
        }
        KeyEvent { code: KeyCode::Home, .. } => {
            app.input.move_start();
        }
        KeyEvent { code: KeyCode::End, .. } => {
            app.input.move_end();
        }
        KeyEvent { code: KeyCode::Char(c), modifiers, .. } 
            if !modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.input.insert(c);
        }
        _ => {}
    }
}
