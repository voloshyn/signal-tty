use super::models::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub trait StorageRepository: Send + Sync {
    fn get_or_create_direct_conversation(
        &self,
        recipient_uuid: &str,
        recipient_number: Option<&str>,
        recipient_name: Option<&str>,
    ) -> Result<Conversation, StorageError>;

    fn get_or_create_group_conversation(
        &self,
        group_id: &str,
        group_name: Option<&str>,
    ) -> Result<Conversation, StorageError>;

    fn get_conversation(&self, id: &str) -> Result<Option<Conversation>, StorageError>;

    fn get_conversation_by_recipient(
        &self,
        recipient_uuid: &str,
    ) -> Result<Option<Conversation>, StorageError>;

    fn get_conversation_by_group(
        &self,
        group_id: &str,
    ) -> Result<Option<Conversation>, StorageError>;

    fn list_conversations(&self) -> Result<Vec<Conversation>, StorageError>;

    fn update_conversation(&self, conversation: &Conversation) -> Result<(), StorageError>;

    fn save_message(&self, message: &Message) -> Result<(), StorageError>;

    fn get_message(&self, id: &str) -> Result<Option<Message>, StorageError>;

    fn get_message_by_signal_id(
        &self,
        sender_uuid: &str,
        timestamp: i64,
    ) -> Result<Option<Message>, StorageError>;

    fn list_messages(
        &self,
        conversation_id: &str,
        limit: u32,
        before_timestamp: Option<i64>,
    ) -> Result<Vec<Message>, StorageError>;

    fn delete_message(&self, id: &str) -> Result<(), StorageError>;

    fn mark_message_deleted(&self, sender_uuid: &str, timestamp: i64) -> Result<(), StorageError>;

    fn update_message_content(
        &self,
        sender_uuid: &str,
        timestamp: i64,
        new_content: &MessageContent,
    ) -> Result<(), StorageError>;

    fn save_reaction(&self, reaction: &Reaction) -> Result<(), StorageError>;

    fn remove_reaction(
        &self,
        message_id: &str,
        sender_uuid: &str,
        emoji: &str,
    ) -> Result<(), StorageError>;

    fn get_reactions(&self, message_id: &str) -> Result<Vec<Reaction>, StorageError>;

    fn save_delivery_status(&self, status: &DeliveryStatus) -> Result<(), StorageError>;

    fn get_delivery_statuses(&self, message_id: &str) -> Result<Vec<DeliveryStatus>, StorageError>;

    fn mark_messages_read(
        &self,
        conversation_id: &str,
        up_to_timestamp: i64,
    ) -> Result<(), StorageError>;
}
