use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub number: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub device_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub number: Option<String>,
    pub uuid: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub profile_name: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub blocked: bool,
}

impl Contact {
    pub fn display_name(&self) -> String {
        self.profile_name
            .clone()
            .or_else(|| self.name.clone())
            .or_else(|| self.number.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn identifier(&self) -> Option<String> {
        self.uuid.clone().or_else(|| self.number.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub is_blocked: bool,
    #[serde(default)]
    pub is_member: bool,
}

impl Group {
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingMessage {
    pub envelope: Envelope,
    #[serde(default)]
    pub account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub source: Option<String>,
    pub source_uuid: Option<String>,
    pub source_name: Option<String>,
    #[serde(default)]
    pub source_device: Option<i32>,
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub data_message: Option<DataMessage>,
    #[serde(default)]
    pub sync_message: Option<SyncMessage>,
    #[serde(default)]
    pub receipt_message: Option<ReceiptMessage>,
    #[serde(default)]
    pub typing_message: Option<TypingMessage>,
    #[serde(default)]
    pub edit_message: Option<EditMessage>,
}

impl Envelope {
    pub fn sender_display(&self) -> String {
        self.source_name
            .clone()
            .or_else(|| self.source.clone())
            .or_else(|| self.source_uuid.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataMessage {
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub expires_in_seconds: Option<i32>,
    #[serde(default)]
    pub group_info: Option<GroupInfo>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub quote: Option<Quote>,
    #[serde(default)]
    pub reaction: Option<Reaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditMessage {
    pub target_sent_timestamp: i64,
    #[serde(default)]
    pub data_message: Option<DataMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    pub group_id: String,
    #[serde(rename = "type")]
    pub group_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub content_type: Option<String>,
    pub filename: Option<String>,
    pub id: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub id: Option<i64>,
    pub author: Option<String>,
    pub author_uuid: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
    pub emoji: String,
    pub target_author: Option<String>,
    pub target_author_uuid: Option<String>,
    pub target_sent_timestamp: Option<i64>,
    #[serde(default)]
    pub is_remove: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMessage {
    #[serde(default)]
    pub sent_message: Option<SentMessage>,
    #[serde(default)]
    pub read_messages: Vec<ReadMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentMessage {
    pub destination: Option<String>,
    pub destination_uuid: Option<String>,
    pub timestamp: Option<i64>,
    pub message: Option<String>,
    #[serde(default)]
    pub group_info: Option<GroupInfo>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub edit_message: Option<EditMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadMessage {
    pub sender: Option<String>,
    pub sender_uuid: Option<String>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptMessage {
    #[serde(rename = "type")]
    pub receipt_type: Option<String>,
    #[serde(default)]
    pub timestamps: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingMessage {
    pub action: Option<String>,
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    #[serde(default)]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub results: Vec<SendResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResultItem {
    pub recipient_address: Option<RecipientAddress>,
    #[serde(rename = "type")]
    pub result_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipientAddress {
    pub uuid: Option<String>,
    pub number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub number: Option<String>,
    pub uuid: Option<String>,
    pub fingerprint: Option<String>,
    pub safety_number: Option<String>,
    pub trust_level: Option<String>,
    pub added_date: Option<i64>,
}
