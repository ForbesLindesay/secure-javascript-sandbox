use secure_js_sandbox::{
    EvaluateError, MemoryOutputPipe, RequestValidationOutcome, SandboxConfig, SandboxInstanceBase,
    WasiCtx,
};
use serde::Serialize;

use crate::SandboxServerConfig;

#[derive(Serialize)]
pub struct EvaluateResponse {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub fuel_consumed: u64,
    pub fuel_remaining: u64,
    pub max_requested_memory_bytes: usize,
    pub max_requested_table_elements: usize,
    pub outbound_requests: Vec<OutboundRequest>,
    pub result: serde_json::Value,
}

pub(crate) struct EvaluateResponseBuilder {
    stdout: MemoryOutputPipe,
    stderr: MemoryOutputPipe,
    initial_cpu_fuel: u64,
}
impl EvaluateResponseBuilder {
    pub fn new(config: SandboxServerConfig) -> (SandboxConfig, EvaluateResponseBuilder) {
        let stdout = MemoryOutputPipe::new(config.memory_limits.stdout_max_bytes);
        let stderr = MemoryOutputPipe::new(config.memory_limits.stderr_max_bytes);
        (
            SandboxConfig {
                ctx: WasiCtx::builder()
                    .stdout(stdout.clone())
                    .stderr(stderr.clone())
                    .build(),
                cpu_fuel: config.cpu_fuel,
                memory_limits: config.memory_limits,
                http: config.http,
            },
            Self {
                stdout,
                stderr,
                initial_cpu_fuel: config.cpu_fuel,
            },
        )
    }
    pub fn build_response(
        self,
        result: Result<serde_json::Value, EvaluateError>,
        sandbox: impl SandboxInstanceBase,
    ) -> EvaluateResponse {
        EvaluateResponse {
            success: result.is_ok(),
            stdout: take_memory_pipe_contents(self.stdout),
            stderr: take_memory_pipe_contents(self.stderr),
            fuel_consumed: self
                .initial_cpu_fuel
                .saturating_sub(sandbox.get_fuel_remaining()),
            fuel_remaining: sandbox.get_fuel_remaining(),
            max_requested_memory_bytes: sandbox.get_max_requested_memory_bytes().unwrap_or(0),
            max_requested_table_elements: sandbox.get_max_requested_table_elements().unwrap_or(0),
            outbound_requests: sandbox
                .take_requests()
                .into_iter()
                .map(|(uri, socket_addr, outcome)| OutboundRequest {
                    uri: uri.to_string(),
                    socket_addr: socket_addr.map(|addr| addr.to_string()),
                    outcome,
                })
                .collect(),
            result: match result {
                Ok(value) => value,
                Err(err) => serde_json::json!({"error": err.to_string()}),
            },
        }
    }
}

#[derive(Serialize)]
pub struct OutboundRequest {
    pub uri: String,
    pub socket_addr: Option<String>,
    pub outcome: RequestValidationOutcome,
}

fn take_memory_pipe_contents(pipe: MemoryOutputPipe) -> String {
    std::str::from_utf8(&pipe.contents())
        .map(|s| s.to_owned())
        .unwrap_or_else(|_| "<invalid utf8 output>".to_string())
}
