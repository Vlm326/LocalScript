use anyhow::{Result, bail};
use crate::config::Config;
use crate::api;

/// Состояние TUI-приложения (отражает backend state machine)
#[derive(Debug, Clone, PartialEq)]
pub enum TuiState {
    /// Ожидание ввода задачи
    EnterTask,
    /// План сгенерирован, ждём подтверждение/фидбек
    AwaitingPlan,
    /// Код сгенерирован, ждём подтверждение/фидбек
    AwaitingCode,
    /// Сессия завершена, код одобрен
    Done,
    /// Ожидание ответа от сервера (загрузка)
    Loading,
    /// Ошибка
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    System(String),
    Plan(String),
    Code(String),
    Feedback(String),
    Error(String),
}

pub struct App {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub state: TuiState,
    pub config: Config,
    pub scroll_offset: usize,
    pub session_id: Option<String>,
    pub current_plan: Option<String>,
    pub current_code: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            state: TuiState::EnterTask,
            config: Config::new(),
            scroll_offset: 0,
            session_id: None,
            current_plan: None,
            current_code: None,
        }
    }

    pub async fn send_message(&mut self) -> Result<()> {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.state == TuiState::Loading {
            return Ok(());
        }

        self.input.clear();
        self.scroll_offset = 0;

        // Добавляем сообщение пользователя в историю
        self.messages.push(ChatMessage::User(text.clone()));

        // Нормализуем "Подтвердить" — регистронезависимо
        let normalized = if text.to_lowercase().contains("подтверд") {
            "Подтвердить".to_string()
        } else {
            text.clone()
        };

        // Сохраняем состояние ДО переключения в Loading
        let prev_state = std::mem::replace(&mut self.state, TuiState::Loading);

        let result = match prev_state {
            TuiState::EnterTask => self._start_session(&text).await,
            TuiState::AwaitingPlan | TuiState::AwaitingCode => {
                self._send_response(&normalized).await
            }
            _ => Ok(()),
        };

        if let Err(e) = result {
            self.messages.push(ChatMessage::Error(e.to_string()));
            self.state = TuiState::Error(e.to_string());
        }

        Ok(())
    }

    async fn _start_session(&mut self, task: &str) -> Result<()> {
        self.messages
            .push(ChatMessage::System(format!("📝 Задача: {}", task)));

        let resp = api::start_session(&self.config, task).await?;
        self.session_id = Some(resp.session_id.clone());

        match resp.state.as_str() {
            "awaiting_plan_confirmation" => {
                if let Some(plan) = resp.plan {
                    self.current_plan = Some(plan.clone());
                    self.messages.push(ChatMessage::Plan(plan));
                }
                self.messages.push(ChatMessage::System(format!(
                    "💬 {}",
                    resp.message
                )));
                self.state = TuiState::AwaitingPlan;
            }
            _ => {
                self._handle_unexpected_state(&resp)?;
            }
        }

        Ok(())
    }

    async fn _send_response(&mut self, user_response: &str) -> Result<()> {
        let sid = self
            .session_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Нет session_id"))?;

        let resp = api::send_response(&self.config, sid, user_response).await?;
        self.session_id = Some(resp.session_id.clone());

        match resp.state.as_str() {
            "awaiting_plan_confirmation" => {
                // План обновлён по фидбеку
                if let Some(plan) = resp.plan {
                    self.current_plan = Some(plan.clone());
                    self.messages.push(ChatMessage::Plan(plan));
                }
                self.messages.push(ChatMessage::System(format!(
                    "💬 {}",
                    resp.message
                )));
                self.state = TuiState::AwaitingPlan;
            }
            "awaiting_code_approval" => {
                if let Some(code) = resp.code {
                    self.current_code = Some(code.clone());
                    self.messages.push(ChatMessage::Code(code));
                }
                if let Some(fb) = resp.sandbox_feedback {
                    if !fb.is_empty() {
                        self.messages.push(ChatMessage::Feedback(fb));
                    }
                }
                self.messages.push(ChatMessage::System(format!(
                    "💬 {}",
                    resp.message
                )));
                self.state = TuiState::AwaitingCode;
            }
            "done" => {
                if let Some(code) = resp.code {
                    self.current_code = Some(code.clone());
                    self.messages.push(ChatMessage::Code(code));
                }
                self.messages.push(ChatMessage::System(format!(
                    "✅ {}",
                    resp.message
                )));
                self.state = TuiState::Done;
            }
            _ => {
                self._handle_unexpected_state(&resp)?;
            }
        }

        Ok(())
    }

    fn _handle_unexpected_state(&mut self, resp: &api::GenerateResponse) -> Result<()> {
        // Если вернулся неожиданный state — просто покажем что есть
        if let Some(plan) = &resp.plan {
            self.current_plan = Some(plan.clone());
            self.messages.push(ChatMessage::Plan(plan.clone()));
        }
        if let Some(code) = &resp.code {
            self.current_code = Some(code.clone());
            self.messages.push(ChatMessage::Code(code.clone()));
        }
        self.messages.push(ChatMessage::System(format!(
            "📋 state={}, {}",
            resp.state, resp.message
        )));
        if !resp
            .sandbox_feedback
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            self.messages.push(ChatMessage::Feedback(
                resp.sandbox_feedback.clone().unwrap_or_default(),
            ));
        }
        // Определяем состояние по содержимому
        if resp.state == "done" {
            self.state = TuiState::Done;
        } else if resp.code.is_some() {
            self.state = TuiState::AwaitingCode;
        } else {
            self.state = TuiState::AwaitingPlan;
        }
        Ok(())
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.session_id = None;
        self.current_plan = None;
        self.current_code = None;
        self.state = TuiState::EnterTask;
        self.scroll_offset = 0;
    }

    pub fn copy_last_code(&self) -> Result<()> {
        if let Some(code) = &self.current_code {
            let mut clipboard = arboard::Clipboard::new()?;
            clipboard.set_text(code)?;
            return Ok(());
        }
        bail!("Нет кода для копирования")
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
