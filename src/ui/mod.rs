mod conversations;
mod input;
mod messages;

use crate::app::{App, Focus};
use crate::avatar::AvatarManager;
use crate::image_cache::ImageCache;
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &mut App, avatar_manager: &mut Option<AvatarManager>, image_cache: &mut Option<ImageCache>) {
    let [left, right] = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(75),
    ])
    .areas(frame.area());

    let [messages_area, input_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(3),
    ])
    .areas(right);

    conversations::render(frame, left, app, app.focus == Focus::Conversations, avatar_manager);
    messages::render(frame, messages_area, app, app.focus == Focus::Messages, image_cache);
    input::render(frame, input_area, app, app.focus == Focus::Input);
}
