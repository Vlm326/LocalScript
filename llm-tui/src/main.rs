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
use crate::app::Event as TuiEvent;

/// Инициализирует логгер, записывающий логи в файл вместо stdout.
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
    let (event_tx, mut event_rx) = mpsc::channel::<TuiEvent>(8);

    loop {
        // ─── Отрисовка — всегда, без блокировок ──────────────────────────
        terminal.draw(|frame| {
            crate::ui::render(frame, &app);
        })?;

        // ─── Обработка событий ─────────────────────────────────────────────
        // Приоритет: API response → User input
        
        tokio::select! {
            // Фоновая задача прислала результат
            Some(api_event) = resp_rx.recv() => {
                let result = match api_event {
                    crate::app::ApiEvent::Response(resp) => Ok(resp),
                    crate::app::ApiEvent::Error(err) => Err(err),
                };
                process_event(&mut app, TuiEvent::ApiResponse(result));
            }

            // Событие от UI (отправлено через handle_key)
            Some(event) = event_rx.recv() => {
                process_user_event(&mut app, &resp_tx, event);
            }

            // Ввод пользователя с таймаутом для поддержания цикла
            result = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                tokio::task::spawn_blocking(|| event::poll(std::time::Duration::from_millis(50)))
            ) => {
                if let Ok(Ok(Ok(true))) = result {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            // Преобразуем ввод в событие и отправляем в канал
                            handle_key_to_event(&mut app, key.code, &event_tx);
                        }
                    }
                }
            }
        }
    }
}

/// Обработка события от пользователя (из канала)
fn process_user_event(app: &mut App, resp_tx: &mpsc::Sender<crate::app::ApiEvent>, event: TuiEvent) {
    match event {
        TuiEvent::UserInput(text) => {
            // Игнорируем ввод во время загрузки
            if app.state == TuiState::Loading {
                log::warn!("User input ignored: state is Loading");
                return;
            }

            // Создаём запрос (submit_message проверяет состояние и валидность)
            if let Some(api_req) = app.submit_message_sync(&text) {
                let tx = resp_tx.clone();
                tokio::spawn(async move {
                    let event = crate::app::execute_api_request(api_req).await;
                    let _ = tx.send(event).await;
                });
            }
        }
        TuiEvent::ApiResponse(_) => {
            // Этот кейс обрабатывается в главном цикле напрямую
        }
    }
}

/// Преобразование нажатия клавиши в событие и отправка в канал
fn handle_key_to_event(app: &mut App, code: KeyCode, event_tx: &mpsc::Sender<TuiEvent>) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            std::process::exit(0);
        }
        KeyCode::Enter => {
            // Отправляем событие вместо прямой обработки
            let input = app.input.clone();
            let _ = event_tx.try_send(TuiEvent::UserInput(input));
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

/// Обработка API ответа — вызывается в главном цикле
fn process_event(app: &mut App, event: TuiEvent) {
    match event {
        TuiEvent::ApiResponse(result) => {
            match result {
                Ok(resp) => {
                    app.handle_response(resp);
                }
                Err(err) => {
                    log::error!("API Error: {}", err);
                    app.messages.push(crate::app::ChatMessage::Error(err.clone()));
                    app.state = TuiState::Error(err);
                }
            }
        }
        TuiEvent::UserInput(_) => {}
    }
}
