use rusqlite::Connection;
use super::super::repository::StorageError;

const MIGRATIONS: &[&str] = &[
    // Migration 1: Initial schema
    r#"
    CREATE TABLE IF NOT EXISTS conversations (
        id TEXT PRIMARY KEY,
        conversation_type TEXT NOT NULL,
        recipient_uuid TEXT,
        recipient_number TEXT,
        recipient_name TEXT,
        group_id TEXT,
        group_name TEXT,
        last_message_timestamp INTEGER,
        unread_count INTEGER NOT NULL DEFAULT 0,
        is_archived INTEGER NOT NULL DEFAULT 0,
        is_muted INTEGER NOT NULL DEFAULT 0
    );

    CREATE INDEX IF NOT EXISTS idx_conversations_recipient ON conversations(recipient_uuid);
    CREATE INDEX IF NOT EXISTS idx_conversations_group ON conversations(group_id);
    CREATE INDEX IF NOT EXISTS idx_conversations_last_msg ON conversations(last_message_timestamp);

    CREATE TABLE IF NOT EXISTS messages (
        id TEXT PRIMARY KEY,
        conversation_id TEXT NOT NULL,
        sender_uuid TEXT NOT NULL,
        sender_name TEXT,
        timestamp INTEGER NOT NULL,
        server_timestamp INTEGER,
        received_at INTEGER NOT NULL,
        content_type TEXT NOT NULL,
        content_data TEXT NOT NULL,
        quote_json TEXT,
        is_outgoing INTEGER NOT NULL DEFAULT 0,
        is_read INTEGER NOT NULL DEFAULT 0,
        is_deleted INTEGER NOT NULL DEFAULT 0,
        FOREIGN KEY (conversation_id) REFERENCES conversations(id)
    );

    CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, timestamp);
    CREATE INDEX IF NOT EXISTS idx_messages_signal_id ON messages(sender_uuid, timestamp);

    CREATE TABLE IF NOT EXISTS reactions (
        id TEXT PRIMARY KEY,
        message_id TEXT NOT NULL,
        sender_uuid TEXT NOT NULL,
        emoji TEXT NOT NULL,
        timestamp INTEGER NOT NULL,
        FOREIGN KEY (message_id) REFERENCES messages(id),
        UNIQUE(message_id, sender_uuid, emoji)
    );

    CREATE INDEX IF NOT EXISTS idx_reactions_message ON reactions(message_id);

    CREATE TABLE IF NOT EXISTS delivery_status (
        message_id TEXT NOT NULL,
        recipient_uuid TEXT NOT NULL,
        state TEXT NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (message_id, recipient_uuid),
        FOREIGN KEY (message_id) REFERENCES messages(id)
    );

    CREATE TABLE IF NOT EXISTS group_members (
        group_id TEXT NOT NULL,
        member_uuid TEXT NOT NULL,
        member_name TEXT,
        role TEXT,
        PRIMARY KEY (group_id, member_uuid)
    );

    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER PRIMARY KEY
    );

    INSERT INTO schema_version (version) VALUES (1);
    "#,
    // Migration 2: Add is_edited column to messages
    r#"
    ALTER TABLE messages ADD COLUMN is_edited INTEGER NOT NULL DEFAULT 0;
    UPDATE schema_version SET version = 2;
    "#,
];

pub fn run_migrations(conn: &Connection) -> Result<(), StorageError> {
    let current_version: i32 = conn
        .query_row(
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i32;
        if version > current_version {
            conn.execute_batch(migration)
                .map_err(|e| StorageError::Database(format!("Migration {} failed: {}", version, e)))?;
        }
    }

    Ok(())
}
