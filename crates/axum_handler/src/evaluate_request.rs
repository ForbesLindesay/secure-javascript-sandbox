use secure_js_sandbox::{CpuFuel, HttpMode, RequestLimit};
use serde::Deserialize;

use crate::{SandboxServerMemoryLimits, SandboxServerRequestTimeouts};

#[derive(Deserialize)]
pub struct EvaluateRequest {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
    pub filename: Option<String>,
}

#[derive(Default, Deserialize)]
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
    pub request_timeouts: SandboxServerRequestTimeouts,
    #[serde(default)]
    pub sandbox_auto_strip_types: bool,
    #[serde(default)]
    pub module_method: Option<Box<str>>,
}

#[derive(Deserialize)]
pub struct EvaluateRequestWithConfig {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
    pub filename: Option<String>,
    #[serde(default)]
    pub config: SandboxServerRequestConfig,
}

#[test]
fn test_request_timeouts_deserialize() {
    use std::time::Duration;

    let request: EvaluateRequestWithConfig = serde_json::from_str(
        r#"{
            "code": "() => 42",
            "parameters": [],
            "config": {
                "request_timeouts": {
                    "connect_timeout": "10s",
                    "first_byte_timeout": "5 minutes"
                }
            }
        }"#,
    )
    .expect("failed to deserialize request");
    let timeouts = request.config.request_timeouts.to_request_timeouts();
    assert_eq!(timeouts.connect_timeout, Duration::from_secs(10));
    assert_eq!(timeouts.first_byte_timeout, Duration::from_mins(5));
    assert_eq!(timeouts.between_bytes_timeout, Duration::from_mins(10));
}
