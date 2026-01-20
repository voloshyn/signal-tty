use super::error::SignalError;
use super::types::*;
use async_trait::async_trait;
use tokio::sync::broadcast;

#[async_trait]
pub trait SignalRepository: Send + Sync {
    async fn connect(&self) -> Result<(), SignalError>;
    fn is_connected(&self) -> bool;
    async fn disconnect(&self) -> Result<(), SignalError>;

    async fn get_account_info(&self) -> Result<Account, SignalError>;
    async fn list_accounts(&self) -> Result<Vec<String>, SignalError>;

    async fn send_message(&self, recipient: &str, message: &str) -> Result<SendResult, SignalError>;
    async fn send_group_message(&self, group_id: &str, message: &str) -> Result<SendResult, SignalError>;
    async fn send_message_with_attachments(&self, recipient: &str, message: &str, attachments: Vec<String>) -> Result<SendResult, SignalError>;
    async fn send_reaction(&self, recipient: &str, emoji: &str, target_author: &str, target_timestamp: i64) -> Result<(), SignalError>;
    async fn remove_reaction(&self, recipient: &str, emoji: &str, target_author: &str, target_timestamp: i64) -> Result<(), SignalError>;

    async fn list_contacts(&self) -> Result<Vec<Contact>, SignalError>;
    async fn get_contact(&self, identifier: &str) -> Result<Contact, SignalError>;
    async fn update_contact_name(&self, identifier: &str, name: &str) -> Result<(), SignalError>;
    async fn block_contact(&self, identifier: &str) -> Result<(), SignalError>;
    async fn unblock_contact(&self, identifier: &str) -> Result<(), SignalError>;

    async fn list_groups(&self) -> Result<Vec<Group>, SignalError>;
    async fn get_group(&self, group_id: &str) -> Result<Group, SignalError>;
    async fn leave_group(&self, group_id: &str) -> Result<(), SignalError>;
    async fn block_group(&self, group_id: &str) -> Result<(), SignalError>;

    async fn list_identities(&self) -> Result<Vec<Identity>, SignalError>;
    async fn trust_identity(&self, identifier: &str, trust_all_keys: bool) -> Result<(), SignalError>;

    async fn send_typing_started(&self, recipient: &str) -> Result<(), SignalError>;
    async fn send_typing_stopped(&self, recipient: &str) -> Result<(), SignalError>;

    async fn send_read_receipt(&self, recipient: &str, timestamps: Vec<i64>) -> Result<(), SignalError>;

    async fn remote_delete(&self, recipient: &str, target_timestamp: i64) -> Result<(), SignalError>;
    async fn remote_delete_group(&self, group_id: &str, target_timestamp: i64) -> Result<(), SignalError>;

    fn incoming_messages(&self) -> broadcast::Receiver<IncomingMessage>;
}
