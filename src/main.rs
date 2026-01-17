mod app;
mod avatar;
mod events;
mod image_cache;
mod infrastructure;
mod storage;
mod ui;

use app::{App, SendTarget};
use avatar::AvatarManager;
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use image_cache::ImageCache;
use infrastructure::{SignalClient, SignalRepository};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;
use storage::{Message, MessageContent, SqliteStorage, StorageRepository};

fn parse_account() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "-a" && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn get_data_dir() -> std::path::PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "signal-tty", "signal-tty") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.to_path_buf()
    } else {
        std::path::PathBuf::from(".")
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn get_my_number() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let accounts_path = std::path::PathBuf::from(home)
        .join(".local/share/signal-cli/data/accounts.json");
    let data = std::fs::read_to_string(accounts_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&data).ok()?;
    json["accounts"].get(0)?["number"].as_str().map(String::from)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = parse_account();
    let my_number = account.clone().or_else(get_my_number);
    let db_path = get_data_dir().join("messages.db");
    let storage = Arc::new(SqliteStorage::open(&db_path)?);
    let signal = SignalClient::new(account);

    signal.connect().await?;
    let mut messages = signal.incoming_messages();

    let mut app = App::new(storage, signal, my_number);
    app.load_conversations();

    let mut avatar_manager = AvatarManager::new();
    let mut image_cache = ImageCache::new();

    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            terminal.draw(|frame| ui::render(frame, &mut app, &mut avatar_manager, &mut image_cache))?;
            needs_redraw = false;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    // For scroll keys, drain any additional pending scroll events to prevent buffering
                    let is_scroll_key = matches!(
                        key.code,
                        KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k') |
                        KeyCode::PageUp | KeyCode::PageDown
                    );
                    
                    events::handle_key_event(&mut app, key);
                    
                    if is_scroll_key {
                        // Drain buffered scroll events (process them but skip redundant renders)
                        while event::poll(Duration::from_millis(0))? {
                            if let Event::Key(next_key) = event::read()? {
                                events::handle_key_event(&mut app, next_key);
                            }
                        }
                    }
                    
                    needs_redraw = true;
                }
                Event::Resize(_, _) => {
                    // Window resized - clear image cache and redraw
                    if let Some(cache) = image_cache.as_mut() {
                        cache.clear();
                    }
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        match tokio::time::timeout(Duration::from_millis(10), messages.recv()).await {
            Ok(Ok(msg)) => {
                app.handle_incoming_message(msg);
                needs_redraw = true;
            }
            Ok(Err(_)) => {
                app.status_message = Some("Signal connection lost".to_string());
                needs_redraw = true;
            }
            Err(_) => {
                // Timeout, no message
            }
        }

        if let Some(text) = app.pending_send.take() {
            needs_redraw = true;
            if let Some(target) = app.get_send_target() {
                let result = match &target {
                    SendTarget::Direct(recipient) => {
                        app.signal.send_message(recipient, &text).await
                    }
                    SendTarget::Group(group_id) => {
                        app.signal.send_group_message(group_id, &text).await
                    }
                };

                match result {
                    Ok(send_result) => {
                        let my_uuid = app.my_uuid.clone().unwrap_or_default();
                        let conversation_id = app
                            .selected_conversation()
                            .map(|c| c.conversation.id.clone());

                        if let Some(conv_id) = conversation_id {
                            let timestamp = send_result.timestamp.unwrap_or_else(now_millis);
                            let message = Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                conversation_id: conv_id.clone(),
                                sender_uuid: my_uuid,
                                sender_name: None,
                                timestamp,
                                server_timestamp: None,
                                received_at: now_millis(),
                                content: MessageContent::Text { body: text.clone() },
                                quote: None,
                                is_outgoing: true,
                                is_read: true,
                                is_deleted: false,
                            };
                            let _ = app.storage.save_message(&message);
                            app.add_message_to_conversation(&conv_id, message);
                        }
                        app.status_message = Some("Sent".to_string());
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Send failed: {}", e));
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    stdout().execute(cursor::Show)?;
    terminal::disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    app.signal.disconnect().await?;

    Ok(())
}
