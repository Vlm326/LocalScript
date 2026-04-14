use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::config::Config;
use std::time::Duration;

/// Структуры точно соответствуют backend API: llm-service/app/main.py

#[derive(Serialize, Clone)]
pub struct GenerateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub task: String,
    #[serde(rename = "user_response")]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub user_response: String,
    #[serde(rename = "llm_validation")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_validation: Option<bool>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct GenerateResponse {
    #[serde(rename = "session_id")]
    pub session_id: String,
    pub state: String,
    pub plan: Option<String>,
    pub code: Option<String>,
    #[serde(rename = "sandbox_feedback")]
    pub sandbox_feedback: Option<String>,
    pub message: String,
}

impl GenerateRequest {
    /// Первый запрос — только task, session_id ещё нет
    pub fn new_task(task: &str) -> Self {
        Self {
            session_id: None,
            task: task.to_string(),
            user_response: String::new(),
            llm_validation: Some(true),
        }
    }

    /// Последующие запросы — с session_id и user_response
    pub fn new_response(session_id: &str, user_response: &str) -> Self {
        Self {
            session_id: Some(session_id.to_string()),
            task: String::new(),
            user_response: user_response.to_string(),
            llm_validation: Some(true),
        }
    }
}

pub async fn generate(config: &Config, req: &GenerateRequest) -> Result<GenerateResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(300)) // 5 минут на запрос
        .build()?;
    
    let url = config.generate_url();
    
    // Логирование запроса
    log::info!("Отправка запроса на {}", url);
    if let Some(ref sid) = req.session_id {
        log::info!("  session_id: {}", sid);
    }
    if !req.task.is_empty() {
        log::info!("  task: {} (символов: {})", req.task.chars().take(50).collect::<String>(), req.task.len());
    }
    if !req.user_response.is_empty() {
        log::info!("  user_response: {}", req.user_response);
    }
    log::info!("  llm_validation: {:?}", req.llm_validation);

    let response = client
        .post(&url)
        .json(req)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        log::error!("Сервер вернул ошибку: {} — {}", status, body);
        anyhow::bail!("Сервер вернул ошибку: {} — {}", status, body);
    }

    let resp: GenerateResponse = response.json().await?;
    
    // Логирование ответа
    log::info!("Получен ответ: state={}, session_id={}", resp.state, resp.session_id);
    if let Some(ref plan) = resp.plan {
        log::info!("  plan: {} символов", plan.len());
    }
    if let Some(ref code) = resp.code {
        log::info!("  code: {} символов", code.len());
    }
    if let Some(ref fb) = resp.sandbox_feedback {
        log::info!("  sandbox_feedback: {} символов", fb.len());
    }
    log::info!("  message: {}", resp.message);

    Ok(resp)
}

pub async fn start_session(config: &Config, task: &str) -> Result<GenerateResponse> {
    let req = GenerateRequest::new_task(task);
    generate(config, &req).await
}

pub async fn send_response(
    config: &Config,
    session_id: &str,
    user_response: &str,
) -> Result<GenerateResponse> {
    let req = GenerateRequest::new_response(session_id, user_response);
    generate(config, &req).await
}
