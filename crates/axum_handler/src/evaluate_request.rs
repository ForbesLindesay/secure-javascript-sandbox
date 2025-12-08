use secure_js_sandbox::{CpuFuel, HttpMode, RequestLimit};
use serde::Deserialize;

use crate::SandboxServerMemoryLimits;

#[derive(Deserialize)]
pub struct EvaluateRequest {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
}


#[derive(Deserialize)]
pub struct SandboxServerRequestConfig {
    #[serde(default)]
    pub cpu_fuel: CpuFuel,
    #[serde(default)]
    pub memory_limits: SandboxServerMemoryLimits,
    #[serde(default)]
    pub http: HttpMode,
    #[serde(default)]
    pub request_limit: RequestLimit,
    #[serde(default)]
    pub sandbox_auto_strip_types: bool,
    #[serde(default)]
    pub module_method: Option<Box<str>>,
}

impl Default for SandboxServerRequestConfig {
    fn default() -> Self {
        SandboxServerRequestConfig {
            cpu_fuel: Default::default(),
            memory_limits: Default::default(),
            http: Default::default(),
            request_limit: Default::default(),
            sandbox_auto_strip_types: false,
            module_method: None,
        }
    }
}


#[derive(Deserialize)]
pub struct EvaluateRequestWithConfig {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
    #[serde(default)]
    pub config: SandboxServerRequestConfig,
}
