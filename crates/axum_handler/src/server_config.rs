use std::sync::Arc;

use serde::{Deserialize, de::DeserializeOwned};

use secure_js_sandbox::{
    EvaluateMode, HttpMode, MemoryLimitBytes, MemoryLimits, MemorySizeBytes,
    ResourceLimit, SandboxConfig, TableLimit,
};

use crate::env::get_env;
use crate::evaluate_request::{EvaluateRequest, EvaluateRequestWithConfig};

fn cpu_fuel_default() -> u64 {
    440_000_000 // Approximately 100ms on my MacBook Pro.
}

pub struct EvaluateInput {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
    pub config: SandboxConfig,
}

pub trait SandboxServerConfigTrait<TRequestType>: Send + Sync + 'static
where
    TRequestType: DeserializeOwned + Send + 'static,
{
    fn get_evaluate_input(&self, request: TRequestType) -> EvaluateInput;
}

impl<TRequestType: DeserializeOwned + Send + 'static, T: SandboxServerConfigTrait<TRequestType>>
    SandboxServerConfigTrait<TRequestType> for Arc<T>
{
    fn get_evaluate_input(&self, request: TRequestType) -> EvaluateInput {
        self.as_ref().get_evaluate_input(request)
    }
}

fn default_trap_on_grow_failure() -> bool {
    false
}

#[derive(Clone, Debug, Deserialize)]
pub struct SandboxServerMemoryLimits {
    #[serde(default)]
    pub memory_size_bytes: MemoryLimitBytes,
    #[serde(default)]
    pub table_elements: TableLimit,
    #[serde(default)]
    pub instances: ResourceLimit,
    #[serde(default)]
    pub tables: ResourceLimit,
    #[serde(default)]
    pub memories: ResourceLimit,
    #[serde(default="default_trap_on_grow_failure")]
    pub trap_on_grow_failure: bool,
    #[serde(default)]
    pub stdout_max_bytes: MemorySizeBytes,
    #[serde(default)]
    pub stderr_max_bytes: MemorySizeBytes,
}
impl SandboxServerMemoryLimits {
    pub fn to_memory_limits(&self) -> MemoryLimits {
        MemoryLimits {
            memory_size_bytes: self.memory_size_bytes,
            table_elements: self.table_elements,
            instances: self.instances,
            tables: self.tables,
            memories: self.memories,
            trap_on_grow_failure: false,
            stderr_bytes: self.stderr_max_bytes,
            stdout_bytes: self.stdout_max_bytes,
        }
    }
}
impl Default for SandboxServerMemoryLimits {
    fn default() -> Self {
        SandboxServerMemoryLimits {
            memory_size_bytes: Default::default(),
            table_elements: Default::default(),
            instances: Default::default(),
            tables: Default::default(),
            memories: Default::default(),
            trap_on_grow_failure: Default::default(),
            stdout_max_bytes: Default::default(),
            stderr_max_bytes: Default::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct SandboxServerConfig {
    #[serde(default = "cpu_fuel_default")]
    pub cpu_fuel: u64,
    #[serde(default)]
    pub memory_limits: SandboxServerMemoryLimits,
    #[serde(default)]
    pub http: HttpMode,
    #[serde(default)]
    pub enable_typescript_support: bool,
    #[serde(default)]
    pub module_method: Option<Box<str>>,
}

impl Default for SandboxServerConfig {
    fn default() -> Self {
        SandboxServerConfig {
            cpu_fuel: cpu_fuel_default(),
            memory_limits: SandboxServerMemoryLimits::default(),
            http: HttpMode::BlockAll,
            enable_typescript_support: false,
            module_method: None,
        }
    }
}

impl SandboxServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        if let Some(cpu_fuel) = get_env("SANDBOX_CPU_FUEL")? {
            config.cpu_fuel = cpu_fuel;
        }
        if let Some(max_memory_bytes) = get_env("SANDBOX_MAX_MEMORY_BYTES")? {
            config.memory_limits.memory_size_bytes = max_memory_bytes;
        }
        if let Some(max_table_elements) = get_env("SANDBOX_MAX_TABLE_ELEMENTS")? {
            config.memory_limits.table_elements = max_table_elements;
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
        if let Some(enable_typescript_support) = get_env("SANDBOX_TYPESCRIPT_SUPPORT")? {
            config.enable_typescript_support = enable_typescript_support;
        }
        if let Some(module_method) = get_env::<String>("SANDBOX_MODULE_METHOD")? {
            config.module_method = Some(module_method.into_boxed_str());
        }

        Ok(config)
    }
}

impl SandboxServerConfigTrait<EvaluateRequest> for SandboxServerConfig {
    fn get_evaluate_input(&self, request: EvaluateRequest) -> EvaluateInput {
        EvaluateInput {
            code: request.code,
            parameters: request.parameters,
            config: SandboxConfig {
                cpu_fuel: self.cpu_fuel,
                memory_limits: self.memory_limits.to_memory_limits(),
                http: self.http.clone(),
                mode: match &self.module_method {
                    Some(method) => EvaluateMode::ModuleMethod(method.clone()),
                    None => EvaluateMode::FunctionCall,
                },
                strip_typescript_types: self.enable_typescript_support,
            },
        }
    }
}

pub struct AllowRequestToConfigureSandbox;
impl SandboxServerConfigTrait<EvaluateRequestWithConfig> for AllowRequestToConfigureSandbox {
    fn get_evaluate_input(
        &self,
        request: EvaluateRequestWithConfig,
    ) -> EvaluateInput {
        EvaluateInput {
            code: request.code,
            parameters: request.parameters,
            config: SandboxConfig {
                cpu_fuel: request.config.cpu_fuel,
                memory_limits: request.config.memory_limits.to_memory_limits(),
                http: request.config.http,
                mode: match request.config.module_method {
                    Some(method) => EvaluateMode::ModuleMethod(method),
                    None => EvaluateMode::FunctionCall,
                },
                strip_typescript_types: request.config.enable_typescript_support,
            },
        }
    }
}
