use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::config::Config;

#[derive(Serialize)]
pub struct GenerateRequest {
    pub prompt: String,
}

#[derive(Deserialize)]
pub struct GenerateResponse {
    pub code: String,
}

pub async fn generate_code(config: &Config, prompt: &str) -> Result<String> {
    let client = Client::new();
    let url = config.generate_url();
    
    let request = GenerateRequest {
        prompt: prompt.to_string(),
    };
    
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Сервер вернул ошибку: {} - {}", status, body);
    }
    
    let response: GenerateResponse = response.json().await?;
    Ok(response.code)
}
