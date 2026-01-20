mod conversations;
mod file_browser;
mod input;
mod messages;

use crate::app::{App, Focus};
use crate::avatar::AvatarManager;
use crate::image_cache::ImageCache;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &mut App, avatar_manager: &mut Option<AvatarManager>, image_cache: &mut Option<ImageCache>) {
    let has_status = app.status_message.is_some();
    let [main_area, status_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(if has_status { 1 } else { 0 }),
    ])
    .areas(frame.area());

    let [left, right] = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(75),
    ])
    .areas(main_area);

    let input_width = right.width.saturating_sub(2) as usize;
    let input_lines = if input_width > 0 {
        ((app.input.text.len() + input_width) / input_width).max(1)
    } else {
        1
    };
    let input_height = (input_lines as u16 + 2).min(right.height / 2);

    let [messages_area, input_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(input_height),
    ])
    .areas(right);

    conversations::render(frame, left, app, app.focus == Focus::Conversations, avatar_manager);
    if app.focus == Focus::FileBrowser {
        file_browser::render(frame, messages_area, app);
    } else {
        messages::render(frame, messages_area, app, app.focus == Focus::Messages, image_cache);
    }
    input::render(frame, input_area, app, app.focus == Focus::Input || app.focus == Focus::FileBrowser);

    if let Some(ref msg) = app.status_message {
        let status = Paragraph::new(Span::styled(msg, Style::default().fg(Color::Yellow)));
        frame.render_widget(status, status_area);
    }
}
