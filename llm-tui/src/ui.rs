use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use crate::app::{App, ChatMessage, TuiState};

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
        .title(" История ")
        .borders(Borders::ALL);

    let items: Vec<ListItem> = app
        .messages
        .iter()
        .rev()
        .skip(app.scroll_offset)
        .flat_map(|msg| match msg {
            ChatMessage::User(text) => {
                let line = Line::from(Span::styled(
                    format!("➤ {}", text),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                vec![line, Line::from("")]
            }
            ChatMessage::System(text) => {
                let line = Line::from(Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                vec![line, Line::from("")]
            }
            ChatMessage::Plan(text) => {
                let header = Line::from(Span::styled(
                    "── ПЛАН ──",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
                let lines: Vec<Line> = text
                    .lines()
                    .map(|l| Line::from(Span::styled(format!("  {}", l), Style::default().fg(Color::Cyan))))
                    .collect();
                let mut result = vec![header];
                result.extend(lines);
                result.push(Line::from(""));
                result
            }
            ChatMessage::Code(text) => {
                let header = Line::from(Span::styled(
                    "── КОД ──",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
                let lines: Vec<Line> = text
                    .lines()
                    .map(|l| Line::from(Span::styled(format!("  {}", l), Style::default().fg(Color::White))))
                    .collect();
                let mut result = vec![header];
                result.extend(lines);
                result.push(Line::from(""));
                result
            }
            ChatMessage::Feedback(text) => {
                let line = Line::from(Span::styled(
                    format!("🔧 Sandbox: {}", text),
                    Style::default()
                        .fg(Color::Red),
                ));
                vec![line, Line::from("")]
            }
            ChatMessage::Error(err) => {
                let line = Line::from(Span::styled(
                    format!("❌ Ошибка: {}", err),
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

fn input_placeholder(app: &App) -> &'static str {
    match &app.state {
        TuiState::EnterTask => "Введите задачу и нажмите Enter...",
        TuiState::AwaitingPlan => "Подтвердить / фидбек к плану...",
        TuiState::AwaitingCode => "Подтвердить / фидбек к коду...",
        TuiState::Done => "Сессия завершена. F4 — новая задача",
        TuiState::Loading => "",
        TuiState::Error(_) => "Ошибка. F4 — новая задача",
    }
}

fn input_enabled(app: &App) -> bool {
    matches!(
        app.state,
        TuiState::EnterTask
            | TuiState::AwaitingPlan
            | TuiState::AwaitingCode
    )
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(format!(" {} ", input_placeholder(app)))
        .borders(Borders::ALL);

    let text = if input_enabled(app) {
        app.input.as_str()
    } else {
        ""
    };

    let input = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(input, area);

    if input_enabled(app) {
        let x = area.x + app.input.len() as u16 + 1;
        let y = area.y + 1;
        frame.set_cursor_position((x, y));
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let (status_text, status_color) = match &app.state {
        TuiState::EnterTask => ("Ввод задачи", Color::Gray),
        TuiState::AwaitingPlan => ("⏳ Ожидание подтверждения плана", Color::Yellow),
        TuiState::AwaitingCode => ("⏳ Ожидание подтверждения кода", Color::Cyan),
        TuiState::Done => ("✅ Код одобрен", Color::Green),
        TuiState::Loading => ("⏳ Генерация...", Color::Yellow),
        TuiState::Error(_) => ("❌ Ошибка", Color::Red),
    };

    let sid = app
        .session_id
        .as_deref()
        .unwrap_or("—");

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("{} | sid: {}", status_text, sid),
            Style::default().fg(status_color),
        ),
        Span::styled(
            " | F3 — копировать, F4 — новая, ↑↓ — скролл, q — выход",
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .alignment(Alignment::Left);

    frame.render_widget(status, area);
}
