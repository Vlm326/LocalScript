use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use crate::app::{App, ChatMessage, TuiState};

// ─── Constants ───────────────────────────────────────────────────────────────

static SPINNER_FRAMES: &[char] = &['⠋', '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

const SIDEBAR_WIDTH_RATIO: u16 = 25; // percent

// ─── Message role definitions ────────────────────────────────────────────────

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

// ─── Reusable helpers ────────────────────────────────────────────────────────

fn get_spinner_frame() -> char {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    SPINNER_FRAMES[(elapsed / 100) as usize % SPINNER_FRAMES.len()]
}

fn state_label(state: &TuiState) -> (&'static str, Color) {
    match state {
        TuiState::EnterTask => ("🔵 EnterTask", Color::Gray),
        TuiState::AwaitingPlan => ("🟡 AwaitingPlan", Color::Yellow),
        TuiState::AwaitingCode => ("🟡 AwaitingCode", Color::Cyan),
        TuiState::Done => ("🟢 Done", Color::Green),
        TuiState::Loading => ("🟡 Loading", Color::Yellow),
        TuiState::Error(_) => ("🔴 Error", Color::Red),
    }
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

// ─── Card-style message rendering ────────────────────────────────────────────

fn render_message_card(msg: &ChatMessage) -> ListItem<'_> {
    let style = role_style(msg);

    let content = match msg {
        ChatMessage::User(text) => text.clone(),
        ChatMessage::System(text) => text.clone(),
        ChatMessage::Plan(text) => text.clone(),
        ChatMessage::Code(text) => text.clone(),
        ChatMessage::Feedback(text) => text.clone(),
        ChatMessage::Error(text) => text.clone(),
    };

    let lines: Vec<Line> = content
        .lines()
        .map(|l| {
            Line::from(Span::styled(
                format!("  {}", l),
                Style::default().fg(style.text_color),
            ))
        })
        .collect();

    // Build card-style rendering with header and indented content
    let mut result: Vec<Line> = vec![
        Line::from(Span::styled(
            format!("─── {} ───", style.header),
            Style::default()
                .fg(style.border_color)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    result.extend(lines);
    result.push(Line::from("")); // spacing between messages

    ListItem::new(Text::from(result))
}

// ─── Sidebar rendering ───────────────────────────────────────────────────────

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let (state_label_text, state_color) = state_label(&app.state);

    let sid_display = app
        .session_id
        .as_deref()
        .unwrap_or("—");

    let hint_text = match &app.state {
        TuiState::EnterTask => "Введите задачу для начала работы",
        TuiState::AwaitingPlan => "Напишите фидбек или «Подтвердить»",
        TuiState::AwaitingCode => "Напишите фидбек или «Подтвердить»",
        TuiState::Done => "F3 — скопировать код, F4 — новая задача",
        TuiState::Loading => "Ожидание ответа от сервера...",
        TuiState::Error(_) => "Произошла ошибка. F4 — сброс",
    };

    let controls = "F3 Copy │ F4 Reset │ q Exit";

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
        .title(" Info ");

    let paragraph = Paragraph::new(Text::from(lines))
        .block(sidebar_block)
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

// ─── Main layout and rendering ───────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(5),     // history + sidebar
            Constraint::Length(3),  // input
            Constraint::Length(1),  // status bar
        ])
        .split(frame.area());

    // Split top area into history and sidebar
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
        .borders(Borders::ALL);

    // Render messages in natural order (oldest → newest)
    // scroll_offset=0 → newest messages visible
    // scroll_offset>0 → scroll back through history
    let total_messages = app.messages.len();
    let end_idx = total_messages.saturating_sub(app.scroll_offset);

    let mut items: Vec<ListItem> = Vec::new();
    for msg in &app.messages[..end_idx] {
        items.push(render_message_card(msg));
    }

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let enabled = input_enabled(app);
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
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        });

    let input = Paragraph::new(Text::from(Span::styled(
        display_text,
        if app.input.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        },
    )))
    .block(input_block);

    frame.render_widget(input, area);

    // Cursor: only when enabled
    if enabled {
        let x = area.x + app.input.len() as u16 + 1;
        let y = area.y + 1;
        frame.set_cursor_position((x, y));
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let is_loading = app.state == TuiState::Loading;

    let spinner = if is_loading {
        get_spinner_frame()
    } else {
        ' '
    };

    let (state_text, state_color) = state_label(&app.state);
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
            Span::styled(
                format!("session: {}", sid),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(" │ "),
            Span::styled(
                "Generating response...",
                Style::default().fg(Color::Yellow),
            ),
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
            Span::styled(
                format!("session: {}", sid),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(" │ "),
            Span::styled(
                "F3 Copy │ F4 Reset │ q Exit",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    };

    let status = Paragraph::new(status_line).alignment(Alignment::Left);
    frame.render_widget(status, area);
}
