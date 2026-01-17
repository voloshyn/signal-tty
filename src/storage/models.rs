use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationType {
    Direct,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub conversation_type: ConversationType,
    pub recipient_uuid: Option<String>,
    pub recipient_number: Option<String>,
    pub recipient_name: Option<String>,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    pub last_message_timestamp: Option<i64>,
    pub unread_count: u32,
    pub is_archived: bool,
    pub is_muted: bool,
}

impl Conversation {
    pub fn new_direct(recipient_uuid: String, recipient_number: Option<String>, recipient_name: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_type: ConversationType::Direct,
            recipient_uuid: Some(recipient_uuid),
            recipient_number,
            recipient_name,
            group_id: None,
            group_name: None,
            last_message_timestamp: None,
            unread_count: 0,
            is_archived: false,
            is_muted: false,
        }
    }

    pub fn new_group(group_id: String, group_name: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_type: ConversationType::Group,
            recipient_uuid: None,
            recipient_number: None,
            recipient_name: None,
            group_id: Some(group_id),
            group_name,
            last_message_timestamp: None,
            unread_count: 0,
            is_archived: false,
            is_muted: false,
        }
    }

    pub fn display_name(&self) -> String {
        match self.conversation_type {
            ConversationType::Direct => {
                self.recipient_name.clone()
                    .or_else(|| self.recipient_number.clone())
                    .or_else(|| self.recipient_uuid.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            }
            ConversationType::Group => {
                self.group_name.clone()
                    .unwrap_or_else(|| "Unknown Group".to_string())
            }
        }
    }

    pub fn identifier(&self) -> String {
        match self.conversation_type {
            ConversationType::Direct => {
                self.recipient_uuid.clone()
                    .or_else(|| self.recipient_number.clone())
                    .unwrap_or_default()
            }
            ConversationType::Group => {
                self.group_id.clone().unwrap_or_default()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text { body: String },
    Attachment { attachments: Vec<AttachmentInfo> },
    Sticker { pack_id: String, sticker_id: i32 },
    RemoteDeleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub id: Option<String>,
    pub content_type: Option<String>,
    pub filename: Option<String>,
    pub size: Option<u64>,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub author_uuid: String,
    pub timestamp: i64,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub sender_uuid: String,
    pub sender_name: Option<String>,
    pub timestamp: i64,
    pub server_timestamp: Option<i64>,
    pub received_at: i64,
    pub content: MessageContent,
    pub quote: Option<Quote>,
    pub is_outgoing: bool,
    pub is_read: bool,
    pub is_deleted: bool,
}

impl Message {
    pub fn signal_id(&self) -> (String, i64) {
        (self.sender_uuid.clone(), self.timestamp)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub id: String,
    pub message_id: String,
    pub sender_uuid: String,
    pub emoji: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryState {
    Sending,
    Sent,
    Delivered,
    Read,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryStatus {
    pub message_id: String,
    pub recipient_uuid: String,
    pub state: DeliveryState,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub group_id: String,
    pub member_uuid: String,
    pub member_name: Option<String>,
    pub role: Option<String>,
}
