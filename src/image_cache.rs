use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ImageCache {
    picker: Picker,
    cache: HashMap<String, Option<StatefulProtocol>>,
    attachments_dir: PathBuf,
}

impl ImageCache {
    pub fn new() -> Option<Self> {
        let picker = Picker::from_query_stdio().ok()?;
        let attachments_dir = get_attachments_dir()?;

        Some(Self {
            picker,
            cache: HashMap::new(),
            attachments_dir,
        })
    }

    pub fn get_image(&mut self, path: &str) -> Option<&mut StatefulProtocol> {
        if !self.cache.contains_key(path) {
            let protocol = self.load_image(path);
            self.cache.insert(path.to_string(), protocol);
        }
        self.cache.get_mut(path).and_then(|opt| opt.as_mut())
    }

    pub fn resolve_attachment_path(&self, attachment_id: &str) -> PathBuf {
        self.attachments_dir.join(attachment_id)
    }

    fn load_image(&mut self, path: &str) -> Option<StatefulProtocol> {
        let full_path = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            self.attachments_dir.join(path)
        };

        if !full_path.exists() {
            return None;
        }

        let data = std::fs::read(&full_path).ok()?;
        image::load_from_memory(&data)
            .ok()
            .map(|img| self.picker.new_resize_protocol(img))
    }

    pub fn resize(&self) -> Resize {
        Resize::Fit(None)
    }

    pub fn is_image(content_type: Option<&str>) -> bool {
        content_type.map_or(false, |ct| ct.starts_with("image/"))
    }
}

fn get_attachments_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".local/share/signal-cli/attachments");
    if dir.exists() {
        Some(dir)
    } else {
        None
    }
}
