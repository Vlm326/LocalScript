use anyhow::Result;
use crate::config::Config;
use crate::api;

#[derive(Debug, Clone)]
pub enum Message {
    User(String),
    Assistant(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Idle,
    Loading,
    Error(String),
}

pub struct App {
    pub messages: Vec<Message>,
    pub input: String,
    pub state: AppState,
    pub config: Config,
    pub scroll_offset: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            state: AppState::Idle,
            config: Config::new(),
            scroll_offset: 0,
        }
    }
    
    pub async fn send_message(&mut self) -> Result<()> {
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() || self.state == AppState::Loading {
            return Ok(());
        }
        
        // Добавляем сообщение пользователя
        self.messages.push(Message::User(prompt.clone()));
        self.input.clear();
        self.state = AppState::Loading;
        self.scroll_offset = 0;
        
        // Отправляем запрос на сервер
        match api::generate_code(&self.config, &prompt).await {
            Ok(code) => {
                self.messages.push(Message::Assistant(code));
                self.state = AppState::Idle;
            }
            Err(e) => {
                self.messages.push(Message::Error(e.to_string()));
                self.state = AppState::Idle;
            }
        }
        
        Ok(())
    }
    
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.scroll_offset = 0;
    }
    
    pub fn copy_last_code(&self) -> Result<()> {
        for msg in self.messages.iter().rev() {
            if let Message::Assistant(code) = msg {
                let mut clipboard = arboard::Clipboard::new()?;
                clipboard.set_text(code)?;
                return Ok(());
            }
        }
        anyhow::bail!("Нет кода для копирования")
    }
    
    pub fn scroll_up(&mut self) {
        if self.scroll_offset < self.messages.len().saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }
    
    pub fn scroll_down(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}
