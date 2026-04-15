use anyhow::{bail, Result};

use std::path::PathBuf;

use crate::api::{self, GenerateResponse};
use crate::config::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiState {
    EnterTask,
    AwaitingPlan,
    AwaitingCode,
    Done,
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

#[derive(Debug, Clone)]
pub enum KeyAction {
    Submit,
    InsertChar(char),
    InsertText(String),
    Backspace,
    CopyLastCode,
    CancelOrReset,
    ScrollUp,
    ScrollDown,
    Quit,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyAction),
    Api(ApiResult),
    Tick,
}

#[derive(Debug, Clone)]
pub enum Effect {
    None,
    StartRequest(ApiRequest),
    CancelRequest,
    Quit,
}

#[derive(Debug, Clone)]
pub enum ApiResult {
    Response {
        request_id: u64,
        response: GenerateResponse,
    },
    Error {
        request_id: u64,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct RequestMeta {
    pub request_id: u64,
    pub origin_state: TuiState,
}

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
    pub scroll_offset: u16,
    pub session_id: Option<String>,
    pub current_plan: Option<String>,
    pub current_code: Option<String>,
    pub active_request: Option<RequestMeta>,
    next_request_id: u64,
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
            active_request: None,
            next_request_id: 1,
        }
    }

    pub fn is_loading(&self) -> bool {
        self.active_request.is_some()
    }

    pub fn input_enabled(&self) -> bool {
        !self.is_loading()
            && matches!(
                self.state,
                TuiState::EnterTask | TuiState::AwaitingPlan | TuiState::AwaitingCode
            )
    }

    pub fn status_state(&self) -> DisplayState<'_> {
        if self.is_loading() {
            DisplayState::Loading
        } else {
            DisplayState::Ready(&self.state)
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) -> Effect {
        match event {
            AppEvent::Key(action) => self.handle_key_action(action),
            AppEvent::Api(result) => {
                self.handle_api_result(result);
                Effect::None
            }
            AppEvent::Tick => Effect::None,
        }
    }

    fn handle_key_action(&mut self, action: KeyAction) -> Effect {
        match action {
            KeyAction::InsertChar(c) => {
                if self.input_enabled() {
                    self.input.push(c);
                }
                Effect::None
            }
            KeyAction::InsertText(text) => {
                if self.input_enabled() {
                    let sanitized = sanitize_paste(&text);
                    if !sanitized.is_empty() {
                        self.input.push_str(&sanitized);
                    }
                }
                Effect::None
            }
            KeyAction::Backspace => {
                if self.input_enabled() {
                    self.input.pop();
                }
                Effect::None
            }
            KeyAction::Submit => self.submit_current_input(),
            KeyAction::CopyLastCode => {
                self.export_last_code();
                Effect::None
            }
            KeyAction::CancelOrReset => {
                if self.is_loading() {
                    self.cancel_active_request()
                } else {
                    self.reset_session();
                    Effect::None
                }
            }
            KeyAction::ScrollUp => {
                self.scroll_up();
                Effect::None
            }
            KeyAction::ScrollDown => {
                self.scroll_down();
                Effect::None
            }
            KeyAction::Quit => Effect::Quit,
        }
    }

    fn submit_current_input(&mut self) -> Effect {
        if !self.input_enabled() {
            return Effect::None;
        }

        let text = self.input.trim().to_string();
        if text.is_empty() {
            return Effect::None;
        }

        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let origin_state = self.state.clone();
        let session_id = self.session_id.clone();
        let config = self.config.clone();

        if origin_state != TuiState::EnterTask && session_id.is_none() {
            let err = "Ошибка: нет session_id. Сессия сброшена.".to_string();
            self.messages.push(ChatMessage::Error(err.clone()));
            self.state = TuiState::Error("No session_id".to_string());
            self.input.clear();
            return Effect::None;
        }

        self.messages.push(ChatMessage::User(text.clone()));
        self.input.clear();
        self.scroll_offset = 0;
        self.active_request = Some(RequestMeta {
            request_id,
            origin_state: origin_state.clone(),
        });

        log::info!(
            "Submitting API request: request_id={}, state={:?}, session_id={:?}",
            request_id,
            origin_state,
            session_id
        );

        Effect::StartRequest(ApiRequest {
            request_id,
            origin_state,
            session_id,
            config,
            text,
        })
    }

    fn cancel_active_request(&mut self) -> Effect {
        let Some(meta) = self.active_request.take() else {
            return Effect::None;
        };

        self.state = meta.origin_state;
        self.messages
            .push(ChatMessage::System("⚠️ Запрос отменён".to_string()));
        Effect::CancelRequest
    }

    fn handle_api_result(&mut self, result: ApiResult) {
        match result {
            ApiResult::Response {
                request_id,
                response,
            } => self.handle_response(request_id, response),
            ApiResult::Error { request_id, error } => self.handle_error(request_id, error),
        }
    }

    fn handle_response(&mut self, request_id: u64, resp: GenerateResponse) {
        let Some(meta) = self.take_matching_request(request_id) else {
            log::warn!("Ignoring stale response for request_id={}", request_id);
            return;
        };

        log::info!(
            "API Response: request_id={}, state={}, session_id={}",
            request_id,
            resp.state,
            resp.session_id
        );

        if !validate_state_transition(&meta.origin_state, &resp.state) {
            let err = format!(
                "Ошибка: неверный переход состояния {:?} -> {}.",
                meta.origin_state, resp.state
            );
            log::error!("{}", err);
            self.messages.push(ChatMessage::Error(err.clone()));
            self.state = TuiState::Error(err);
            return;
        }

        self.session_id = Some(resp.session_id.clone());
        self.apply_response(resp);
    }

    fn handle_error(&mut self, request_id: u64, err: String) {
        let Some(_) = self.take_matching_request(request_id) else {
            log::warn!("Ignoring stale error for request_id={}", request_id);
            return;
        };

        log::error!("API Error: {}", err);
        self.messages.push(ChatMessage::Error(err.clone()));
        self.state = TuiState::Error(err);
    }

    fn take_matching_request(&mut self, request_id: u64) -> Option<RequestMeta> {
        if self
            .active_request
            .as_ref()
            .is_some_and(|meta| meta.request_id == request_id)
        {
            return self.active_request.take();
        }
        None
    }

    fn apply_response(&mut self, resp: GenerateResponse) {
        match resp.state.as_str() {
            "awaiting_plan_confirmation" => {
                if let Some(plan) = resp.plan {
                    self.current_plan = Some(plan.clone());
                    self.messages.push(ChatMessage::Plan(plan));
                }
                self.messages
                    .push(ChatMessage::System(format!("💬 {}", resp.message)));
                self.state = TuiState::AwaitingPlan;
            }
            "awaiting_code_approval" => {
                if let Some(code) = resp.code {
                    self.current_code = Some(code.clone());
                    self.messages.push(ChatMessage::Code(code));
                }
                if let Some(fb) = resp.sandbox_feedback.filter(|fb| !fb.is_empty()) {
                    self.messages.push(ChatMessage::Feedback(fb));
                }
                self.messages
                    .push(ChatMessage::System(format!("💬 {}", resp.message)));
                self.state = TuiState::AwaitingCode;
            }
            "done" => {
                if let Some(code) = resp.code {
                    self.current_code = Some(code.clone());
                    self.messages.push(ChatMessage::Code(code));
                }
                self.messages
                    .push(ChatMessage::System(format!("✅ {}", resp.message)));
                self.state = TuiState::Done;
            }
            other => {
                let err = format!("Неожиданное состояние ответа: {}", other);
                log::error!("{}", err);
                self.messages.push(ChatMessage::Error(err.clone()));
                self.state = TuiState::Error(err);
            }
        }

        self.scroll_offset = 0;
    }

    fn reset_session(&mut self) {
        self.messages.clear();
        self.input.clear();
        self.session_id = None;
        self.current_plan = None;
        self.current_code = None;
        self.state = TuiState::EnterTask;
        self.scroll_offset = 0;
        self.active_request = None;
    }

    pub fn copy_last_code(&self) -> Result<()> {
        if let Some(code) = &self.current_code {
            let mut clipboard = arboard::Clipboard::new()?;
            clipboard.set_text(code)?;
            return Ok(());
        }
        bail!("Нет кода для копирования")
    }

    fn export_last_code(&mut self) {
        let Some(code) = self.current_code.clone() else {
            self.messages
                .push(ChatMessage::Error("Нет кода для экспорта".to_string()));
            return;
        };

        match self.copy_last_code() {
            Ok(()) => {
                self.messages
                    .push(ChatMessage::System("✅ Код скопирован в буфер обмена".to_string()));
            }
            Err(e) => {
                self.messages.push(ChatMessage::System(format!(
                    "⚠️ Не удалось скопировать в буфер обмена: {}",
                    e
                )));
            }
        }

        let lua_path = PathBuf::from("/app/exports/llm_last_code.lua");
        let json_path = PathBuf::from("/app/exports/llm_last_code.json");
        let content = ensure_trailing_newline(&code);

        match std::fs::write(&lua_path, content.clone()) {
            Ok(()) => self.messages.push(ChatMessage::System(format!(
                "💾 Код сохранён в файл: {}",
                lua_path.display()
            ))),
            Err(e) => self.messages.push(ChatMessage::Error(format!(
                "Не удалось сохранить файл {}: {}",
                lua_path.display(),
                e
            ))),
        }

        self.export_via_parser(&code, &json_path);
    }

    fn export_via_parser(&mut self, _code: &str, json_path: &PathBuf) {
        let parser_script = PathBuf::from("/app/parse_lua_to_json.py");
        if !parser_script.exists() {
            self.messages.push(ChatMessage::Error(
                "⚠️ Скрипт parse_lua_to_json.py не найден".to_string(),
            ));
            return;
        }

        let output = std::process::Command::new("python3")
            .arg("-c")
            .arg(format!(
                "import sys; sys.path.insert(0, '/app'); from parse_lua_to_json import parse_lua_to_json; print(parse_lua_to_json(sys.stdin.read()))"
            ))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    let json_content = String::from_utf8_lossy(&output.stdout);
                    match std::fs::write(json_path, json_content.as_ref()) {
                        Ok(()) => self.messages.push(ChatMessage::System(format!(
                            "📄 JSON сохранён в файл: {}",
                            json_path.display()
                        ))),
                        Err(e) => self.messages.push(ChatMessage::Error(format!(
                            "⚠️ Не удалось сохранить JSON: {}",
                            e
                        ))),
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.messages.push(ChatMessage::Error(format!(
                        "⚠️ Ошибка парсера: {}",
                        stderr
                    )));
                }
            }
            Err(e) => {
                self.messages.push(ChatMessage::Error(format!(
                    "⚠️ Не удалось запустить парсер: {}",
                    e
                )));
            }
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(3);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(3);
    }
}

