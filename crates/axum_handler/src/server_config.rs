use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::MethodRouter;
use serde::{Deserialize, de::DeserializeOwned};

use secure_js_sandbox::{
    ApiRequestBodyLimit, CpuFuel, CustomHttpMode, CustomImportMap, EvaluateMode, HttpMode,
    ImportMap, MemoryLimitBytes, MemoryLimits, MemorySizeBytes, RequestLimit, ResourceLimit,
    SandboxConfig, StaticImportSource, TableLimit,
};

use crate::env::get_env;
use crate::evaluate_request::{EvaluateRequest, EvaluateRequestWithConfig};

pub struct EvaluateInput<
    THttpMode: CustomHttpMode = HttpMode,
    TImportMap: CustomImportMap = ImportMap,
> {
    pub code: String,
    pub parameters: Vec<serde_json::Value>,
    pub config: SandboxConfig<THttpMode, TImportMap>,
}

pub trait CustomSandboxServerConfig<
    TRequestType,
    THttpMode: CustomHttpMode = HttpMode,
    TImportMap: CustomImportMap = ImportMap,
>: Send + Sync + 'static where
    TRequestType: DeserializeOwned + Send + 'static,
{
    fn get_api_request_body_limit(&self) -> ApiRequestBodyLimit;
    fn get_evaluate_input(&self, request: TRequestType) -> EvaluateInput<THttpMode, TImportMap>;
}

impl<
    TRequestType: DeserializeOwned + Send + 'static,
    THttpMode: CustomHttpMode,
    TImportMap: CustomImportMap,
    T: CustomSandboxServerConfig<TRequestType, THttpMode, TImportMap>,
> CustomSandboxServerConfig<TRequestType, THttpMode, TImportMap> for Arc<T>
{
    fn get_api_request_body_limit(&self) -> ApiRequestBodyLimit {
        self.as_ref().get_api_request_body_limit()
    }
    fn get_evaluate_input(&self, request: TRequestType) -> EvaluateInput<THttpMode, TImportMap> {
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
    #[serde(default = "default_trap_on_grow_failure")]
    pub trap_on_grow_failure: bool,
    #[serde(default)]
    pub stdout_max_bytes: MemorySizeBytes,
    #[serde(default)]
    pub stderr_max_bytes: MemorySizeBytes,
}
macro_rules! set_from_env {
    ($self:ident, $field:ident, $prefix:expr, $env_var:expr) => {
        if let Some(value) = get_env(&format!("{}_{}", $prefix, $env_var))? {
            $self.$field = value;
        }
    };
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
    pub(crate) fn set_from_env(&mut self, prefix: &str) -> anyhow::Result<()> {
        set_from_env!(self, memory_size_bytes, prefix, "MAX_MEMORY_BYTES");
        set_from_env!(self, table_elements, prefix, "MAX_TABLE_ELEMENTS");
        set_from_env!(self, instances, prefix, "MAX_INSTANCES");
        set_from_env!(self, tables, prefix, "MAX_TABLES");
        set_from_env!(self, memories, prefix, "MAX_MEMORIES");
        set_from_env!(self, trap_on_grow_failure, prefix, "TRAP_ON_GROW_FAILURE");
        set_from_env!(self, stdout_max_bytes, prefix, "STDOUT_MAX_BYTES");
        set_from_env!(self, stderr_max_bytes, prefix, "STDERR_MAX_BYTES");
        Ok(())
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

pub struct SandboxServerConfig<
    THttpMode: CustomHttpMode = HttpMode,
    TImportMap: CustomImportMap + Clone = ImportMap,
> {
    pub api_request_body_limit: ApiRequestBodyLimit,
    pub cpu_fuel: CpuFuel,
    pub memory_limits: SandboxServerMemoryLimits,
    pub http: THttpMode,
    pub request_limit: RequestLimit,
    pub import_map: TImportMap,
    pub sandbox_auto_strip_types: bool,
    pub module_method: Option<Box<str>>,
}

impl Default for SandboxServerConfig {
    fn default() -> Self {
        SandboxServerConfig {
            api_request_body_limit: Default::default(),
            cpu_fuel: Default::default(),
            memory_limits: Default::default(),
            http: Default::default(),
            request_limit: Default::default(),
            import_map: Default::default(),
            sandbox_auto_strip_types: false,
            module_method: None,
        }
    }
}

impl SandboxServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self {
            api_request_body_limit: api_request_body_limit_from_env()?,
            cpu_fuel: get_env("SANDBOX_CPU_FUEL")?.unwrap_or_default(),
            memory_limits: Default::default(),
            http: get_env("SANDBOX_HTTP_MODE")?.unwrap_or_default(),
            request_limit: get_env("SANDBOX_REQUEST_LIMIT")?.unwrap_or_default(),
            import_map: import_map_from_env()?,
            sandbox_auto_strip_types: get_env("SANDBOX_AUTO_STRIP_TYPES")?.unwrap_or(false),
            module_method: get_env::<String>("SANDBOX_MODULE_METHOD")?
                .map(|str| str.into_boxed_str()),
        };
        config.memory_limits.set_from_env("SANDBOX")?;
        Ok(config)
    }
}

impl<THttpMode: CustomHttpMode, TImportMap: CustomImportMap + Clone>
    CustomSandboxServerConfig<EvaluateRequest, THttpMode, TImportMap>
    for SandboxServerConfig<THttpMode, TImportMap>
{
    fn get_api_request_body_limit(&self) -> ApiRequestBodyLimit {
        self.api_request_body_limit
    }
    fn get_evaluate_input(&self, request: EvaluateRequest) -> EvaluateInput<THttpMode, TImportMap> {
        EvaluateInput {
            code: request.code,
            parameters: request.parameters,
            config: SandboxConfig {
                cpu_fuel: self.cpu_fuel,
                memory_limits: self.memory_limits.to_memory_limits(),
                http: self.http.clone(),
                imports: self.import_map.clone(),
                request_limit: self.request_limit,
                mode: match &self.module_method {
                    Some(method) => EvaluateMode::ModuleMethod(method.clone()),
                    None => EvaluateMode::FunctionCall,
                },
                strip_typescript_types: self.sandbox_auto_strip_types,
                filename: request.filename,
            },
        }
    }
}

pub struct AllowRequestToConfigureSandbox<TImportMap: CustomImportMap + Clone = ImportMap> {
    pub api_request_body_limit: ApiRequestBodyLimit,
    pub import_map: TImportMap,
}
impl Default for AllowRequestToConfigureSandbox {
    fn default() -> Self {
        AllowRequestToConfigureSandbox {
            api_request_body_limit: Default::default(),
            import_map: Default::default(),
        }
    }
}

impl AllowRequestToConfigureSandbox {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            api_request_body_limit: api_request_body_limit_from_env()?,
            import_map: import_map_from_env()?,
        })
    }
}
impl<TImportMap: CustomImportMap + Clone>
    CustomSandboxServerConfig<EvaluateRequestWithConfig, HttpMode, TImportMap>
    for AllowRequestToConfigureSandbox<TImportMap>
{
    fn get_api_request_body_limit(&self) -> ApiRequestBodyLimit {
        self.api_request_body_limit
    }
    fn get_evaluate_input(
        &self,
        request: EvaluateRequestWithConfig,
    ) -> EvaluateInput<HttpMode, TImportMap> {
        EvaluateInput {
            code: request.code,
            parameters: request.parameters,
            config: SandboxConfig {
                cpu_fuel: request.config.cpu_fuel,
                memory_limits: request.config.memory_limits.to_memory_limits(),
                http: request.config.http,
                imports: self.import_map.clone(),
                request_limit: request.config.request_limit,
                mode: match request.config.module_method {
                    Some(method) => EvaluateMode::ModuleMethod(method),
                    None => EvaluateMode::FunctionCall,
                },
                strip_typescript_types: request.config.sandbox_auto_strip_types,
                filename: request.filename,
            },
        }
    }
}

