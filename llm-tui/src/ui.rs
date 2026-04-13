use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use crate::app::{App, Message, AppState};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_history(frame, app, chunks[0]);
    render_input(frame, app, chunks[1]);
    render_status_bar(frame, app, chunks[2]);
}

fn render_history(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" История генерации ")
        .borders(Borders::ALL);
    
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .rev()
        .skip(app.scroll_offset)
        .flat_map(|msg| match msg {
            Message::User(text) => {
                let line = Line::from(Span::styled(
                    format!("> {}", text),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                vec![line, Line::from("")]
            }
            Message::Assistant(code) => {
                let lines: Vec<Line> = code
                    .lines()
                    .map(|l| {
                        Line::from(Span::styled(
                            format!("  {}", l),
                            Style::default().fg(Color::Green),
                        ))
                    })
                    .collect();
                let mut result = lines;
                result.push(Line::from(""));
                result
            }
            Message::Error(err) => {
                let line = Line::from(Span::styled(
                    format!("Ошибка: {}", err),
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ));
                vec![line, Line::from("")]
            }
        })
        .map(ListItem::new)
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Ввод (Enter — отправить) ")
        .borders(Borders::ALL);

    let input = Paragraph::new(app.input.as_str())
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(input, area);
    
    // Курсор
    let x = area.x + app.input.len() as u16 + 1;
    let y = area.y + 1;
    frame.set_cursor_position((x, y));
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_text = match &app.state {
        AppState::Idle => "Готово | F3 — копировать код, F4 — очистить, ↑↓ — скролл, q — выход",
        AppState::Loading => "⏳ Генерация...",
        AppState::Error(_) => "Произошла ошибка",
    };

    let status_color = match &app.state {
        AppState::Idle => Color::Gray,
        AppState::Loading => Color::Yellow,
        AppState::Error(_) => Color::Red,
    };

    let status = Paragraph::new(Span::styled(
        status_text,
        Style::default().fg(status_color),
    ))
    .alignment(Alignment::Left);

    frame.render_widget(status, area);
}
