mod app;
mod api;
mod config;
mod ui;

use std::io;
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

#[tokio::main]
async fn main() -> Result<()> {
    // Инициализация терминала
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Запуск приложения
    let result = run_app(&mut terminal).await;

    // Восстановление терминала
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Ошибка: {}", err);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new();
    let (tx, mut rx) = mpsc::channel(1);

    loop {
        // Отрисовка
        terminal.draw(|frame| {
            crate::ui::render(frame, &app);
        })?;

        // Обработка событий с таймаутом
        tokio::select! {
            Some(()) = rx.recv() => {
                app.send_message().await?;
            }
            result = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                tokio::task::spawn_blocking(|| event::poll(std::time::Duration::from_millis(100)))
            ) => {
                if let Ok(Ok(Ok(true))) = result {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                KeyCode::Enter => {
                                    let _ = tx.send(()).await;
                                }
                                KeyCode::Char(c) => {
                                    app.input.push(c);
                                }
                                KeyCode::Backspace => {
                                    app.input.pop();
                                }
                                KeyCode::F(3) => {
                                    if let Err(e) = app.copy_last_code() {
                                        app.messages.push(app::Message::Error(e.to_string()));
                                    }
                                }
                                KeyCode::F(4) => {
                                    app.clear_history();
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
                    }
                }
            }
        }
    }
}
