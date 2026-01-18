use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct AvatarManager {
    picker: Picker,
    avatars_dir: PathBuf,
    cache: HashMap<String, Option<StatefulProtocol>>,
}

impl AvatarManager {
    pub fn new() -> Option<Self> {
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let avatars_dir = dirs_avatar_path()?;

        Some(Self {
            picker,
            avatars_dir,
            cache: HashMap::new(),
        })
    }

    pub fn get_avatar(&mut self, identifier: &str) -> Option<&mut StatefulProtocol> {
        if !self.cache.contains_key(identifier) {
            let protocol = self.load_avatar(identifier);
            self.cache.insert(identifier.to_string(), protocol);
        }

        self.cache.get_mut(identifier).and_then(|opt| opt.as_mut())
    }

    fn load_avatar(&mut self, identifier: &str) -> Option<StatefulProtocol> {
        let profile_path = self.avatars_dir.join(format!("profile-{}", identifier));
        let contact_path = self.avatars_dir.join(format!("contact-{}", identifier));

        let avatar_path = if profile_path.exists() {
            profile_path
        } else if contact_path.exists() {
            contact_path
        } else {
            return None;
        };

        let data = std::fs::read(&avatar_path).ok()?;
        image::load_from_memory(&data)
            .ok()
            .map(|img| self.picker.new_resize_protocol(img))
    }

    pub fn get_conversation_avatar(
        &mut self,
        recipient_uuid: Option<&str>,
        recipient_number: Option<&str>,
    ) -> Option<&mut StatefulProtocol> {
        if let Some(number) = recipient_number
            && self.has_avatar_file(number)
        {
            return self.get_avatar(number);
        }

        if let Some(uuid) = recipient_uuid
            && self.has_avatar_file(uuid)
        {
            return self.get_avatar(uuid);
        }

        None
    }

    fn has_avatar_file(&self, identifier: &str) -> bool {
        let profile = self.avatars_dir.join(format!("profile-{}", identifier));
        let contact = self.avatars_dir.join(format!("contact-{}", identifier));
        profile.exists() || contact.exists()
    }

}

fn dirs_avatar_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let avatars_dir = PathBuf::from(home).join(".local/share/signal-cli/avatars");

    if avatars_dir.exists() {
        Some(avatars_dir)
    } else {
        None
    }
}
