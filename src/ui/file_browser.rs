use crate::app::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{}B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1}K", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}M", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let fb = &app.file_browser;

    let path_display = fb.current_dir.to_string_lossy();
    let title = format!(" {} ", path_display);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if fb.entries.is_empty() {
        let empty = Paragraph::new("(empty directory)")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner_area);
        return;
    }

    let items: Vec<ListItem> = fb
        .entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let is_marked = fb.marked.contains(&idx);
            let is_selected = idx == fb.selected;

            let icon = if entry.is_dir { "/" } else { " " };
            let mark = if is_marked { "*" } else { " " };
            let size_str = if entry.is_dir {
                String::new()
            } else {
                format_size(entry.size)
            };

            let name_style = if entry.is_dir {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else if is_marked {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(mark, Style::default().fg(Color::Green)),
                Span::styled(icon, Style::default().fg(Color::Blue)),
                Span::styled(&entry.name, name_style),
                Span::raw(" "),
                Span::styled(size_str, Style::default().fg(Color::DarkGray)),
            ]);

            let mut item = ListItem::new(line);
            if is_selected {
                item = item.style(Style::default().bg(Color::DarkGray));
            }
            item
        })
        .collect();

    let list = List::new(items);
    let mut state = ListState::default();
    state.select(Some(fb.selected));

    frame.render_stateful_widget(list, inner_area, &mut state);
}
