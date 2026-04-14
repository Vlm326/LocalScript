use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, ChatMessage, DisplayState, TuiState};

static SPINNER_FRAMES: &[char] = &['⠋', '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SIDEBAR_WIDTH_RATIO: u16 = 25;
const BASE_STYLE: Style = Style::new().bg(Color::Reset);

struct RoleStyle {
    header: &'static str,
    border_color: Color,
    text_color: Color,
}

fn role_style(msg: &ChatMessage) -> RoleStyle {
    match msg {
        ChatMessage::User(_) => RoleStyle {
            header: "➤ YOU",
            border_color: Color::Cyan,
            text_color: Color::White,
        },
        ChatMessage::System(_) => RoleStyle {
            header: "🤖 SYSTEM",
            border_color: Color::Yellow,
            text_color: Color::White,
        },
        ChatMessage::Plan(_) => RoleStyle {
            header: "📋 PLAN",
            border_color: Color::Magenta,
            text_color: Color::Cyan,
        },
        ChatMessage::Code(_) => RoleStyle {
            header: "💻 CODE",
            border_color: Color::Green,
            text_color: Color::White,
        },
        ChatMessage::Feedback(_) => RoleStyle {
            header: "🔧 SANDBOX",
            border_color: Color::LightRed,
            text_color: Color::White,
        },
        ChatMessage::Error(_) => RoleStyle {
            header: "❌ ERROR",
            border_color: Color::Red,
            text_color: Color::Red,
        },
    }
}

fn get_spinner_frame() -> char {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    SPINNER_FRAMES[(elapsed / 100) as usize % SPINNER_FRAMES.len()]
}

fn state_label(display: DisplayState<'_>) -> (&'static str, Color) {
    match display {
        DisplayState::Loading => ("🟡 Loading", Color::Yellow),
        DisplayState::Ready(state) => match state {
            TuiState::EnterTask => ("🔵 EnterTask", Color::Gray),
            TuiState::AwaitingPlan => ("🟡 AwaitingPlan", Color::Yellow),
            TuiState::AwaitingCode => ("🟡 AwaitingCode", Color::Cyan),
            TuiState::Done => ("🟢 Done", Color::Green),
            TuiState::Error(_) => ("🔴 Error", Color::Red),
        },
    }
}

fn input_placeholder(app: &App) -> &'static str {
    if app.is_loading() {
        return "";
    }
    match &app.state {
        TuiState::EnterTask => "Введите задачу и нажмите Enter...",
        TuiState::AwaitingPlan => "Подтвердить / фидбек к плану...",
        TuiState::AwaitingCode => "Подтвердить / фидбек к коду...",
        TuiState::Done => "Сессия завершена. F4 — новая задача",
        TuiState::Error(_) => "Ошибка. F4 — новая задача",
    }
}

fn message_lines(msg: &ChatMessage) -> Vec<Line<'static>> {
    let style = role_style(msg);
    let content = match msg {
        ChatMessage::User(text)
        | ChatMessage::System(text)
        | ChatMessage::Plan(text)
        | ChatMessage::Code(text)
        | ChatMessage::Feedback(text)
        | ChatMessage::Error(text) => text,
    };

    let mut out: Vec<Line<'static>> = Vec::new();
    out.push(Line::from(Span::styled(
        format!("─── {} ───", style.header),
        Style::default()
            .fg(style.border_color)
            .add_modifier(Modifier::BOLD),
    )));

    for line in content.lines() {
        out.push(Line::from(Span::styled(
            format!("  {}", line),
            Style::default().fg(style.text_color),
        )));
    }

    out.push(Line::from(""));
    out
}

pub fn render(frame: &mut Frame, app: &App) {
    frame.render_widget(Clear, frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - SIDEBAR_WIDTH_RATIO),
            Constraint::Percentage(SIDEBAR_WIDTH_RATIO),
        ])
        .split(main_chunks[0]);

    render_history(frame, app, top_chunks[0]);
    render_sidebar(frame, app, top_chunks[1]);
    render_input(frame, app, main_chunks[1]);
    render_status_bar(frame, app, main_chunks[2]);
}

