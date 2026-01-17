use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// Loaded image data ready to create protocol
struct LoadedImageData {
    path: String,
    img: image::DynamicImage,
    width: u32,
    height: u32,
}

struct CachedImage {
    protocol: StatefulProtocol,
    width: u32,
    height: u32,
}

/// State of an image in the cache
enum ImageState {
    /// Image is being loaded in background
    Loading,
    /// Image loaded successfully
    Loaded(CachedImage),
    /// Image failed to load
    Failed,
}

pub struct ImageCache {
    picker: Picker,
    cache: HashMap<String, ImageState>,
    attachments_dir: PathBuf,
    /// Channel to receive loaded images from background threads
    load_receiver: Receiver<Option<LoadedImageData>>,
    /// Channel to send load requests to background
    load_sender: Sender<(String, PathBuf)>,
}

const MAX_IMAGE_WIDTH: u16 = 60;
const MAX_IMAGE_HEIGHT: u16 = 20;
const MIN_IMAGE_HEIGHT: u16 = 4;

impl ImageCache {
    pub fn new() -> Option<Self> {
        let picker = Picker::from_query_stdio().ok()?;
        let attachments_dir = get_attachments_dir()?;

        // Create channels for async image loading
        let (load_sender, thread_receiver) = mpsc::channel::<(String, PathBuf)>();
        let (thread_sender, load_receiver) = mpsc::channel::<Option<LoadedImageData>>();

        // Spawn background thread for image loading
        thread::spawn(move || {
            while let Ok((path, full_path)) = thread_receiver.recv() {
                let result = load_image_data(&path, &full_path);
                let _ = thread_sender.send(result);
            }
        });

        Some(Self {
            picker,
            cache: HashMap::new(),
            attachments_dir,
            load_receiver,
            load_sender,
        })
    }

    /// Process any completed image loads from background thread
    pub fn process_pending(&mut self) -> bool {
        let mut any_loaded = false;
        while let Ok(result) = self.load_receiver.try_recv() {
            if let Some(data) = result {
                let protocol = self.picker.new_resize_protocol(data.img);
                self.cache.insert(data.path, ImageState::Loaded(CachedImage {
                    protocol,
                    width: data.width,
                    height: data.height,
                }));
                any_loaded = true;
            }
        }
        any_loaded
    }

    pub fn get_image_with_size(&mut self, path: &str, max_width: u16) -> Option<(&mut StatefulProtocol, u16, u16)> {
        // First process any pending loads
        self.process_pending();
        
        // Start loading if not in cache
        if !self.cache.contains_key(path) {
            self.start_loading(path);
            return None; // Return None while loading
        }
        
        // Check if loaded
        match self.cache.get_mut(path) {
            Some(ImageState::Loaded(cached)) => {
                let (w, h) = calculate_display_size(cached.width, cached.height, max_width);
                Some((&mut cached.protocol, w, h))
            }
            _ => None, // Still loading or failed
        }
    }

    fn start_loading(&mut self, path: &str) {
        let full_path = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            self.attachments_dir.join(path)
        };

        if !full_path.exists() {
            self.cache.insert(path.to_string(), ImageState::Failed);
            return;
        }

        // Mark as loading and send to background thread
        self.cache.insert(path.to_string(), ImageState::Loading);
        let _ = self.load_sender.send((path.to_string(), full_path));
    }

    pub fn get_image(&mut self, path: &str) -> Option<&mut StatefulProtocol> {
        self.get_image_with_size(path, MAX_IMAGE_WIDTH).map(|(p, _, _)| p)
    }

    /// Check if image is still loading (for showing placeholder)
    pub fn is_loading(&self, path: &str) -> bool {
        matches!(self.cache.get(path), Some(ImageState::Loading) | None)
    }

    pub fn get_image_height(&mut self, path: &str, max_width: u16) -> u16 {
        self.process_pending();
        
        if !self.cache.contains_key(path) {
            self.start_loading(path);
            return MIN_IMAGE_HEIGHT; // Return default while loading
        }
        
        match self.cache.get(path) {
            Some(ImageState::Loaded(cached)) => {
                let (_, h) = calculate_display_size(cached.width, cached.height, max_width);
                h
            }
            _ => MIN_IMAGE_HEIGHT,
        }
    }

    pub fn resolve_attachment_path(&self, attachment_id: &str) -> PathBuf {
        self.attachments_dir.join(attachment_id)
    }

    pub fn resize(&self) -> Resize {
        Resize::Fit(None)
    }

    pub fn is_image(content_type: Option<&str>) -> bool {
        content_type.map_or(false, |ct| ct.starts_with("image/"))
    }
}

/// Load image data in background thread
fn load_image_data(path: &str, full_path: &PathBuf) -> Option<LoadedImageData> {
    let data = std::fs::read(full_path).ok()?;
    let img = image::load_from_memory(&data).ok()?;
    let width = img.width();
    let height = img.height();
    
    Some(LoadedImageData {
        path: path.to_string(),
        img,
        width,
        height,
    })
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
