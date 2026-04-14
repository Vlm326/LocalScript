use anyhow::{Result, bail};
use crate::config::Config;
use crate::api::{self, GenerateResponse};

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

/// События, обрабатываемые в главном цикле UI
#[derive(Debug, Clone)]
pub enum Event {
    /// Пользователь ввёл текст (нажал Enter)
    UserInput(String),
    /// Ответ от API
    ApiResponse(Result<GenerateResponse, String>),
}

/// Результат асинхронного API-запроса, отправляемый из фоновой задачи в UI-поток.
#[derive(Debug, Clone)]
pub enum ApiEvent {
    Response(GenerateResponse),
    Error(String),
}

/// Валидация допустимых переходов состояний
fn validate_state_transition(from: &TuiState, to: &str) -> bool {
    match (from, to) {
        (TuiState::EnterTask, "awaiting_plan_confirmation") => true,
        (TuiState::AwaitingPlan, "awaiting_plan_confirmation") => true,
        (TuiState::AwaitingPlan, "awaiting_code_approval") => true,
        (TuiState::AwaitingCode, "awaiting_code_approval") => true,
        (TuiState::AwaitingCode, "done") => true,
        _ => false,
    }
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

    // ─── Sync: мгновенное обновление UI при нажатии Enter ────────────────────

    /// Синхронная часть: добавляет сообщение пользователя, очищает input,
    /// переключает в Loading. Вызывается НЕМЕДЛЕННО при нажатии Enter.
    /// Принимает текст напрямую (не из input поля).
    pub fn submit_message_sync(&mut self, text: &str) -> Option<ApiRequest> {
        let text = text.trim().to_string();
        if text.is_empty() || self.state == TuiState::Loading {
            return None;
        }

        log::info!("User input: state={:?}, text={}", self.state, text);

        // Мгновенная реакция UI
        self.input.clear();
        self.scroll_offset = 0;
        self.messages.push(ChatMessage::User(text.clone()));

        // Нормализуем "Подтвердить" — регистронезависимо
        let normalized = if text.to_lowercase().contains("подтверд") {
            "Подтвердить".to_string()
        } else {
            text
        };

        // Валидация session_id для существующих сессий
        if self.state != TuiState::EnterTask && self.session_id.is_none() {
            log::error!("No session_id for state {:?}", self.state);
            self.messages.push(ChatMessage::Error("Ошибка: нет session_id. Сессия сброшена.".to_string()));
            self.state = TuiState::Error("No session_id".to_string());
            return None;
        }

        // Захватываем данные для фонового запроса ПЕРЕД сменой состояния
        let prev_state = self.state.clone();
        let session_id = self.session_id.clone();
        let config = self.config.clone();

        log::info!("Submitting API request: {:?} -> session_id={:?}", prev_state, session_id);

        // Переключаем в Loading
        self.state = TuiState::Loading;

        // Возвращаем запрос, который фоновая задача выполнит
        Some(ApiRequest {
            prev_state,
            session_id,
            config,
            text: normalized,
        })
    }

    // ─── Обработка ответа API (вызывается из главного цикла) ──────────────────

    /// Обрабатывает ответ от API. Вызывается ТОЛЬКО из главного цикла UI.
    pub fn handle_response(&mut self, resp: GenerateResponse) {
        self._apply_response(resp);
    }

    /// Вызывается когда фоновая задача прислала ошибку.
    #[allow(dead_code)]
    pub fn handle_error(&mut self, err: String) {
        log::error!("API Error: {}", err);
        self.messages.push(ChatMessage::Error(err.clone()));
        self.state = TuiState::Error(err);
    }

    fn _apply_response(&mut self, resp: GenerateResponse) {
        let prev_state = self.state.clone();
        
        // Логируем ответ
        log::info!("API Response: state={}, session_id={}", resp.state, resp.session_id);
        
        // Валидируем переход состояния
        if !validate_state_transition(&prev_state, &resp.state) {
            log::error!("Invalid state transition: {:?} -> {}. Игнорируем ответ.", prev_state, resp.state);
            self.messages.push(ChatMessage::Error(format!(
                "Ошибка: неверный переход состояния {:?} -> {}. Обратите внимание: состояние сброшено.",
                prev_state, resp.state
            )));
            self.state = TuiState::Error(format!("Invalid transition: {:?} -> {}", prev_state, resp.state));
            return;
        }

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
                self._handle_unexpected_state(&resp);
            }
        }
    }

    fn _handle_unexpected_state(&mut self, resp: &GenerateResponse) {
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
        if resp.state == "done" {
            self.state = TuiState::Done;
        } else if resp.code.is_some() {
            self.state = TuiState::AwaitingCode;
        } else {
            self.state = TuiState::AwaitingPlan;
        }
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

// ─── Данные для фоновой задачи (владеющие типы, Send) ────────────────────────

pub struct ApiRequest {
    pub prev_state: TuiState,
    pub session_id: Option<String>,
    pub config: Config,
    pub text: String,
}

/// Выполняет API-запрос в фоне. Вызывается из `tokio::spawn`.
pub async fn execute_api_request(req: ApiRequest) -> ApiEvent {
    let result: Result<GenerateResponse> = match &req.prev_state {
        TuiState::EnterTask => api::start_session(&req.config, &req.text).await,
        TuiState::AwaitingPlan | TuiState::AwaitingCode => {
            let sid = match req.session_id.as_deref() {
                Some(s) => s,
                None => return ApiEvent::Error("Нет session_id".into()),
            };
            api::send_response(&req.config, sid, &req.text).await
        }
        _ => return ApiEvent::Error(format!("Неожиданное состояние: {:?}", req.prev_state)),
    };

    match result {
        Ok(resp) => ApiEvent::Response(resp),
        Err(e) => ApiEvent::Error(e.to_string()),
    }
}
