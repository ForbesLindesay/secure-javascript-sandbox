#![deny(warnings)]

use axum::{
    Json,
    routing::{MethodRouter, post},
};
use secure_js_sandbox::{RequestValidationOutcome, SandboxConfig, SandboxEngine, WasiCtx};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{error::Error, str::FromStr, sync::Arc};

pub use secure_js_sandbox::{CustomHttpMode, HttpMode, MemoryLimits, MemoryOutputPipe};

pub trait SandboxServerConfigImpl: Send + Sync + 'static {
    type RequestType: DeserializeOwned + Send + 'static;
    fn get_evaluate_input(&self, request: Self::RequestType) -> EvaluateInput;
}

impl<T: SandboxServerConfigImpl> SandboxServerConfigImpl for Arc<T> {
    type RequestType = T::RequestType;
    fn get_evaluate_input(&self, request: T::RequestType) -> EvaluateInput {
        (**self).get_evaluate_input(request)
    }
}

pub struct EvaluateInput {
    pub script: String,
    pub args: Vec<serde_json::Value>,
    pub config: SandboxServerConfig,
}

#[derive(Deserialize)]
pub struct EvaluateRequest {
    pub script: String,
    pub args: Vec<serde_json::Value>,
}

fn cpu_fuel_default() -> u64 {
    440_000_000 // Approximately 100ms on my MacBook Pro.
}

#[derive(Clone, Deserialize)]
pub struct SandboxServerConfig {
    #[serde(default="cpu_fuel_default")]
    pub cpu_fuel: u64,
    #[serde(default)]
    pub memory_limits: MemoryLimits,
    #[serde(default)]
    pub http: HttpMode,
}

impl Default for SandboxServerConfig {
    fn default() -> Self {
        SandboxServerConfig {
            cpu_fuel: cpu_fuel_default(),
            memory_limits: MemoryLimits::default(),
            http: HttpMode::Disabled,
        }
    }
}

pub fn get_env<T: FromStr>(var: &str) -> anyhow::Result<Option<T>> {
    match std::env::var(var) {
        Ok(s) => match s.parse() {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err(anyhow::anyhow!("Failed to parse env var {}: {}", var, s)),
        },
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to read env var {}: {}", var, e)),
    }
}

impl SandboxServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        if let Some(cpu_fuel) = get_env("SANDBOX_CPU_FUEL")? {
            config.cpu_fuel = cpu_fuel;
        }
        if let Some(max_memory_bytes) = get_env("SANDBOX_MAX_MEMORY_BYTES")? {
            config.memory_limits.memory_size_bytes = Some(max_memory_bytes);
        }
        if let Some(max_table_elements) = get_env("SANDBOX_MAX_TABLE_ELEMENTS")? {
            config.memory_limits.table_elements = Some(max_table_elements);
        }
        if let Some(instances) = get_env("SANDBOX_MAX_INSTANCES")? {
            config.memory_limits.instances = instances;
        }
        if let Some(tables) = get_env("SANDBOX_MAX_TABLES")? {
            config.memory_limits.tables = tables;
        }
        if let Some(memories) = get_env("SANDBOX_MAX_MEMORIES")? {
            config.memory_limits.memories = memories;
        }
        if let Some(trap_on_grow_failure) = get_env("SANDBOX_TRAP_ON_GROW_FAILURE")? {
            config.memory_limits.trap_on_grow_failure = trap_on_grow_failure;
        }
        if let Some(stdout_max_bytes) = get_env("SANDBOX_STDOUT_MAX_BYTES")? {
            config.memory_limits.stdout_max_bytes = stdout_max_bytes;
        }
        if let Some(stderr_max_bytes) = get_env("SANDBOX_STDERR_MAX_BYTES")? {
            config.memory_limits.stderr_max_bytes = stderr_max_bytes;
        }
        if let Some(http) = get_env("SANDBOX_HTTP_MODE")? {
            config.http = http;
        }

        Ok(config)
    }
}

impl SandboxServerConfigImpl for SandboxServerConfig {
    type RequestType = EvaluateRequest;

    fn get_evaluate_input(&self, request: Self::RequestType) -> EvaluateInput {
        EvaluateInput {
            script: request.script,
            args: request.args,
            config: self.clone(),
        }
    }
}

#[derive(Deserialize)]
pub struct EvaluateRequestWithConfig {
    pub script: String,
    pub args: Vec<serde_json::Value>,
    #[serde(default)]
    pub config: SandboxServerConfig,
}
pub struct AllowRequestToConfigureSandbox;
impl SandboxServerConfigImpl for AllowRequestToConfigureSandbox {
    type RequestType = EvaluateRequestWithConfig;

    fn get_evaluate_input(&self, request: Self::RequestType) -> EvaluateInput {
        EvaluateInput {
            script: request.script,
            args: request.args,
            config: request.config,
        }
    }
}

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

#[derive(Serialize)]
pub struct OutboundRequest {
    pub uri: String,
    pub socket_addr: Option<String>,
    pub outcome: RequestValidationOutcome,
}

pub async fn create_evaluate_handler<
    TConfig: SandboxServerConfigImpl,
    T: Clone + Send + Sync + 'static,
>(
    config: TConfig,
) -> Result<MethodRouter<T>, Box<dyn Error>> {
    let config = Arc::new(config);
    let engine = Arc::new(SandboxEngine::new()?);
    let result: MethodRouter<T> = post(
        async move |Json(request): Json<TConfig::RequestType>| -> Json<serde_json::Value> {
            match evaluate(&config, request, &engine).await {
                Ok(response) => Json(serde_json::to_value(response).unwrap()),
                Err(err) => Json(serde_json::json!({"error": err.to_string()})),
            }
        },
    );
    Ok(result)
}

pub async fn evaluate<TConfig: SandboxServerConfigImpl>(
    config: &TConfig,
    request: TConfig::RequestType,
    engine: &SandboxEngine,
) -> anyhow::Result<EvaluateResponse> {
    let EvaluateInput {
        script,
        args,
        config,
    } = config.get_evaluate_input(request);

    let stdout = MemoryOutputPipe::new(config.memory_limits.stdout_max_bytes);
    let stderr = MemoryOutputPipe::new(config.memory_limits.stderr_max_bytes);

    let mut sandbox = engine
        .build(SandboxConfig {
            ctx: WasiCtx::builder()
                .stdout(stdout.clone())
                .stderr(stderr.clone())
                .build(),
            cpu_fuel: config.cpu_fuel,
            memory_limits: config.memory_limits,
            http: config.http,
        })
        .await?;
    let result = sandbox.evaluate(&script, &args).await;

    Ok(EvaluateResponse {
        success: result.is_ok(),
        stdout: take_memory_pipe_contents(stdout),
        stderr: take_memory_pipe_contents(stderr),
        fuel_consumed: config.cpu_fuel.saturating_sub(sandbox.get_fuel_remaining()),
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
    })
}

fn take_memory_pipe_contents(pipe: MemoryOutputPipe) -> String {
    std::str::from_utf8(&pipe.contents())
        .map(|s| s.to_owned())
        .unwrap_or_else(|_| "<invalid utf8 output>".to_string())
}
