use ratatui::layout::Rect;
use ratatui_image::Resize;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

struct CachedImage {
    protocol: Protocol,
    render_width: u16,
    render_height: u16,
}

struct ProcessedImage {
    path: String,
    protocol: Protocol,
    render_width: u16,
    render_height: u16,
}

enum CacheEntry {
    Loading,
    Loaded(CachedImage),
    Failed,
}

pub struct ImageCache {
    cache: HashMap<String, CacheEntry>,
    load_sender: Sender<(String, u16)>,
    result_receiver: Receiver<Option<ProcessedImage>>,
}

const MAX_IMAGE_WIDTH: u16 = 60;
const MAX_IMAGE_HEIGHT: u16 = 20;
const MIN_IMAGE_HEIGHT: u16 = 4;

impl ImageCache {
    pub fn new() -> Option<Self> {
        let picker = Picker::from_query_stdio().ok()?;
        let attachments_dir = get_attachments_dir()?;

        let (load_sender, load_receiver) = mpsc::channel::<(String, u16)>();
        let (result_sender, result_receiver) = mpsc::channel::<Option<ProcessedImage>>();

        thread::spawn(move || {
            let picker = picker;
            while let Ok((path, max_width)) = load_receiver.recv() {
                let full_path = if path.starts_with('/') {
                    PathBuf::from(&path)
                } else {
                    attachments_dir.join(&path)
                };

                let result = (|| {
                    let data = std::fs::read(&full_path).ok()?;
                    let image = image::load_from_memory(&data).ok()?;

                    let (render_width, render_height) =
                        calculate_display_size(image.width(), image.height(), max_width);
                    let render_rect = Rect::new(0, 0, render_width, render_height);

                    let protocol = picker
                        .new_protocol(image, render_rect, Resize::Fit(None))
                        .ok()?;

                    Some(ProcessedImage {
                        path,
                        protocol,
                        render_width,
                        render_height,
                    })
                })();

                let _ = result_sender.send(result);
            }
        });

        Some(Self {
            cache: HashMap::new(),
            load_sender,
            result_receiver,
        })
    }

    pub fn process_next_loaded_image(&mut self) -> bool {
        if let Ok(result) = self.result_receiver.try_recv() {
            match result {
                Some(processed) => {
                    self.cache.insert(
                        processed.path,
                        CacheEntry::Loaded(CachedImage {
                            protocol: processed.protocol,
                            render_width: processed.render_width,
                            render_height: processed.render_height,
                        }),
                    );
                }
                None => {}
            }
            return true;
        }
        false
    }

    pub fn get_image_with_size(
        &mut self,
        path: &str,
        max_width: u16,
    ) -> Option<(&Protocol, u16, u16)> {
        if !self.cache.contains_key(path) {
            self.cache.insert(path.to_string(), CacheEntry::Loading);
            let _ = self.load_sender.send((path.to_string(), max_width));
            return None;
        }

        match self.cache.get(path) {
            Some(CacheEntry::Loaded(cached)) => {
                Some((&cached.protocol, cached.render_width, cached.render_height))
            }
            _ => None,
        }
    }

    pub fn get_image(&mut self, path: &str) -> Option<&Protocol> {
        self.get_image_with_size(path, MAX_IMAGE_WIDTH)
            .map(|(p, _, _)| p)
    }

    pub fn is_loading(&self, path: &str) -> bool {
        matches!(self.cache.get(path), Some(CacheEntry::Loading))
    }

    pub fn get_image_height(&self, path: &str) -> u16 {
        match self.cache.get(path) {
            Some(CacheEntry::Loaded(cached)) => cached.render_height,
            _ => MIN_IMAGE_HEIGHT,
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn is_image(content_type: Option<&str>) -> bool {
        content_type.is_some_and(|ct| ct.starts_with("image/"))
    }

    pub fn preload_images(&mut self, paths: &[String], max_width: u16) {
        for path in paths {
            if !self.cache.contains_key(path) {
                self.cache.insert(path.clone(), CacheEntry::Loading);
                let _ = self.load_sender.send((path.clone(), max_width));
            }
        }
    }
}

fn calculate_display_size(img_width: u32, img_height: u32, max_width: u16) -> (u16, u16) {
    if img_width == 0 || img_height == 0 {
        return (max_width, MIN_IMAGE_HEIGHT);
    }

    let aspect_ratio = img_width as f64 / img_height as f64;

    let cell_aspect = 2.0;
    let adjusted_ratio = aspect_ratio * cell_aspect;

    let display_width = max_width.min(MAX_IMAGE_WIDTH);
    let display_height = (display_width as f64 / adjusted_ratio) as u16;

    let display_height = display_height.clamp(MIN_IMAGE_HEIGHT, MAX_IMAGE_HEIGHT);

    let display_width = if display_height == MAX_IMAGE_HEIGHT || display_height == MIN_IMAGE_HEIGHT
    {
        ((display_height as f64 * adjusted_ratio) as u16).min(display_width)
    } else {
        display_width
    };

    (display_width, display_height)
}

fn get_attachments_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".local/share/signal-cli/attachments");
    if dir.exists() { Some(dir) } else { None }
}
