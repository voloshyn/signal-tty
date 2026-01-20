mod app;
mod avatar;
mod events;
mod image_cache;
mod infrastructure;
mod storage;
mod ui;

use app::{App, RemoteDeleteTarget, SendTarget};
use avatar::AvatarManager;
use crossterm::ExecutableCommand;
use crossterm::cursor;
use crossterm::event::{self, DisableFocusChange, EnableFocusChange, Event};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use image_cache::ImageCache;
use infrastructure::{SignalClient, SignalRepository};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
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

fn mime_from_path(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "tiff" | "tif" => "image/tiff",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        _ => return None,
    };
    Some(mime.to_string())
}

fn get_my_number() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let accounts_path =
        std::path::PathBuf::from(home).join(".local/share/signal-cli/data/accounts.json");
    let data = std::fs::read_to_string(accounts_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&data).ok()?;
    json["accounts"].get(0)?["number"]
        .as_str()
        .map(String::from)
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

    if let Some((recipient, timestamps)) = app.mark_current_conversation_read() {
        let _ = app.signal.send_read_receipt(&recipient, timestamps).await;
    }

    let mut avatar_manager = AvatarManager::new();
    let mut image_cache = ImageCache::new();

    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(cursor::Hide)?;
    stdout().execute(EnableFocusChange)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            terminal
                .draw(|frame| ui::render(frame, &mut app, &mut avatar_manager, &mut image_cache))?;
            needs_redraw = false;
        }

        if event::poll(Duration::from_millis(20))? {
            match event::read()? {
                Event::Key(key) => {
                    events::handle_key_event(&mut app, key);

                    while event::poll(Duration::from_millis(0))? {
                        if let Event::Key(next_key) = event::read()? {
                            events::handle_key_event(&mut app, next_key);
                        }
                    }
                    if let Some(ref mut cache) = image_cache {
                        let paths = app.take_preload_paths();
                        if !paths.is_empty() {
                            // TODO: hardcoded max width
                            cache.preload_images(&paths, 60);
                        }
                    }

                    if let Some((recipient, timestamps)) = app.mark_current_conversation_read() {
                        let _ = app.signal.send_read_receipt(&recipient, timestamps).await;
                    }

                    needs_redraw = true;
                }
                Event::Resize(_, _) => {
                    needs_redraw = true;
                }
                Event::FocusGained => {
                    terminal.clear()?;
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        if let Some(ref mut cache) = image_cache
            && cache.process_next_loaded_image()
        {
            needs_redraw = true;
        }

        // Check for incoming Signal messages
        match tokio::time::timeout(Duration::from_millis(20), messages.recv()).await {
            Ok(Ok(msg)) => {
                app.handle_incoming_message(msg);
                if let Some((recipient, timestamps)) = app.mark_current_conversation_read() {
                    let _ = app.signal.send_read_receipt(&recipient, timestamps).await;
                }
                needs_redraw = true;
            }
            Ok(Err(_)) => {
                app.status_message = Some("Signal connection lost".to_string());
                needs_redraw = true;
            }
            Err(_) => {}
        }

        if let Some(text) = app.pending_send.take() {
            let attachments = std::mem::take(&mut app.pending_attachments);
            needs_redraw = true;
            if let Some(target) = app.get_send_target() {
                let my_uuid = app.my_uuid.clone().unwrap_or_default();
                let conv_id = app
                    .selected_conversation()
                    .map(|c| c.conversation.id.clone());

                let content = if !attachments.is_empty() {
                    let att_info: Vec<_> = attachments
                        .iter()
                        .map(|p| storage::AttachmentInfo {
                            id: None,
                            content_type: mime_from_path(p),
                            filename: p.file_name().map(|n| n.to_string_lossy().to_string()),
                            size: p.metadata().ok().map(|m| m.len()),
                            local_path: Some(p.to_string_lossy().to_string()),
                        })
                        .collect();
                    MessageContent::Attachment { attachments: att_info }
                } else {
                    MessageContent::Text { body: text.clone() }
                };

                let mut message = None;
                if let Some(ref conv_id) = conv_id {
                    let msg = Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        conversation_id: conv_id.clone(),
                        sender_uuid: my_uuid,
                        sender_name: None,
                        timestamp: now_millis(),
                        server_timestamp: None,
                        received_at: now_millis(),
                        content,
                        quote: None,
                        is_outgoing: true,
                        is_read: true,
                        is_deleted: false,
                        is_edited: false,
                    };
                    app.add_message_to_conversation(conv_id, msg.clone());
                    message = Some(msg);
                }

                for att_path in &attachments {
                    if mime_from_path(att_path).is_some_and(|m| m.starts_with("image/")) {
                        app.pending_preload_paths.push(att_path.to_string_lossy().to_string());
                    }
                }

                let attachment_paths: Vec<String> = attachments
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                let result = match &target {
                    SendTarget::Direct(recipient) => {
                        if attachment_paths.is_empty() {
                            app.signal.send_message(recipient, &text).await
                        } else {
                            app.signal.send_message_with_attachments(recipient, &text, attachment_paths).await
                        }
                    }
                    SendTarget::Group(group_id) => {
                        app.signal.send_group_message(group_id, &text).await
                    }
                };

                match result {
                    Ok(send_result) => {
                        if let Some(mut msg) = message {
                            let old_ts = msg.timestamp;
                            if let Some(ts) = send_result.timestamp {
                                msg.timestamp = ts;
                                if let Some(conv) = app.selected_conversation_mut() {
                                    if let Some(ref mut msgs) = conv.messages {
                                        if let Some(m) = msgs.iter_mut().find(|m| m.timestamp == old_ts) {
                                            m.timestamp = ts;
                                        }
                                    }
                                }
                            }
                            let _ = app.storage.save_message(&msg);
                        }
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Send failed: {}", e));
                    }
                }
            }
        }

        for pending in std::mem::take(&mut app.pending_remote_deletes) {
            for ts in pending.timestamps {
                let result = match &pending.target {
                    RemoteDeleteTarget::Direct(recipient) => {
                        app.signal.remote_delete(recipient, ts).await
                    }
                    RemoteDeleteTarget::Group(group_id) => {
                        app.signal.remote_delete_group(group_id, ts).await
                    }
                };
                if let Err(e) = result {
                    app.status_message = Some(format!("Remote delete failed: {}", e));
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    stdout().execute(cursor::Show)?;
    stdout().execute(DisableFocusChange)?;
    terminal::disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    app.signal.disconnect().await?;

    Ok(())
}
