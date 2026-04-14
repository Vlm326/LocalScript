mod app;
mod api;
mod config;
mod ui;

use std::fs::OpenOptions;
use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::app::{App, AppEvent, Effect, KeyAction};

fn init_file_logger() {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("llm-tui.log")
        .expect("Не удалось создать файл лога llm-tui.log");

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_file_logger();
    log::info!("=== llm-tui запущен (логи пишутся в llm-tui.log) ===");

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    execute!(
        stdout,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    std::panic::set_hook(Box::new(|panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        eprintln!("PANIC: {}", panic_info);
    }));

    let result = run_app(&mut terminal).await;

    std::panic::set_hook(Box::new(|_| {}));
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Ошибка: {}", err);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(64);
    spawn_input_reader(event_tx.clone());

    let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
    let mut active_request: Option<JoinHandle<()>> = None;
    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|frame| ui::render(frame, &app))?;

        tokio::select! {
            _ = tick.tick() => {
                app.handle_event(AppEvent::Tick);
            }
            Some(event) = event_rx.recv() => {
                let effect = app.handle_event(event);
                should_quit = apply_effect(effect, &event_tx, &mut active_request);
            }
        }
    }

    if let Some(handle) = active_request.take() {
        handle.abort();
        let _ = handle.await;
    }

    Ok(())
}

fn spawn_input_reader(event_tx: mpsc::Sender<AppEvent>) {
    tokio::task::spawn_blocking(move || loop {
        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                if let Some(action) = map_key_to_action(key.code) {
                    if event_tx.blocking_send(AppEvent::Key(action)).is_err() {
                        break;
                    }
                }
            }
            Ok(_) => {}
            Err(err) => {
                log::error!("Input reader failed: {}", err);
                break;
            }
        }
    });
}

fn map_key_to_action(code: KeyCode) -> Option<KeyAction> {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => Some(KeyAction::Quit),
        KeyCode::Enter => Some(KeyAction::Submit),
        KeyCode::Char(c) => Some(KeyAction::InsertChar(c)),
        KeyCode::Backspace => Some(KeyAction::Backspace),
        KeyCode::F(3) => Some(KeyAction::CopyLastCode),
        KeyCode::F(4) => Some(KeyAction::CancelOrReset),
        KeyCode::Up => Some(KeyAction::ScrollUp),
        KeyCode::Down => Some(KeyAction::ScrollDown),
        _ => None,
    }
}

fn apply_effect(
    effect: Effect,
    event_tx: &mpsc::Sender<AppEvent>,
    active_request: &mut Option<JoinHandle<()>>,
) -> bool {
    match effect {
        Effect::None => false,
        Effect::StartRequest(api_req) => {
            if let Some(handle) = active_request.take() {
                handle.abort();
            }

            let tx = event_tx.clone();
            *active_request = Some(tokio::spawn(async move {
                let result = crate::app::execute_api_request(api_req).await;
                let _ = tx.send(AppEvent::Api(result)).await;
            }));
            false
        }
        Effect::CancelRequest => {
            if let Some(handle) = active_request.take() {
                handle.abort();
            }
            false
        }
        Effect::Quit => {
            if let Some(handle) = active_request.take() {
                handle.abort();
            }
            true
        }
    }
}
