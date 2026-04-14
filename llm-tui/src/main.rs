mod app;
mod api;
mod config;
mod ui;

use std::io;
use std::fs::OpenOptions;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use anyhow::Result;

use crate::app::App;
use crate::app::TuiState;

/// Инициализирует логгер, записывающий логи в файл вместо stdout.
/// Это критически важно: ratatui использует alternate screen buffer,
/// и любые записи в stdout/stderr ломают рендеринг терминала.
fn init_file_logger() {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("llm-tui.log")
        .expect("Не удалось создать файл лога llm-tui.log");

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    )
    .target(env_logger::Target::Pipe(Box::new(log_file)))
    .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_file_logger();
    log::info!("=== llm-tui запущен (логи пишутся в llm-tui.log) ===");

    // Инициализация терминала: alternate screen + raw mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    execute!(stdout, crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Паника-безопасное восстановление терминала
    std::panic::set_hook(Box::new(|panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        eprintln!("PANIC: {}", panic_info);
    }));

    // Запуск приложения
    let result = run_app(&mut terminal).await;

    // Восстановление терминала: raw mode off + leave alternate screen
    std::panic::set_hook(Box::new(|_| {}));
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Ошибка выводится ТОЛЬКО после выхода из alternate screen
    if let Err(err) = result {
        eprintln!("Ошибка: {}", err);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();

    // Канал для результатов фоновых задач: worker → UI
    let (resp_tx, mut resp_rx) = mpsc::channel::<crate::app::ApiEvent>(8);

    loop {
        // ─── Отрисовка — всегда, без блокировок ──────────────────────────
        terminal.draw(|frame| {
            crate::ui::render(frame, &app);
        })?;

        // ─── Мультиплексирование: ответ от worker'а ИЛИ ввод пользователя ─
        tokio::select! {
            // Фоновая задача прислала результат — применяем мгновенно
            Some(api_event) = resp_rx.recv() => {
                app.handle_api_event(api_event);
            }

            // Ввод пользователя с таймаутом для поддержания цикла
            result = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                tokio::task::spawn_blocking(|| event::poll(std::time::Duration::from_millis(50)))
            ) => {
                if let Ok(Ok(Ok(true))) = result {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            handle_key(&mut app, &resp_tx, key.code);
                        }
                    }
                }
            }
        }
    }
}

/// Обработка клавиши — синхронная, мгновенная реакция на UI.
fn handle_key(app: &mut App, resp_tx: &mpsc::Sender<crate::app::ApiEvent>, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            std::process::exit(0);
        }
        KeyCode::Enter => {
            // Синхронное обновление UI: сообщение появляется мгновенно
            if let Some(api_req) = app.submit_message() {
                // Запускаем фоновую задачу — UI НЕ блокируется
                let tx = resp_tx.clone();
                tokio::spawn(async move {
                    let event = crate::app::execute_api_request(api_req).await;
                    let _ = tx.send(event).await;
                });
            }
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::F(3) => {
            if let Err(e) = app.copy_last_code() {
                app.messages.push(crate::app::ChatMessage::Error(e.to_string()));
            }
        }
        KeyCode::F(4) => {
            // Если загрузка — просто сбросим состояние, иначе полная очистка
            if app.state == TuiState::Loading {
                app.state = TuiState::EnterTask;
                app.messages.push(crate::app::ChatMessage::System(
                    "⚠️ Запрос отменён".to_string()
                ));
            } else {
                app.clear_history();
            }
        }
        KeyCode::Up => {
            app.scroll_up();
        }
        KeyCode::Down => {
            app.scroll_down();
        }
        _ => {}
    }
}
