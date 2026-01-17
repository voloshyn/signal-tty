use crate::infrastructure::{IncomingMessage, SignalClient, SignalRepository};
use crate::storage::{
    Conversation, ConversationType, Message, MessageContent, SqliteStorage, StorageRepository,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Conversations,
    Messages,
    Input,
}

#[derive(Debug, Clone)]
pub enum SendTarget {
    Direct(String),
    Group(String),
}

#[derive(Debug)]
pub struct InputState {
    pub text: String,
    pub cursor: usize,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }
}

impl InputState {
    pub fn insert(&mut self, c: char) {
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn delete_back(&mut self) {
        if self.cursor > 0 {
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor = prev;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
        }
    }

    pub fn move_start(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    pub fn clear(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }
}

#[derive(Debug)]
pub struct ConversationView {
    pub conversation: Conversation,
    pub messages: Option<Vec<Message>>,
    pub scroll_offset: usize,
}

impl ConversationView {
    pub fn new(conversation: Conversation) -> Self {
        Self {
            conversation,
            messages: None,
            scroll_offset: 0,
        }
    }

    pub fn load_messages(&mut self, storage: &SqliteStorage) {
        if self.messages.is_none() {
            if let Ok(msgs) = storage.list_messages(&self.conversation.id, 100, None) {
                self.messages = Some(msgs);
                self.scroll_to_bottom();
            }
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        if let Some(ref msgs) = self.messages {
            self.scroll_offset = msgs.len().saturating_sub(1);
        }
    }

    pub fn add_message(&mut self, message: Message) {
        if let Some(ref mut msgs) = self.messages {
            msgs.push(message);
            self.scroll_to_bottom();
        }
    }
}

pub struct App {
    pub storage: Arc<SqliteStorage>,
    pub signal: SignalClient,
    pub my_uuid: Option<String>,
    pub my_number: Option<String>,

    pub conversations: Vec<ConversationView>,
    pub selected: usize,
    pub focus: Focus,
    pub input: InputState,

    pub should_quit: bool,
    pub status_message: Option<String>,
    pub pending_send: Option<String>,
}

impl App {
    pub fn new(storage: Arc<SqliteStorage>, signal: SignalClient, my_number: Option<String>) -> Self {
        Self {
            storage,
            signal,
            my_uuid: None,
            my_number,
            conversations: Vec::new(),
            selected: 0,
            focus: Focus::Conversations,
            input: InputState::default(),
            should_quit: false,
            status_message: None,
            pending_send: None,
        }
    }

    pub fn load_conversations(&mut self) {
        if let Ok(convs) = self.storage.list_conversations() {
            self.conversations = convs.into_iter().map(ConversationView::new).collect();
            if !self.conversations.is_empty() {
                self.selected = 0;
                self.conversations[0].load_messages(&self.storage);
            }
        }
    }

    pub fn selected_conversation(&self) -> Option<&ConversationView> {
        self.conversations.get(self.selected)
    }

    pub fn selected_conversation_mut(&mut self) -> Option<&mut ConversationView> {
        self.conversations.get_mut(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.conversations.is_empty() {
            self.selected = (self.selected + 1) % self.conversations.len();
            self.conversations[self.selected].load_messages(&self.storage);
        }
    }

    pub fn select_prev(&mut self) {
        if !self.conversations.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.conversations.len() - 1);
            self.conversations[self.selected].load_messages(&self.storage);
        }
    }

    pub fn scroll_messages_up(&mut self) {
        if let Some(conv) = self.selected_conversation_mut() {
            conv.scroll_offset = conv.scroll_offset.saturating_sub(1);
        }
    }

    pub fn scroll_messages_down(&mut self) {
        if let Some(conv) = self.selected_conversation_mut() {
            if let Some(ref msgs) = conv.messages {
                if conv.scroll_offset < msgs.len().saturating_sub(1) {
                    conv.scroll_offset += 1;
                }
            }
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Conversations => Focus::Messages,
            Focus::Messages => Focus::Input,
            Focus::Input => Focus::Conversations,
        };
    }

    pub fn focus_input(&mut self) {
        self.focus = Focus::Input;
    }

    pub fn queue_send_message(&mut self, text: String) {
        if !text.is_empty() && self.selected_conversation().is_some() {
            self.pending_send = Some(text);
        }
    }

    pub fn get_send_target(&self) -> Option<SendTarget> {
        let conv = self.selected_conversation()?;
        match conv.conversation.conversation_type {
            ConversationType::Direct => {
                let recipient = conv
                    .conversation
                    .recipient_uuid
                    .clone()
                    .or_else(|| conv.conversation.recipient_number.clone())?;
                Some(SendTarget::Direct(recipient))
            }
            ConversationType::Group => {
                let group_id = conv.conversation.group_id.clone()?;
                Some(SendTarget::Group(group_id))
            }
        }
    }

    pub fn handle_incoming_message(&mut self, msg: IncomingMessage) {
        let envelope = &msg.envelope;
        let sender_uuid = match envelope.source_uuid.as_ref() {
            Some(uuid) => uuid,
            None => return,
        };
        let sender_name = envelope.source_name.clone();
        let timestamp = envelope.timestamp.unwrap_or_else(|| now_millis());

        if let Some(data) = &envelope.data_message {
            let text = data.message.clone().unwrap_or_default();
            if text.is_empty() && data.attachments.is_empty() {
                return;
            }

            let group_info = data.group_info.as_ref();
            let is_outgoing = self
                .my_uuid
                .as_ref()
                .map(|u| u == sender_uuid)
                .unwrap_or(false);

            let conversation = if let Some(group) = group_info {
                self.storage
                    .get_or_create_group_conversation(&group.group_id, None)
                    .ok()
            } else {
                self.storage
                    .get_or_create_direct_conversation(
                        sender_uuid,
                        envelope.source.as_deref(),
                        sender_name.as_deref(),
                    )
                    .ok()
            };

            if let Some(conv) = conversation {
                let content = if !text.is_empty() {
                    MessageContent::Text { body: text }
                } else {
                    MessageContent::Text {
                        body: "[Attachment]".to_string(),
                    }
                };

                let message = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    conversation_id: conv.id.clone(),
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

                let _ = self.storage.save_message(&message);
                self.add_message_to_conversation(&conv.id, message);
            }
        }

        // Handle sync_message (our messages from other devices)
        if let Some(sync) = &envelope.sync_message {
            if let Some(sent) = &sync.sent_message {
                let text = sent.message.clone().unwrap_or_default();
                if text.is_empty() {
                    return;
                }

                let group_info = sent.group_info.as_ref();

                let conversation = if let Some(group) = group_info {
                    self.storage
                        .get_or_create_group_conversation(&group.group_id, None)
                        .ok()
                } else if let Some(dest) = &sent.destination_uuid {
                    self.storage
                        .get_or_create_direct_conversation(dest, sent.destination.as_deref(), None)
                        .ok()
                } else if let Some(dest) = &sent.destination {
                    self.storage
                        .get_or_create_direct_conversation(dest, Some(dest), None)
                        .ok()
                } else {
                    None
                };

                if let Some(conv) = conversation {
                    let message = Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        conversation_id: conv.id.clone(),
                        sender_uuid: sender_uuid.clone(),
                        sender_name: sender_name.clone(),
                        timestamp: sent.timestamp.unwrap_or(timestamp),
                        server_timestamp: None,
                        received_at: now_millis(),
                        content: MessageContent::Text { body: text },
                        quote: None,
                        is_outgoing: true,
                        is_read: true,
                        is_deleted: false,
                    };

                    let _ = self.storage.save_message(&message);
                    self.add_message_to_conversation(&conv.id, message);
                }
            }
        }
    }

    fn add_message_to_conversation(&mut self, conversation_id: &str, message: Message) {
        // Find existing conversation view
        if let Some(conv_view) = self
            .conversations
            .iter_mut()
            .find(|c| c.conversation.id == conversation_id)
        {
            conv_view.add_message(message);
            return;
        }

        // New conversation - reload list
        self.load_conversations();
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