fn ensure_trailing_newline(s: &str) -> String {
    if s.ends_with('\n') {
        s.to_string()
    } else {
        format!("{}\n", s)
    }
}

fn sanitize_paste(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\r' | '\n' | '\t' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out
}

fn find_repo_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..10 {
        if dir.join("docker-compose.yml").is_file() || dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

pub enum DisplayState<'a> {
    Loading,
    Ready(&'a TuiState),
}

#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub request_id: u64,
    pub origin_state: TuiState,
    pub session_id: Option<String>,
    pub config: Config,
    pub text: String,
}

pub async fn execute_api_request(req: ApiRequest) -> ApiResult {
    let result: Result<GenerateResponse> = match &req.origin_state {
        TuiState::EnterTask => api::start_session(&req.config, &req.text).await,
        TuiState::AwaitingPlan | TuiState::AwaitingCode => {
            let sid = match req.session_id.as_deref() {
                Some(s) => s,
                None => {
                    return ApiResult::Error {
                        request_id: req.request_id,
                        error: "Нет session_id".into(),
                    };
                }
            };
            api::send_response(&req.config, sid, &req.text).await
        }
        TuiState::Done | TuiState::Error(_) => {
            return ApiResult::Error {
                request_id: req.request_id,
                error: format!("Неожиданное состояние: {:?}", req.origin_state),
            };
        }
    };

    match result {
        Ok(response) => ApiResult::Response {
            request_id: req.request_id,
            response,
        },
        Err(error) => ApiResult::Error {
            request_id: req.request_id,
            error: error.to_string(),
        },
    }
}
