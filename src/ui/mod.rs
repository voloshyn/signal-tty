mod conversations;
mod input;
mod messages;

use crate::app::{App, Focus};
use crate::avatar::AvatarManager;
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &App, avatar_manager: &mut Option<AvatarManager>) {
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
    messages::render(frame, messages_area, app, app.focus == Focus::Messages);
    input::render(frame, input_area, app, app.focus == Focus::Input);
}
