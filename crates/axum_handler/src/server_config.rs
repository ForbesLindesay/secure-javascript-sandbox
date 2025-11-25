use std::sync::Arc;

use serde::{Deserialize, de::DeserializeOwned};

use secure_js_sandbox::{HttpMode, MemoryLimits};

use crate::{
    EvaluateFunctionInput, EvaluateFunctionRequest, EvaluateFunctionRequestWithConfig,
    EvaluateModuleInput, EvaluateModuleRequest, EvaluateModuleRequestWithConfig, get_env,
};

fn cpu_fuel_default() -> u64 {
    440_000_000 // Approximately 100ms on my MacBook Pro.
}

pub trait SandboxServerConfigTrait<TEvaluateInput>: Send + Sync + 'static {
    type RequestType: DeserializeOwned + Send + 'static;
    fn get_evaluate_input(&self, request: Self::RequestType) -> TEvaluateInput;
}

impl<TEvaluateInput, T: SandboxServerConfigTrait<TEvaluateInput>>
    SandboxServerConfigTrait<TEvaluateInput> for Arc<T>
{
    type RequestType = T::RequestType;
    fn get_evaluate_input(&self, request: T::RequestType) -> TEvaluateInput {
        (**self).get_evaluate_input(request)
    }
}

#[derive(Clone, Deserialize)]
pub struct SandboxServerConfig {
    #[serde(default = "cpu_fuel_default")]
    pub cpu_fuel: u64,
    #[serde(default)]
    pub memory_limits: MemoryLimits,
    #[serde(default)]
    pub http: HttpMode,
    #[serde(default)]
    pub enable_typescript_support: bool,
}

impl Default for SandboxServerConfig {
    fn default() -> Self {
        SandboxServerConfig {
            cpu_fuel: cpu_fuel_default(),
            memory_limits: MemoryLimits::default(),
            http: HttpMode::BlockAll,
            enable_typescript_support: false,
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
        if let Some(enable_typescript_support) = get_env("SANDBOX_TYPESCRIPT_SUPPORT")? {
            config.enable_typescript_support = enable_typescript_support;
        }

        Ok(config)
    }
}

impl SandboxServerConfigTrait<EvaluateFunctionInput> for SandboxServerConfig {
    type RequestType = EvaluateFunctionRequest;

    fn get_evaluate_input(&self, request: Self::RequestType) -> EvaluateFunctionInput {
        EvaluateFunctionInput {
            script: request.script,
            args: request.args,
            config: self.clone(),
        }
    }
}

impl SandboxServerConfigTrait<EvaluateModuleInput> for SandboxServerConfig {
    type RequestType = EvaluateModuleRequest;

    fn get_evaluate_input(&self, request: Self::RequestType) -> EvaluateModuleInput {
        EvaluateModuleInput {
            code: request.code,
            method: request.method,
            args: request.args,
            config: self.clone(),
        }
    }
}

pub struct AllowRequestToConfigureSandbox;
impl SandboxServerConfigTrait<EvaluateFunctionInput> for AllowRequestToConfigureSandbox {
    type RequestType = EvaluateFunctionRequestWithConfig;

    fn get_evaluate_input(&self, request: Self::RequestType) -> EvaluateFunctionInput {
        EvaluateFunctionInput {
            script: request.script,
            args: request.args,
            config: request.config,
        }
    }
}

impl SandboxServerConfigTrait<EvaluateModuleInput> for AllowRequestToConfigureSandbox {
    type RequestType = EvaluateModuleRequestWithConfig;

    fn get_evaluate_input(&self, request: Self::RequestType) -> EvaluateModuleInput {
        EvaluateModuleInput {
            code: request.code,
            method: request.method,
            args: request.args,
            config: request.config,
        }
    }
}
