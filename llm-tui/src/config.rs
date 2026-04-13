use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
}

impl Config {
    pub fn new() -> Self {
        let base_url = env::var("LLM_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        
        Self { base_url }
    }
    
    pub fn generate_url(&self) -> String {
        format!("{}/generate", self.base_url.trim_end_matches('/'))
    }
}
