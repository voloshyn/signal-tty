mod infrastructure;
mod storage;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use infrastructure::{IncomingMessage, SignalClient, SignalRepository};
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

fn process_incoming_message(
    msg: &IncomingMessage,
    storage: &SqliteStorage,
    my_uuid: Option<&str>,
) -> Option<String> {
    let envelope = &msg.envelope;
    let sender_uuid = envelope.source_uuid.as_ref()?;
    let sender_name = envelope.source_name.clone();
    let timestamp = envelope.timestamp.unwrap_or_else(now_millis);

    if let Some(data) = &envelope.data_message {
        let text = data.message.clone().unwrap_or_default();
        if text.is_empty() && data.attachments.is_empty() {
            return None;
        }

        let group_info = data.group_info.as_ref();
        let is_outgoing = my_uuid.map(|u| u == sender_uuid).unwrap_or(false);

        let conversation = if let Some(group) = group_info {
            storage
                .get_or_create_group_conversation(&group.group_id, None)
                .ok()?
        } else {
            storage
                .get_or_create_direct_conversation(
                    sender_uuid,
                    envelope.source.as_deref(),
                    sender_name.as_deref(),
                )
                .ok()?
        };

        let content = if !text.is_empty() {
            MessageContent::Text { body: text.clone() }
        } else {
            MessageContent::Text {
                body: "[Attachment]".to_string(),
            }
        };

        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation.id.clone(),
            sender_uuid: sender_uuid.clone(),
            sender_name: sender_name.clone(),
            timestamp,
            server_timestamp: None,
            received_at: now_millis(),
            content,
            quote: None,
            is_outgoing,
            is_read: is_outgoing,
            is_deleted: false,
        };

        storage.save_message(&message).ok()?;

        let display = format!(
            "[{}] {}: {}",
            conversation.display_name(),
            sender_name.as_deref().unwrap_or("Unknown"),
            text
        );
        return Some(display);
    }

    if let Some(sync) = &envelope.sync_message {
        if let Some(sent) = &sync.sent_message {
            let text = sent.message.clone().unwrap_or_default();
            if text.is_empty() {
                return None;
            }

            let group_info = sent.group_info.as_ref();

            let conversation = if let Some(group) = group_info {
                storage
                    .get_or_create_group_conversation(&group.group_id, None)
                    .ok()?
            } else if let Some(dest) = &sent.destination_uuid {
                storage
                    .get_or_create_direct_conversation(dest, sent.destination.as_deref(), None)
                    .ok()?
            } else if let Some(dest) = &sent.destination {
                storage
                    .get_or_create_direct_conversation(dest, Some(dest), None)
                    .ok()?
            } else {
                return None;
            };

            let message = Message {
                id: uuid::Uuid::new_v4().to_string(),
                conversation_id: conversation.id.clone(),
                sender_uuid: sender_uuid.clone(),
                sender_name: sender_name.clone(),
                timestamp: sent.timestamp.unwrap_or(timestamp),
                server_timestamp: None,
                received_at: now_millis(),
                content: MessageContent::Text { body: text.clone() },
                quote: None,
                is_outgoing: true,
                is_read: true,
                is_deleted: false,
            };

            storage.save_message(&message).ok()?;

            let display = format!("[{}] You: {}", conversation.display_name(), text);
            return Some(display);
        }
    }

    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = parse_account();

    let db_path = get_data_dir().join("messages.db");
    println!("Database: {}", db_path.display());

    let storage = Arc::new(SqliteStorage::open(&db_path)?);

    let client = SignalClient::new(account);

    println!("Connecting to signal-cli...");
    client.connect().await?;
    println!("Connected! Receiving messages... (press 'q' to quit)\n");

    let mut messages = client.incoming_messages();

    crossterm::terminal::enable_raw_mode()?;

    loop {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key {
                    KeyEvent { code: KeyCode::Char('q'), .. } => break,
                    KeyEvent { code: KeyCode::Char('c'), modifiers, .. } 
                        if modifiers.contains(event::KeyModifiers::CONTROL) => break,
                    _ => {}
                }
            }
        }

        match tokio::time::timeout(Duration::from_millis(50), messages.recv()).await {
            Ok(Ok(msg)) => {
                if let Some(display) = process_incoming_message(&msg, &storage, None) {
                    println!("{}\r", display);
                }
            }
            Ok(Err(_)) => {
                println!("Message channel closed\r");
                break;
            }
            Err(_) => {}
        }
    }

    crossterm::terminal::disable_raw_mode()?;

    println!("\nDisconnecting...");
    client.disconnect().await?;

    let conversations = storage.list_conversations()?;
    println!("\n--- Stored Conversations ({}) ---", conversations.len());
    for conv in conversations {
        let msgs = storage.list_messages(&conv.id, 1, None)?;
        let last_msg = msgs.first().map(|m| match &m.content {
            MessageContent::Text { body } => {
                if body.len() > 40 {
                    format!("{}...", &body[..40])
                } else {
                    body.clone()
                }
            }
            _ => "[media]".to_string(),
        });
        println!(
            "  {} - {}",
            conv.display_name(),
            last_msg.unwrap_or_else(|| "(no messages)".to_string())
        );
    }

    Ok(())
}
