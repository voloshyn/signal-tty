mod migrations;

use super::models::*;
use super::repository::{StorageError, StorageRepository};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let conn = Connection::open(path).map_err(|e| StorageError::Database(e.to_string()))?;

        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.run_migrations()?;
        Ok(storage)
    }

    fn run_migrations(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        migrations::run_migrations(&conn)
    }

    fn parse_conversation_type(s: &str) -> ConversationType {
        match s {
            "group" => ConversationType::Group,
            _ => ConversationType::Direct,
        }
    }

    fn conversation_type_str(t: ConversationType) -> &'static str {
        match t {
            ConversationType::Direct => "direct",
            ConversationType::Group => "group",
        }
    }

    fn parse_message_content(content_type: &str, content_data: &str) -> MessageContent {
        match content_type {
            "text" => MessageContent::Text {
                body: content_data.to_string(),
            },
            "attachment" => {
                let attachments: Vec<AttachmentInfo> =
                    serde_json::from_str(content_data).unwrap_or_default();
                MessageContent::Attachment { attachments }
            }
            "sticker" => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(content_data) {
                    MessageContent::Sticker {
                        pack_id: v
                            .get("pack_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        sticker_id: v.get("sticker_id").and_then(|v| v.as_i64()).unwrap_or(0)
                            as i32,
                    }
                } else {
                    MessageContent::Text {
                        body: String::new(),
                    }
                }
            }
            "deleted" => MessageContent::RemoteDeleted,
            _ => MessageContent::Text {
                body: content_data.to_string(),
            },
        }
    }

    fn message_content_to_parts(content: &MessageContent) -> (&'static str, String) {
        match content {
            MessageContent::Text { body } => ("text", body.clone()),
            MessageContent::Attachment { attachments } => (
                "attachment",
                serde_json::to_string(attachments).unwrap_or_default(),
            ),
            MessageContent::Sticker {
                pack_id,
                sticker_id,
            } => {
                let data = serde_json::json!({ "pack_id": pack_id, "sticker_id": sticker_id });
                ("sticker", data.to_string())
            }
            MessageContent::RemoteDeleted => ("deleted", String::new()),
        }
    }
}

impl StorageRepository for SqliteStorage {
    fn get_or_create_direct_conversation(
        &self,
        recipient_uuid: &str,
        recipient_number: Option<&str>,
        recipient_name: Option<&str>,
    ) -> Result<Conversation, StorageError> {
        if let Some(conv) = self.get_conversation_by_recipient(recipient_uuid)? {
            if recipient_name.is_some() || recipient_number.is_some() {
                let mut updated = conv.clone();
                if recipient_name.is_some() {
                    updated.recipient_name = recipient_name.map(|s| s.to_string());
                }
                if recipient_number.is_some() {
                    updated.recipient_number = recipient_number.map(|s| s.to_string());
                }
                self.update_conversation(&updated)?;
                return Ok(updated);
            }
            return Ok(conv);
        }

        let conv = Conversation::new_direct(
            recipient_uuid.to_string(),
            recipient_number.map(|s| s.to_string()),
            recipient_name.map(|s| s.to_string()),
        );

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (id, conversation_type, recipient_uuid, recipient_number, recipient_name, unread_count, is_archived, is_muted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 0)",
            params![
                conv.id,
                Self::conversation_type_str(conv.conversation_type),
                conv.recipient_uuid,
                conv.recipient_number,
                conv.recipient_name,
            ],
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(conv)
    }

