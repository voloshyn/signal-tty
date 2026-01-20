use crate::infrastructure::{IncomingMessage, SignalClient};
use crate::storage::{
    Conversation, ConversationType, Message, MessageContent, SqliteStorage, StorageRepository,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Conversations,
    ConversationFilter,
    Messages,
    Input,
    FileBrowser,
}

#[derive(Debug, Clone)]
pub enum SendTarget {
    Direct(String),
    Group(String),
}

#[derive(Debug, Default)]
pub struct InputState {
    pub text: String,
    pub cursor: usize,
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

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug)]
pub struct FileBrowserState {
    pub current_dir: PathBuf,
    pub entries: Vec<DirEntry>,
    pub selected: usize,
    pub marked: HashSet<usize>,
    pub show_hidden: bool,
}

impl Default for FileBrowserState {
    fn default() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut state = Self {
            current_dir,
            entries: Vec::new(),
            selected: 0,
            marked: HashSet::new(),
            show_hidden: false,
        };
        state.refresh();
        state
    }
}

impl FileBrowserState {
    pub fn refresh(&mut self) {
        self.entries.clear();
        self.selected = 0;
        self.marked.clear();

        let Ok(read_dir) = std::fs::read_dir(&self.current_dir) else {
            return;
        };

        let mut entries: Vec<DirEntry> = read_dir
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if !self.show_hidden && name.starts_with('.') {
                    return None;
                }
                let metadata = e.metadata().ok()?;
                Some(DirEntry {
                    name,
                    path: e.path(),
                    is_dir: metadata.is_dir(),
                    size: metadata.len(),
                })
            })
            .collect();

        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        self.entries = entries;
    }

    pub fn go_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.refresh();
        }
    }

    pub fn enter_selected(&mut self) -> Option<PathBuf> {
        let entry = self.entries.get(self.selected)?;
        if entry.is_dir {
            self.current_dir = entry.path.clone();
            self.refresh();
            None
        } else {
            Some(entry.path.clone())
        }
    }

    pub fn toggle_mark(&mut self) {
        if self.selected < self.entries.len() && !self.entries[self.selected].is_dir {
            if self.marked.contains(&self.selected) {
                self.marked.remove(&self.selected);
            } else {
                self.marked.insert(self.selected);
            }
        }
    }

    pub fn get_marked_or_selected(&self) -> Vec<PathBuf> {
        if self.marked.is_empty() {
            if let Some(entry) = self.entries.get(self.selected) {
                if !entry.is_dir {
                    return vec![entry.path.clone()];
                }
            }
            Vec::new()
        } else {
            self.marked
                .iter()
                .filter_map(|&idx| self.entries.get(idx))
                .map(|e| e.path.clone())
                .collect()
        }
    }

    pub fn go_home(&mut self) {
        if let Ok(home) = std::env::var("HOME") {
            self.current_dir = PathBuf::from(home);
            self.refresh();
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh();
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len();
        if delta < 0 {
            self.selected = self.selected.saturating_sub((-delta) as usize);
        } else {
            self.selected = (self.selected + delta as usize).min(len - 1);
        }
    }

    pub fn go_top(&mut self) {
        self.selected = 0;
    }

    pub fn go_bottom(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageSelection {
    pub anchor: usize,
    pub cursor: usize,
}

impl MessageSelection {
    pub fn range(&self) -> std::ops::RangeInclusive<usize> {
        self.anchor.min(self.cursor)..=self.anchor.max(self.cursor)
    }
}

pub const SCROLL_LINES: usize = 3;

#[derive(Debug)]
pub struct ConversationView {
    pub conversation: Conversation,
    pub messages: Option<Vec<Message>>,
    pub scroll_offset: usize,
    pub has_more_messages: bool,
    pub selection: Option<MessageSelection>,
    pub visible_range: Option<(usize, usize)>,
    pub last_message_preview: Option<Message>,
}

impl ConversationView {
    pub fn new(conversation: Conversation, storage: &SqliteStorage) -> Self {
        let last_message_preview = storage
            .list_messages(&conversation.id, 1, None)
            .ok()
            .and_then(|msgs| msgs.into_iter().next());
        Self {
            conversation,
            messages: None,
            scroll_offset: 0,
            has_more_messages: true,
            selection: None,
            visible_range: None,
            last_message_preview,
        }
    }

    pub fn load_messages(&mut self, storage: &SqliteStorage) -> bool {
        if self.messages.is_none()
            && let Ok(msgs) = storage.list_messages(&self.conversation.id, 100, None)
        {
            self.has_more_messages = msgs.len() >= 100;
            self.messages = Some(msgs);
            self.scroll_to_bottom();
            return true;
        }
        false
    }

    pub fn collect_image_paths(&self) -> Vec<String> {
        let Some(ref msgs) = self.messages else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        for msg in msgs.iter().rev() {
            if let MessageContent::Attachment { attachments } = &msg.content {
                for att in attachments {
                    if att
                        .content_type
                        .as_ref()
                        .is_some_and(|ct| ct.starts_with("image/"))
                        && let Some(path) = &att.local_path
                    {
                        paths.push(path.clone());
                    }
                }
            }
        }
        paths
    }

    pub fn load_older_messages(&mut self, storage: &SqliteStorage) -> Vec<String> {
        if !self.has_more_messages {
            return Vec::new();
        }

        let oldest_timestamp = self
            .messages
            .as_ref()
            .and_then(|msgs| msgs.first())
            .map(|m| m.timestamp);

        if let Some(before_ts) = oldest_timestamp
            && let Ok(older_msgs) =
                storage.list_messages(&self.conversation.id, 100, Some(before_ts))
        {
            if older_msgs.is_empty() {
                self.has_more_messages = false;
                return Vec::new();
            }

            self.has_more_messages = older_msgs.len() >= 100;

            // Collect image paths from newly loaded messages
            let mut paths = Vec::new();
            for msg in &older_msgs {
                if let MessageContent::Attachment { attachments } = &msg.content {
                    for att in attachments {
                        if att
                            .content_type
                            .as_ref()
                            .is_some_and(|ct| ct.starts_with("image/"))
                            && let Some(path) = &att.local_path
                        {
                            paths.push(path.clone());
                        }
                    }
                }
            }

            if let Some(ref mut msgs) = self.messages {
                // Prepend older messages
                let mut new_msgs = older_msgs;
                new_msgs.append(msgs);
                *msgs = new_msgs;

                // No scroll_offset adjustment needed - it's line-based from bottom
                return paths;
            }
        }
        Vec::new()
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn unread_incoming_timestamps(&self) -> Vec<i64> {
        self.messages.as_ref().map_or(Vec::new(), |msgs| {
            msgs.iter()
                .filter(|m| !m.is_read && !m.is_outgoing)
                .map(|m| m.timestamp)
                .collect()
        })
    }

    pub fn add_message(&mut self, message: Message) {
        self.last_message_preview = Some(message.clone());
        if let Some(ref mut msgs) = self.messages {
            msgs.push(message);
            self.scroll_offset = 0;
        }
    }

    pub fn enter_selection_mode(&mut self) {
        if let Some(ref msgs) = self.messages {
            if !msgs.is_empty() {
                let center_idx = if let Some((start, end)) = self.visible_range {
                    (start + end) / 2
                } else {
                    msgs.len() - 1
                };
                self.selection = Some(MessageSelection {
                    anchor: center_idx,
                    cursor: center_idx,
                });
            }
        }
    }

    pub fn exit_selection_mode(&mut self) {
        self.selection = None;
    }

    pub fn move_selection(&mut self, direction: i32, extend: bool) {
        let msg_count = self.messages.as_ref().map(|m| m.len()).unwrap_or(0);
        if msg_count == 0 {
            return;
        }

        let Some(ref mut sel) = self.selection else {
            return;
        };

        let new_cursor = if direction < 0 {
            sel.cursor.saturating_sub(1)
        } else {
            (sel.cursor + 1).min(msg_count - 1)
        };

        sel.cursor = new_cursor;
        if !extend {
            sel.anchor = new_cursor;
        }
    }

    pub fn shrink_selection(&mut self) {
        let Some(ref mut sel) = self.selection else {
            return;
        };

        if sel.cursor > sel.anchor {
            sel.cursor -= 1;
        } else if sel.cursor < sel.anchor {
            sel.cursor += 1;
        }
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let msgs = self.messages.as_ref()?;
        let sel = self.selection.as_ref()?;
        let range = sel.range();

        let mut lines = Vec::new();
        for idx in range {
            if let Some(msg) = msgs.get(idx) {
                let text = match &msg.content {
                    MessageContent::Text { body } => body.clone(),
                    MessageContent::Attachment { attachments } => {
                        attachments
                            .iter()
                            .map(|a| {
                                a.filename
                                    .as_deref()
                                    .unwrap_or("[attachment]")
                                    .to_string()
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                    MessageContent::Sticker { .. } => "[Sticker]".to_string(),
                    MessageContent::RemoteDeleted => "[Deleted]".to_string(),
                };
                lines.push(text);
            }
        }
        Some(lines.join("\n"))
    }

    pub fn delete_selected_messages(&mut self) -> Vec<String> {
        let Some(ref sel) = self.selection else {
            return Vec::new();
        };
        let Some(ref mut msgs) = self.messages else {
            return Vec::new();
        };

        let range = sel.range();
        let ids: Vec<String> = range
            .clone()
            .filter_map(|idx| msgs.get(idx).map(|m| m.id.clone()))
            .collect();

        let indices_to_remove: Vec<usize> = range.collect();
        for idx in indices_to_remove.into_iter().rev() {
            if idx < msgs.len() {
                msgs.remove(idx);
            }
        }

        self.selection = None;
        ids
    }

    pub fn get_selected_outgoing_timestamps(&self) -> Vec<i64> {
        let Some(ref sel) = self.selection else {
            return Vec::new();
        };
        let Some(ref msgs) = self.messages else {
            return Vec::new();
        };

        sel.range()
            .filter_map(|idx| {
                msgs.get(idx)
                    .filter(|m| m.is_outgoing)
                    .map(|m| m.timestamp)
            })
            .collect()
    }

    pub fn remote_delete_target(&self) -> Option<RemoteDeleteTarget> {
        match self.conversation.conversation_type {
            ConversationType::Direct => {
                let recipient = self
                    .conversation
                    .recipient_uuid
                    .clone()
                    .or_else(|| self.conversation.recipient_number.clone())?;
                Some(RemoteDeleteTarget::Direct(recipient))
            }
            ConversationType::Group => {
                let group_id = self.conversation.group_id.clone()?;
                Some(RemoteDeleteTarget::Group(group_id))
            }
        }
    }

    pub fn get_selected_attachment_paths(&self) -> Vec<String> {
        let Some(ref sel) = self.selection else {
            return Vec::new();
        };
        let Some(ref msgs) = self.messages else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        for idx in sel.range() {
            if let Some(msg) = msgs.get(idx) {
                if let MessageContent::Attachment { attachments } = &msg.content {
                    for att in attachments {
                        if let Some(path) = &att.local_path {
                            paths.push(path.clone());
                        }
                    }
                }
            }
        }
        paths
    }
}

#[derive(Debug, Clone)]
pub enum RemoteDeleteTarget {
    Direct(String),
    Group(String),
}

#[derive(Debug, Clone)]
pub struct PendingRemoteDelete {
    pub target: RemoteDeleteTarget,
    pub timestamps: Vec<i64>,
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
    pub filter_input: InputState,
    pub file_browser: FileBrowserState,
    pub pending_attachments: Vec<PathBuf>,

    pub should_quit: bool,
    pub status_message: Option<String>,
    pub pending_send: Option<String>,
    pub pending_remote_deletes: Vec<PendingRemoteDelete>,
    pub messages_height: usize,
    pub needs_image_preload: bool,
    pub pending_preload_paths: Vec<String>,
    pub show_empty_conversations: bool,
}

impl App {
    pub fn new(
        storage: Arc<SqliteStorage>,
        signal: SignalClient,
        my_number: Option<String>,
    ) -> Self {
        Self {
            storage,
            signal,
            my_uuid: None,
            my_number,
            conversations: Vec::new(),
            selected: 0,
            focus: Focus::Conversations,
            input: InputState::default(),
            filter_input: InputState::default(),
            file_browser: FileBrowserState::default(),
            pending_attachments: Vec::new(),
            should_quit: false,
            status_message: None,
            pending_send: None,
            pending_remote_deletes: Vec::new(),
            messages_height: 20,
            needs_image_preload: false,
            pending_preload_paths: Vec::new(),
            show_empty_conversations: false,
        }
    }

    pub fn load_conversations(&mut self) {
        if let Ok(convs) = self.storage.list_conversations() {
            self.conversations = convs
                .into_iter()
                .map(|c| ConversationView::new(c, &self.storage))
                .collect();
            let first_with_messages = self
                .conversations
                .iter()
                .position(|c| c.conversation.last_message_timestamp.is_some());
            if let Some(idx) = first_with_messages {
                self.selected = idx;
                if self.conversations[idx].load_messages(&self.storage) {
                    self.needs_image_preload = true;
                }
            }
        }
    }

    pub fn take_preload_paths(&mut self) -> Vec<String> {
        let mut paths = std::mem::take(&mut self.pending_preload_paths);

        if self.needs_image_preload {
            self.needs_image_preload = false;
            if let Some(conv_paths) = self
                .selected_conversation()
                .map(|c| c.collect_image_paths())
            {
                paths.extend(conv_paths);
            }
        }

        paths
    }

    pub fn selected_conversation(&self) -> Option<&ConversationView> {
        self.conversations.get(self.selected)
    }

    pub fn selected_conversation_mut(&mut self) -> Option<&mut ConversationView> {
        self.conversations.get_mut(self.selected)
    }

    pub fn select_next(&mut self) {
        let indices = self.filtered_conversation_indices();
        if indices.is_empty() {
            return;
        }
        let current_pos = indices.iter().position(|&i| i == self.selected);
        let new_pos = match current_pos {
            Some(pos) => (pos + 1) % indices.len(),
            None => 0,
        };
        if let Some(&new_idx) = indices.get(new_pos) {
            self.selected = new_idx;
            if self.conversations[self.selected].load_messages(&self.storage) {
                self.needs_image_preload = true;
            }
        }
    }

    pub fn select_prev(&mut self) {
        let indices = self.filtered_conversation_indices();
        if indices.is_empty() {
            return;
        }
        let current_pos = indices.iter().position(|&i| i == self.selected);
        let new_pos = match current_pos {
            Some(pos) => pos.checked_sub(1).unwrap_or(indices.len() - 1),
            None => 0,
        };
        if let Some(&new_idx) = indices.get(new_pos) {
            self.selected = new_idx;
            if self.conversations[self.selected].load_messages(&self.storage) {
                self.needs_image_preload = true;
            }
        }
    }

    pub fn scroll_messages_up(&mut self) {
        let storage = self.storage.clone();

        if let Some(conv) = self.selected_conversation_mut() {
            // Scroll up by fixed lines
            conv.scroll_offset += SCROLL_LINES;

            // Check if we need to load more messages
            if conv.has_more_messages {
                let paths = conv.load_older_messages(&storage);
                if !paths.is_empty() {
                    self.pending_preload_paths.extend(paths);
                }
            }
        }
    }

    pub fn scroll_messages_down(&mut self) {
        if let Some(conv) = self.selected_conversation_mut() {
            // Scroll down by fixed lines, but don't go below 0
            conv.scroll_offset = conv.scroll_offset.saturating_sub(SCROLL_LINES);
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Conversations | Focus::ConversationFilter => Focus::Messages,
            Focus::Messages => Focus::Input,
            Focus::Input | Focus::FileBrowser => Focus::Messages,
        };
        self.filter_input.clear();
    }

    #[allow(dead_code)]
    pub fn focus_input(&mut self) {
        self.focus = Focus::Input;
    }

    pub fn queue_send_message(&mut self, text: String) {
        if (!text.is_empty() || !self.pending_attachments.is_empty())
            && self.selected_conversation().is_some()
        {
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

    pub fn mark_current_conversation_read(&mut self) -> Option<(String, Vec<i64>)> {
        let conv = self.selected_conversation()?;
        if conv.conversation.conversation_type != ConversationType::Direct {
            return None;
        }

        let timestamps = conv.unread_incoming_timestamps();
        if timestamps.is_empty() {
            return None;
        }

        let conversation_id = conv.conversation.id.clone();
        let recipient = conv
            .conversation
            .recipient_uuid
            .clone()
            .or_else(|| conv.conversation.recipient_number.clone())?;

        let max_timestamp = *timestamps.iter().max()?;
        let _ = self
            .storage
            .mark_messages_read(&conversation_id, max_timestamp);

        if let Some(conv) = self.selected_conversation_mut()
            && let Some(ref mut msgs) = conv.messages
        {
            for msg in msgs.iter_mut() {
                if !msg.is_outgoing && timestamps.contains(&msg.timestamp) {
                    msg.is_read = true;
                }
            }
        }

        Some((recipient, timestamps))
    }

    pub fn handle_incoming_message(&mut self, msg: IncomingMessage) {
        let envelope = &msg.envelope;
        let sender_uuid = match envelope.source_uuid.as_ref() {
            Some(uuid) => uuid,
            None => return,
        };
        let sender_name = envelope.source_name.clone();
        let timestamp = envelope.timestamp.unwrap_or_else(now_millis);

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
                let content = if !data.attachments.is_empty() {
                    let attachments = data
                        .attachments
                        .iter()
                        .map(|a| crate::storage::AttachmentInfo {
                            id: a.id.clone(),
                            content_type: a.content_type.clone(),
                            filename: a.filename.clone(),
                            size: a.size.map(|s| s as u64),
                            local_path: a.id.clone(),
                        })
                        .collect();
                    MessageContent::Attachment { attachments }
                } else {
                    MessageContent::Text { body: text }
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
                    is_edited: false,
                };

                let _ = self.storage.save_message(&message);
                self.add_message_to_conversation(&conv.id, message);
            }
        }

        if let Some(edit) = &envelope.edit_message {
            self.handle_edit_message(sender_uuid, edit);
        }

        if let Some(sync) = &envelope.sync_message
            && let Some(sent) = &sync.sent_message
        {
            if let Some(edit) = &sent.edit_message {
                self.handle_edit_message(sender_uuid, edit);
                return;
            }

            let text = sent.message.clone().unwrap_or_default();
            if text.is_empty() && sent.attachments.is_empty() {
                return;
            }

            let sync_timestamp = sent.timestamp.unwrap_or(timestamp);
            if self
                .storage
                .get_message_by_signal_id(sender_uuid, sync_timestamp)
                .ok()
                .flatten()
                .is_some()
            {
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
                let content = if !sent.attachments.is_empty() {
                    let attachments = sent
                        .attachments
                        .iter()
                        .map(|a| crate::storage::AttachmentInfo {
                            id: a.id.clone(),
                            content_type: a.content_type.clone(),
                            filename: a.filename.clone(),
                            size: a.size.map(|s| s as u64),
                            local_path: a.id.clone(),
                        })
                        .collect();
                    MessageContent::Attachment { attachments }
                } else {
                    MessageContent::Text { body: text }
                };

                let message = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    conversation_id: conv.id.clone(),
                    sender_uuid: sender_uuid.clone(),
                    sender_name: sender_name.clone(),
                    timestamp: sent.timestamp.unwrap_or(timestamp),
                    server_timestamp: None,
                    received_at: now_millis(),
                    content,
                    quote: None,
                    is_outgoing: true,
                    is_read: true,
                    is_deleted: false,
                    is_edited: false,
                };

                let _ = self.storage.save_message(&message);
                self.add_message_to_conversation(&conv.id, message);
            }
        }
    }

    fn handle_edit_message(
        &mut self,
        sender_uuid: &str,
        edit: &crate::infrastructure::EditMessage,
    ) {
        let target_timestamp = edit.target_sent_timestamp;

        let Some(data) = &edit.data_message else {
            return;
        };
        let new_text = data.message.clone().unwrap_or_default();
        if new_text.is_empty() {
            return;
        }

        let new_content = MessageContent::Text { body: new_text };
        let _ = self
            .storage
            .update_message_content(sender_uuid, target_timestamp, &new_content);
        let _ = self
            .storage
            .update_message_content("", target_timestamp, &new_content);

        for conv in &mut self.conversations {
            if let Some(ref mut msgs) = conv.messages {
                for msg in msgs.iter_mut() {
                    if msg.timestamp == target_timestamp
                        && (msg.sender_uuid == sender_uuid || msg.sender_uuid.is_empty())
                    {
                        msg.content = new_content.clone();
                        msg.is_edited = true;
                        return;
                    }
                }
            }
        }
    }

    pub fn add_message_to_conversation(&mut self, conversation_id: &str, message: Message) {
        let found_idx = self
            .conversations
            .iter()
            .position(|c| c.conversation.id == conversation_id);

        if let Some(idx) = found_idx {
            let timestamp = message.timestamp;
            let conv_view = &mut self.conversations[idx];
            if conv_view.messages.is_none() {
                conv_view.load_messages(&self.storage);
            }
            conv_view.add_message(message);
            conv_view.conversation.last_message_timestamp = Some(timestamp);

            self.sort_conversations();
            return;
        }

        self.load_conversations();
    }

    fn sort_conversations(&mut self) {
        let selected_id = self
            .conversations
            .get(self.selected)
            .map(|c| c.conversation.id.clone());

        self.conversations.sort_by(|a, b| {
            b.conversation
                .last_message_timestamp
                .cmp(&a.conversation.last_message_timestamp)
        });

        if let Some(id) = selected_id
            && let Some(new_idx) = self
                .conversations
                .iter()
                .position(|c| c.conversation.id == id)
        {
            self.selected = new_idx;
        }
    }

    pub fn filtered_conversation_indices(&self) -> Vec<usize> {
        let filter = self.filter_input.text.to_lowercase();
        if filter.is_empty() {
            return self
                .conversations
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    self.show_empty_conversations
                        || c.conversation.last_message_timestamp.is_some()
                })
                .map(|(i, _)| i)
                .collect();
        }

        self.conversations
            .iter()
            .enumerate()
            .filter(|(_, conv_view)| {
                let conv = &conv_view.conversation;

                let is_note_to_self = self.my_number.as_ref().is_some_and(|my_num| {
                    conv.recipient_number.as_ref() == Some(my_num)
                }) || self.my_uuid.as_ref().is_some_and(|my_uuid| {
                    conv.recipient_uuid.as_ref() == Some(my_uuid)
                });

                if is_note_to_self && "note to self".contains(&filter) {
                    return true;
                }

                let name = conv.display_name().to_lowercase();
                if name.contains(&filter) {
                    return true;
                }
                if let Some(ref number) = conv.recipient_number {
                    if number.to_lowercase().contains(&filter) {
                        return true;
                    }
                }
                if let Some(ref uuid) = conv.recipient_uuid {
                    if uuid.to_lowercase().contains(&filter) {
                        return true;
                    }
                }
                false
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn select_filtered(&mut self, direction: i32) {
        let indices = self.filtered_conversation_indices();
        if indices.is_empty() {
            return;
        }

        let current_pos = indices.iter().position(|&i| i == self.selected);
        let new_pos = match current_pos {
            Some(pos) => {
                if direction > 0 {
                    (pos + 1).min(indices.len() - 1)
                } else {
                    pos.saturating_sub(1)
                }
            }
            None => 0,
        };

        if let Some(&new_idx) = indices.get(new_pos) {
            self.selected = new_idx;
            if self.conversations[self.selected].load_messages(&self.storage) {
                self.needs_image_preload = true;
            }
        }
    }

    pub fn ensure_selection_matches_filter(&mut self) {
        let indices = self.filtered_conversation_indices();
        if indices.is_empty() || indices.contains(&self.selected) {
            return;
        }
        if let Some(&first) = indices.first() {
            self.selected = first;
            if self.conversations[self.selected].load_messages(&self.storage) {
                self.needs_image_preload = true;
            }
        }
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
