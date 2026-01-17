use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use std::collections::HashMap;
use std::path::PathBuf;

struct CachedImage {
    protocol: StatefulProtocol,
    width: u32,
    height: u32,
}

pub struct ImageCache {
    picker: Picker,
    cache: HashMap<String, Option<CachedImage>>,
    attachments_dir: PathBuf,
}

const MAX_IMAGE_WIDTH: u16 = 60;
const MAX_IMAGE_HEIGHT: u16 = 20;
const MIN_IMAGE_HEIGHT: u16 = 4;

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

    pub fn get_image_with_size(&mut self, path: &str, max_width: u16) -> Option<(&mut StatefulProtocol, u16, u16)> {
        if !self.cache.contains_key(path) {
            let cached = self.load_image(path);
            self.cache.insert(path.to_string(), cached);
        }
        
        self.cache.get_mut(path).and_then(|opt| {
            opt.as_mut().map(|cached| {
                let (w, h) = calculate_display_size(cached.width, cached.height, max_width);
                (&mut cached.protocol, w, h)
            })
        })
    }

    pub fn get_image(&mut self, path: &str) -> Option<&mut StatefulProtocol> {
        self.get_image_with_size(path, MAX_IMAGE_WIDTH).map(|(p, _, _)| p)
    }

    pub fn get_image_height(&mut self, path: &str, max_width: u16) -> u16 {
        if !self.cache.contains_key(path) {
            let cached = self.load_image(path);
            self.cache.insert(path.to_string(), cached);
        }
        
        self.cache.get(path)
            .and_then(|opt| opt.as_ref())
            .map(|cached| {
                let (_, h) = calculate_display_size(cached.width, cached.height, max_width);
                h
            })
            .unwrap_or(MIN_IMAGE_HEIGHT)
    }

    pub fn resolve_attachment_path(&self, attachment_id: &str) -> PathBuf {
        self.attachments_dir.join(attachment_id)
    }

    fn load_image(&mut self, path: &str) -> Option<CachedImage> {
        let full_path = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            self.attachments_dir.join(path)
        };

        if !full_path.exists() {
            return None;
        }

        let data = std::fs::read(&full_path).ok()?;
        let img = image::load_from_memory(&data).ok()?;
        let width = img.width();
        let height = img.height();
        let protocol = self.picker.new_resize_protocol(img);
        
        Some(CachedImage { protocol, width, height })
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn resize(&self) -> Resize {
        Resize::Fit(None)
    }

    pub fn is_image(content_type: Option<&str>) -> bool {
        content_type.map_or(false, |ct| ct.starts_with("image/"))
    }
}

fn calculate_display_size(img_width: u32, img_height: u32, max_width: u16) -> (u16, u16) {
    if img_width == 0 || img_height == 0 {
        return (max_width, MIN_IMAGE_HEIGHT);
    }
    
    let aspect_ratio = img_width as f64 / img_height as f64;
    
    let cell_aspect = 2.0; // approximate height/width ratio of terminal cells
    let adjusted_ratio = aspect_ratio * cell_aspect;
    
    let display_width = max_width.min(MAX_IMAGE_WIDTH);
    let display_height = (display_width as f64 / adjusted_ratio) as u16;
    
    let display_height = display_height.clamp(MIN_IMAGE_HEIGHT, MAX_IMAGE_HEIGHT);
    
    let display_width = if display_height == MAX_IMAGE_HEIGHT || display_height == MIN_IMAGE_HEIGHT {
        ((display_height as f64 * adjusted_ratio) as u16).min(display_width)
    } else {
        display_width
    };
    
    (display_width, display_height)
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