    fn get_or_create_group_conversation(
        &self,
        group_id: &str,
        group_name: Option<&str>,
    ) -> Result<Conversation, StorageError> {
        if let Some(conv) = self.get_conversation_by_group(group_id)? {
            if group_name.is_some() && conv.group_name != group_name.map(|s| s.to_string()) {
                let mut updated = conv.clone();
                updated.group_name = group_name.map(|s| s.to_string());
                self.update_conversation(&updated)?;
                return Ok(updated);
            }
            return Ok(conv);
        }

        let conv = Conversation::new_group(group_id.to_string(), group_name.map(|s| s.to_string()));

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (id, conversation_type, group_id, group_name, unread_count, is_archived, is_muted)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 0)",
            params![
                conv.id,
                Self::conversation_type_str(conv.conversation_type),
                conv.group_id,
                conv.group_name,
            ],
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(conv)
    }

    fn get_conversation(&self, id: &str) -> Result<Option<Conversation>, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, conversation_type, recipient_uuid, recipient_number, recipient_name,
                    group_id, group_name, last_message_timestamp, unread_count, is_archived, is_muted
             FROM conversations WHERE id = ?1",
            params![id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    conversation_type: Self::parse_conversation_type(&row.get::<_, String>(1)?),
                    recipient_uuid: row.get(2)?,
                    recipient_number: row.get(3)?,
                    recipient_name: row.get(4)?,
                    group_id: row.get(5)?,
                    group_name: row.get(6)?,
                    last_message_timestamp: row.get(7)?,
                    unread_count: row.get(8)?,
                    is_archived: row.get::<_, i32>(9)? != 0,
                    is_muted: row.get::<_, i32>(10)? != 0,
                })
            },
        ).optional().map_err(|e| StorageError::Database(e.to_string()))
    }

    fn get_conversation_by_recipient(
        &self,
        recipient_uuid: &str,
    ) -> Result<Option<Conversation>, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, conversation_type, recipient_uuid, recipient_number, recipient_name,
                    group_id, group_name, last_message_timestamp, unread_count, is_archived, is_muted
             FROM conversations WHERE recipient_uuid = ?1 AND conversation_type = 'direct'",
            params![recipient_uuid],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    conversation_type: Self::parse_conversation_type(&row.get::<_, String>(1)?),
                    recipient_uuid: row.get(2)?,
                    recipient_number: row.get(3)?,
                    recipient_name: row.get(4)?,
                    group_id: row.get(5)?,
                    group_name: row.get(6)?,
                    last_message_timestamp: row.get(7)?,
                    unread_count: row.get(8)?,
                    is_archived: row.get::<_, i32>(9)? != 0,
                    is_muted: row.get::<_, i32>(10)? != 0,
                })
            },
        ).optional().map_err(|e| StorageError::Database(e.to_string()))
    }

    fn get_conversation_by_group(
        &self,
        group_id: &str,
    ) -> Result<Option<Conversation>, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, conversation_type, recipient_uuid, recipient_number, recipient_name,
                    group_id, group_name, last_message_timestamp, unread_count, is_archived, is_muted
             FROM conversations WHERE group_id = ?1 AND conversation_type = 'group'",
            params![group_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    conversation_type: Self::parse_conversation_type(&row.get::<_, String>(1)?),
                    recipient_uuid: row.get(2)?,
                    recipient_number: row.get(3)?,
                    recipient_name: row.get(4)?,
                    group_id: row.get(5)?,
                    group_name: row.get(6)?,
                    last_message_timestamp: row.get(7)?,
                    unread_count: row.get(8)?,
                    is_archived: row.get::<_, i32>(9)? != 0,
                    is_muted: row.get::<_, i32>(10)? != 0,
                })
            },
        ).optional().map_err(|e| StorageError::Database(e.to_string()))
    }

    fn list_conversations(&self) -> Result<Vec<Conversation>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_type, recipient_uuid, recipient_number, recipient_name,
                    group_id, group_name, last_message_timestamp, unread_count, is_archived, is_muted
             FROM conversations
             ORDER BY last_message_timestamp DESC NULLS LAST"
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    conversation_type: Self::parse_conversation_type(&row.get::<_, String>(1)?),
                    recipient_uuid: row.get(2)?,
                    recipient_number: row.get(3)?,
                    recipient_name: row.get(4)?,
                    group_id: row.get(5)?,
                    group_name: row.get(6)?,
                    last_message_timestamp: row.get(7)?,
                    unread_count: row.get(8)?,
                    is_archived: row.get::<_, i32>(9)? != 0,
                    is_muted: row.get::<_, i32>(10)? != 0,
                })
            })
            .map_err(|e| StorageError::Database(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Database(e.to_string()))
    }

    fn update_conversation(&self, conversation: &Conversation) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET
                recipient_number = ?2,
                recipient_name = ?3,
                group_name = ?4,
                last_message_timestamp = ?5,
                unread_count = ?6,
                is_archived = ?7,
                is_muted = ?8
             WHERE id = ?1",
            params![
                conversation.id,
                conversation.recipient_number,
                conversation.recipient_name,
                conversation.group_name,
                conversation.last_message_timestamp,
                conversation.unread_count,
                conversation.is_archived as i32,
                conversation.is_muted as i32,
            ],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn save_message(&self, message: &Message) -> Result<(), StorageError> {
        let (content_type, content_data) = Self::message_content_to_parts(&message.content);
        let quote_json = message
            .quote
            .as_ref()
            .map(|q| serde_json::to_string(q).unwrap_or_default());

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO messages
             (id, conversation_id, sender_uuid, sender_name, timestamp, server_timestamp, received_at,
              content_type, content_data, quote_json, is_outgoing, is_read, is_deleted, is_edited)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                message.id,
                message.conversation_id,
                message.sender_uuid,
                message.sender_name,
                message.timestamp,
                message.server_timestamp,
                message.received_at,
                content_type,
                content_data,
                quote_json,
                message.is_outgoing as i32,
                message.is_read as i32,
                message.is_deleted as i32,
                message.is_edited as i32,
            ],
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        conn.execute(
            "UPDATE conversations SET last_message_timestamp = MAX(COALESCE(last_message_timestamp, 0), ?2)
             WHERE id = ?1",
            params![message.conversation_id, message.timestamp],
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_message(&self, id: &str) -> Result<Option<Message>, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, conversation_id, sender_uuid, sender_name, timestamp, server_timestamp, received_at,
                    content_type, content_data, quote_json, is_outgoing, is_read, is_deleted, is_edited
             FROM messages WHERE id = ?1",
            params![id],
            |row| {
                let content_type: String = row.get(7)?;
                let content_data: String = row.get(8)?;
                let quote_json: Option<String> = row.get(9)?;

                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    sender_uuid: row.get(2)?,
                    sender_name: row.get(3)?,
                    timestamp: row.get(4)?,
                    server_timestamp: row.get(5)?,
                    received_at: row.get(6)?,
                    content: Self::parse_message_content(&content_type, &content_data),
                    quote: quote_json.and_then(|s| serde_json::from_str(&s).ok()),
                    is_outgoing: row.get::<_, i32>(10)? != 0,
                    is_read: row.get::<_, i32>(11)? != 0,
                    is_deleted: row.get::<_, i32>(12)? != 0,
                    is_edited: row.get::<_, i32>(13)? != 0,
                })
            },
        ).optional().map_err(|e| StorageError::Database(e.to_string()))
    }

    fn get_message_by_signal_id(
        &self,
        sender_uuid: &str,
        timestamp: i64,
    ) -> Result<Option<Message>, StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, conversation_id, sender_uuid, sender_name, timestamp, server_timestamp, received_at,
                    content_type, content_data, quote_json, is_outgoing, is_read, is_deleted, is_edited
             FROM messages WHERE sender_uuid = ?1 AND timestamp = ?2",
            params![sender_uuid, timestamp],
            |row| {
                let content_type: String = row.get(7)?;
                let content_data: String = row.get(8)?;
                let quote_json: Option<String> = row.get(9)?;

                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    sender_uuid: row.get(2)?,
                    sender_name: row.get(3)?,
                    timestamp: row.get(4)?,
                    server_timestamp: row.get(5)?,
                    received_at: row.get(6)?,
                    content: Self::parse_message_content(&content_type, &content_data),
                    quote: quote_json.and_then(|s| serde_json::from_str(&s).ok()),
                    is_outgoing: row.get::<_, i32>(10)? != 0,
                    is_read: row.get::<_, i32>(11)? != 0,
                    is_deleted: row.get::<_, i32>(12)? != 0,
                    is_edited: row.get::<_, i32>(13)? != 0,
                })
            },
        ).optional().map_err(|e| StorageError::Database(e.to_string()))
    }

    fn list_messages(
        &self,
        conversation_id: &str,
        limit: u32,
        before_timestamp: Option<i64>,
    ) -> Result<Vec<Message>, StorageError> {
        let conn = self.conn.lock().unwrap();

        let (sql, params_vec): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(ts) =
            before_timestamp
        {
            (
                "SELECT id, conversation_id, sender_uuid, sender_name, timestamp, server_timestamp, received_at,
                        content_type, content_data, quote_json, is_outgoing, is_read, is_deleted, is_edited
                 FROM messages WHERE conversation_id = ?1 AND timestamp < ?2
                 ORDER BY timestamp DESC LIMIT ?3",
                vec![Box::new(conversation_id.to_string()), Box::new(ts), Box::new(limit)]
            )
        } else {
            (
                "SELECT id, conversation_id, sender_uuid, sender_name, timestamp, server_timestamp, received_at,
                        content_type, content_data, quote_json, is_outgoing, is_read, is_deleted, is_edited
                 FROM messages WHERE conversation_id = ?1
                 ORDER BY timestamp DESC LIMIT ?2",
                vec![Box::new(conversation_id.to_string()), Box::new(limit)]
            )
        };

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                let content_type: String = row.get(7)?;
                let content_data: String = row.get(8)?;
                let quote_json: Option<String> = row.get(9)?;

                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    sender_uuid: row.get(2)?,
                    sender_name: row.get(3)?,
                    timestamp: row.get(4)?,
                    server_timestamp: row.get(5)?,
                    received_at: row.get(6)?,
                    content: Self::parse_message_content(&content_type, &content_data),
                    quote: quote_json.and_then(|s| serde_json::from_str(&s).ok()),
                    is_outgoing: row.get::<_, i32>(10)? != 0,
                    is_read: row.get::<_, i32>(11)? != 0,
                    is_deleted: row.get::<_, i32>(12)? != 0,
                    is_edited: row.get::<_, i32>(13)? != 0,
                })
            })
            .map_err(|e| StorageError::Database(e.to_string()))?;

        let mut messages: Vec<Message> = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        messages.reverse();
        Ok(messages)
    }

    fn delete_message(&self, id: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM messages WHERE id = ?1", params![id])
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn mark_message_deleted(&self, sender_uuid: &str, timestamp: i64) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET is_deleted = 1, content_type = 'deleted', content_data = ''
             WHERE sender_uuid = ?1 AND timestamp = ?2",
            params![sender_uuid, timestamp],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn update_message_content(
        &self,
        sender_uuid: &str,
        timestamp: i64,
        new_content: &MessageContent,
    ) -> Result<(), StorageError> {
        let (content_type, content_data) = Self::message_content_to_parts(new_content);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET content_type = ?3, content_data = ?4, is_edited = 1
             WHERE sender_uuid = ?1 AND timestamp = ?2",
            params![sender_uuid, timestamp, content_type, content_data],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn save_reaction(&self, reaction: &Reaction) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO reactions (id, message_id, sender_uuid, emoji, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                reaction.id,
                reaction.message_id,
                reaction.sender_uuid,
                reaction.emoji,
                reaction.timestamp,
            ],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn remove_reaction(
        &self,
        message_id: &str,
        sender_uuid: &str,
        emoji: &str,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM reactions WHERE message_id = ?1 AND sender_uuid = ?2 AND emoji = ?3",
            params![message_id, sender_uuid, emoji],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn get_reactions(&self, message_id: &str) -> Result<Vec<Reaction>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, message_id, sender_uuid, emoji, timestamp FROM reactions WHERE message_id = ?1"
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![message_id], |row| {
                Ok(Reaction {
                    id: row.get(0)?,
                    message_id: row.get(1)?,
                    sender_uuid: row.get(2)?,
                    emoji: row.get(3)?,
                    timestamp: row.get(4)?,
                })
            })
            .map_err(|e| StorageError::Database(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Database(e.to_string()))
    }

    fn save_delivery_status(&self, status: &DeliveryStatus) -> Result<(), StorageError> {
        let state_str = match status.state {
            DeliveryState::Sending => "sending",
            DeliveryState::Sent => "sent",
            DeliveryState::Delivered => "delivered",
            DeliveryState::Read => "read",
            DeliveryState::Failed => "failed",
        };

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO delivery_status (message_id, recipient_uuid, state, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                status.message_id,
                status.recipient_uuid,
                state_str,
                status.updated_at
            ],
        )
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    fn get_delivery_statuses(&self, message_id: &str) -> Result<Vec<DeliveryStatus>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT message_id, recipient_uuid, state, updated_at FROM delivery_status WHERE message_id = ?1"
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![message_id], |row| {
                let state_str: String = row.get(2)?;
                let state = match state_str.as_str() {
                    "sending" => DeliveryState::Sending,
                    "sent" => DeliveryState::Sent,
                    "delivered" => DeliveryState::Delivered,
                    "read" => DeliveryState::Read,
                    "failed" => DeliveryState::Failed,
                    _ => DeliveryState::Sending,
                };

                Ok(DeliveryStatus {
                    message_id: row.get(0)?,
                    recipient_uuid: row.get(1)?,
                    state,
                    updated_at: row.get(3)?,
                })
            })
            .map_err(|e| StorageError::Database(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| StorageError::Database(e.to_string()))
    }

    fn mark_messages_read(
        &self,
        conversation_id: &str,
        up_to_timestamp: i64,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages SET is_read = 1 WHERE conversation_id = ?1 AND timestamp <= ?2 AND is_read = 0",
            params![conversation_id, up_to_timestamp],
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        conn.execute(
            "UPDATE conversations SET unread_count = (
                SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND is_read = 0 AND is_outgoing = 0
             ) WHERE id = ?1",
            params![conversation_id],
        ).map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }
}