fn render_history(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" History ")
        .borders(Borders::ALL)
        .style(BASE_STYLE);

    let mut lines: Vec<Line<'static>> = Vec::new();
    for msg in &app.messages {
        lines.extend(message_lines(msg));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (нет сообщений)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let view_height = area.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(view_height) as u16;

    // app.scroll_offset is "lines from bottom"; Paragraph::scroll is "lines from top"
    let back = app.scroll_offset.min(max_scroll);
    let scroll_top = max_scroll.saturating_sub(back);

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((scroll_top, 0))
        .wrap(Wrap { trim: false })
        .style(BASE_STYLE);

    frame.render_widget(paragraph, area);
}

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let (state_label_text, state_color) = state_label(app.status_state());
    let sid_display = app.session_id.as_deref().unwrap_or("—");

    let hint_text = if app.is_loading() {
        "Ожидание ответа от сервера..."
    } else {
        match &app.state {
            TuiState::EnterTask => "Введите задачу для начала работы",
            TuiState::AwaitingPlan => "Напишите фидбек или «Подтвердить»",
            TuiState::AwaitingCode => "Напишите фидбек или «Подтвердить»",
            TuiState::Done => "F3 — скопировать код, F4 — новая задача",
            TuiState::Error(_) => "Произошла ошибка. F4 — сброс",
        }
    };

    let controls = "F3 Export │ F4 Reset │ Esc/Ctrl+Q Exit";

    let lines = vec![
        Line::from(Span::styled(
            " ═══ STATE ═══",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", state_label_text),
            Style::default().fg(state_color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " ═══ SESSION ═══",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", sid_display),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " ═══ HINT ═══",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", hint_text),
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " ═══ CONTROLS ═══",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", controls),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let sidebar_block = Block::default()
        .borders(Borders::ALL)
        .title(" Info ")
        .style(BASE_STYLE);

    let paragraph = Paragraph::new(Text::from(lines))
        .block(sidebar_block)
        .alignment(Alignment::Left)
        .style(BASE_STYLE);

    frame.render_widget(paragraph, area);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let enabled = app.input_enabled();
    let placeholder = input_placeholder(app);

    let display_text = if app.input.is_empty() {
        placeholder.to_string()
    } else {
        app.input.clone()
    };

    let input_block = Block::default()
        .title(" Input ")
        .borders(Borders::ALL)
        .style(if enabled {
            Style::default().bg(Color::Reset).fg(Color::Cyan)
        } else {
            Style::default().bg(Color::Reset)
        });

    let input = Paragraph::new(Text::from(Span::styled(
        display_text,
        if app.input.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        },
    )))
    .block(input_block)
    .style(BASE_STYLE);

    frame.render_widget(input, area);

    if enabled {
        let inner_width = area.width.saturating_sub(2);
        let width = UnicodeWidthStr::width(app.input.as_str()) as u16;
        let cursor_dx = width.min(inner_width.saturating_sub(1));
        let x = area.x + 1 + cursor_dx;
        let y = area.y + 1;
        frame.set_cursor_position((x, y));
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let is_loading = app.is_loading();
    let spinner = if is_loading { get_spinner_frame() } else { ' ' };

    let (state_text, state_color) = state_label(app.status_state());
    let sid = app.session_id.as_deref().unwrap_or("—");

    let status_line = if is_loading {
        Line::from(vec![
            Span::styled(
                format!("{} {}", spinner, state_text),
                Style::default()
                    .fg(state_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" │ "),
            Span::styled(format!("session: {}", sid), Style::default().fg(Color::Gray)),
            Span::raw(" │ "),
            Span::styled("Generating response...", Style::default().fg(Color::Yellow)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                state_text,
                Style::default()
                    .fg(state_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" │ "),
            Span::styled(format!("session: {}", sid), Style::default().fg(Color::Gray)),
            Span::raw(" │ "),
            Span::styled(
                "F3 Export │ F4 Reset │ Esc/Ctrl+Q Exit",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };

    let status = Paragraph::new(status_line)
        .alignment(Alignment::Left)
        .style(BASE_STYLE);
    frame.render_widget(status, area);
}