fn api_request_body_limit_from_env() -> anyhow::Result<ApiRequestBodyLimit> {
    get_env("SANDBOX_API_REQUEST_BODY_LIMIT_BYTES").map(|v| v.unwrap_or_default())
}

fn import_map_from_env() -> anyhow::Result<ImportMap> {
    if let Some(import_map_path) = get_env::<PathBuf>("SANDBOX_IMPORT_MAP_PATH")? {
        let import_map_path = import_map_path.canonicalize()?;
        let parent_dir = import_map_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Parent path has no parent directory"))?;
        let import_map_content = std::fs::read_to_string(&import_map_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read import map file {}: {}",
                import_map_path.display(),
                e
            )
        })?;
        let import_map: HashMap<String, String> = serde_json::from_str(&import_map_content)
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse import map file {}: {}",
                    import_map_path.display(),
                    e
                )
            })?;
        let import_map: HashMap<String, StaticImportSource> = import_map
            .into_iter()
            .map(|(key, path)| {
                StaticImportSource::parse_string(path, parent_dir).map(|path| (key, path))
            })
            .collect::<anyhow::Result<HashMap<String, StaticImportSource>>>()?;
        Ok(ImportMap::StaticImportMap(Arc::new(import_map)))
    } else {
        Ok(ImportMap::default())
    }
}

pub(crate) fn set_request_body_limit<T: Clone + Send + Sync + 'static>(
    router: MethodRouter<T>,
    api_request_body_limit: ApiRequestBodyLimit,
) -> MethodRouter<T> {
    let limit = match api_request_body_limit {
        ApiRequestBodyLimit::Limited(bytes) => DefaultBodyLimit::max(bytes),
        ApiRequestBodyLimit::Unbounded => DefaultBodyLimit::disable(),
    };
    router.layer(limit)
}
