use crate::app::{App, Focus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
            if modifiers.is_empty() && app.focus != Focus::Input =>
        {
            app.should_quit = true;
            return;
        }
        KeyEvent { code: KeyCode::Tab, .. } => {
            app.cycle_focus();
            return;
        }
        KeyEvent { code: KeyCode::Esc, .. } => {
            app.focus = Focus::Conversations;
            return;
        }
        _ => {}
    }

    // Focus-specific handling
    match app.focus {
        Focus::Conversations => handle_conversations_key(app, key),
        Focus::Messages => handle_messages_key(app, key),
        Focus::Input => handle_input_key(app, key),
    }
}

fn handle_conversations_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Enter => {
            app.focus = Focus::Input;
        }
        _ => {}
    }
}

fn handle_messages_key(app: &mut App, key: KeyEvent) {
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
            // Scroll to top (oldest messages)
            for _ in 0..1000 {
                app.scroll_messages_up();
            }
        }
        KeyCode::End => {
            if let Some(conv) = app.selected_conversation_mut() {
                conv.scroll_to_bottom();
            }
        }
        KeyCode::Enter => {
            app.focus = Focus::Input;
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
