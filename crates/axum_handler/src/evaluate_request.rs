use serde::Deserialize;

use crate::SandboxServerConfig;

#[derive(Deserialize)]
pub struct EvaluateRequest {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct EvaluateRequestWithConfig {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
    #[serde(default)]
    pub config: SandboxServerConfig,
}
