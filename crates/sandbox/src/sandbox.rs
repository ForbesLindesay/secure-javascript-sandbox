use std::fmt;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{ResourceTable, WasiCtx};
use wasmtime_wasi_http::WasiHttpCtx;

use crate::state::SandboxState;
use crate::{
    CpuFuel, CustomHttpMode, CustomImportMap, HttpMode, ImportMap, MemoryLimits, RequestLimit,
};

mod bindings {
    wasmtime::component::bindgen!({
        path: "src/sandbox",
        imports: {
            "local:host": async
        },
        exports: {
            default: async
        }
    });
}

pub(crate) use bindings::local::host::host_impl::Host;
pub use bindings::local::host::host_impl::ResolvedModule;

#[derive(Clone)]
pub struct SandboxConfig<
    THttpMode: CustomHttpMode = HttpMode,
    TImportMap: CustomImportMap = ImportMap,
> {
    /// Limit of CPU instructions that can be executed in this sandbox.
    pub cpu_fuel: CpuFuel,
    /// Limit the memory that can be allocated by the sandbox.
    pub memory_limits: MemoryLimits,
    /// Allow/block outbound http(s) requests.
    pub http: THttpMode,
    pub imports: TImportMap,
    /// Limit the number of outbound HTTP requests that can be made.
    pub request_limit: RequestLimit,
    /// Evaluate as a module by calling an exported method, or as a function expression.
    pub mode: EvaluateMode,
    /// Whether to strip TypeScript type annotations from the code before evaluating - if it's a module, only the initial module has types stripped.
    pub strip_typescript_types: bool,
}
impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            cpu_fuel: Default::default(),
            memory_limits: Default::default(),
            http: Default::default(),
            imports: Default::default(),
            request_limit: Default::default(),
            mode: Default::default(),
            strip_typescript_types: false,
        }
    }
}

pub struct SandboxEngine<
    THttpMode: CustomHttpMode = HttpMode,
    TImportMap: CustomImportMap = ImportMap,
> {
    engine: Engine,
    component: Component,
    linker: Linker<SandboxState<TImportMap, THttpMode>>,
}

impl<THttpMode: CustomHttpMode, TImportMap: CustomImportMap> SandboxEngine<THttpMode, TImportMap> {
    pub fn new() -> anyhow::Result<Self> {
        let mut engine_config = Config::new();
        // engine_config.cache_config_load_default().unwrap();
        // engine_config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        engine_config.consume_fuel(true);
        engine_config.async_support(true);

        // An engine stores and configures global compilation settings like
        // optimization level, enabled wasm features, etc.
        let engine = Engine::new(&engine_config).unwrap();
        let mut linker: Linker<SandboxState<TImportMap, THttpMode>> = Linker::new(&engine);

        // Wasi Provides support for accessing system APIs from the sandbox.
        // System APIs are only exposed based on the capabilities in the WasiCtx
        // on the store. We are enabling the APIs needed for Date.now() and Math.random()
        // to work from within JavaScript.
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;
        bindings::local::host::host_impl::add_to_linker::<
            SandboxState<TImportMap, THttpMode>,
            SandboxState<TImportMap, THttpMode>,
        >(&mut linker, |s| s)?;

        let component: Component =
            unsafe { Component::deserialize(&engine, include_bytes!("sandbox/sandbox.bin"))? };

        Ok(Self {
            engine,
            component,
            linker,
        })
    }

    async fn build(
        &self,
        cpu_fuel: CpuFuel,
        memory_limits: MemoryLimits,
        http: THttpMode,
        imports: TImportMap,
        request_limit: RequestLimit,
    ) -> anyhow::Result<SandboxInstance<THttpMode, TImportMap>> {
        let stdout = MemoryOutputPipe::new(memory_limits.stdout_bytes.into());
        let stderr = MemoryOutputPipe::new(memory_limits.stderr_bytes.into());
        let ctx: WasiCtx = WasiCtx::builder()
            .stdout(stdout.clone())
            .stderr(stderr.clone())
            .build();
        let mut store = Store::new(
            &self.engine,
            SandboxState {
                wasi_ctx: ctx,
                wasi_http: WasiHttpCtx::new(),
                resource_table: ResourceTable::default(),
                memory_limits,
                http,
                imports,
                request_limit,
                max_requested_memory_bytes: None,
                max_requested_table_elements: None,
                requests: Default::default(),
                request_count: 0,
            },
        );
        store.limiter(|s| s);
        store.set_fuel(cpu_fuel.into())?;
        let sandbox =
            bindings::Root::instantiate_async(&mut store, &self.component, &self.linker).await?;
        Ok(SandboxInstance {
            store,
            sandbox,
            stdout,
            stderr,
        })
    }
    pub async fn evaluate(
        &self,
        code: &str,
        parameters: &[serde_json::Value],
        config: SandboxConfig<THttpMode, TImportMap>,
    ) -> SandboxEvaluationResult {
        match self
            .build(
                config.cpu_fuel,
                config.memory_limits,
                config.http,
                config.imports,
                config.request_limit,
            )
            .await
        {
            Ok(instance) => {
                instance
                    .evaluate(
                        code,
                        parameters,
                        &EvaluateOptions {
                            mode: config.mode,
                            strip_typescript_types: config.strip_typescript_types,
                        },
                    )
                    .await
            }
            Err(err) => SandboxEvaluationResult {
                result: Err(EvaluateError::WasmError(err)),
                stdout: String::new(),
                stderr: String::new(),
                fuel_remaining: 0,
                max_requested_memory_bytes: None,
                max_requested_table_elements: None,
                outbound_requests: vec![],
            },
        }
    }
}

#[derive(Debug)]
pub enum EvaluateError {
    FuelExhausted,
    JavaScriptError(String),
    WasmError(wasmtime::Error),
    JsonError(serde_json::Error),
}

impl fmt::Display for EvaluateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EvaluateError::FuelExhausted => write!(f, "CPU fuel exhausted"),
            EvaluateError::JavaScriptError(msg) => write!(f, "JavaScript error: {}", msg),
            EvaluateError::WasmError(err) => write!(f, "Wasm error: {}", err),
            EvaluateError::JsonError(err) => write!(f, "JSON error: {}", err),
        }
    }
}
impl From<String> for EvaluateError {
    fn from(msg: String) -> Self {
        EvaluateError::JavaScriptError(msg)
    }
}
impl From<wasmtime::Error> for EvaluateError {
    fn from(err: wasmtime::Error) -> Self {
        EvaluateError::WasmError(err)
    }
}
impl From<serde_json::Error> for EvaluateError {
    fn from(err: serde_json::Error) -> Self {
        EvaluateError::JsonError(err)
    }
}

struct SandboxInstance<THttpMode: CustomHttpMode, TImportMap: CustomImportMap> {
    sandbox: bindings::Root,
    store: wasmtime::Store<crate::state::SandboxState<TImportMap, THttpMode>>,
    stdout: MemoryOutputPipe,
    stderr: MemoryOutputPipe,
}
impl<THttpMode: CustomHttpMode, TImportMap: CustomImportMap>
    SandboxInstance<THttpMode, TImportMap>
{
    fn handle_result(self, result: Result<(), anyhow::Error>) -> SandboxEvaluationResult {
        let full_stdout = take_memory_pipe_contents(self.stdout);
        let full_stderr = take_memory_pipe_contents(self.stderr);
        let mut stdout = full_stdout.split("73914D86-55DF-495D-BAD5-B45D571D154D");
        let stdout_str = stdout.next();
        let result_str = stdout.next().and_then(|result_str| {
            result_str
                .split("8C47F950-3E81-46B1-976E-177A89380038")
                .next()
        });
        let mut stderr = full_stderr.split("E8FEE14A-BBF5-4B08-9E00-6D61189D897D");
        let stderr_str = stderr.next();
        let error_str = stderr.next();

        let result = if result.is_err() && self.store.get_fuel().unwrap_or(0) == 0 {
            Err(crate::EvaluateError::FuelExhausted)
        } else if let Some(error_str) = error_str {
            Err(crate::EvaluateError::JavaScriptError(
                error_str.trim_ascii().to_string(),
            ))
        } else if let Some(result_str) = result_str {
            serde_json::from_str(result_str).map_err(Into::into)
        } else if let Some(err) = result.err() {
            Err(crate::EvaluateError::WasmError(err))
        } else {
            Err(crate::EvaluateError::JavaScriptError(
                "No result returned from sandbox".to_string(),
            ))
        };
        SandboxEvaluationResult {
            result,
            stdout: stdout_str.unwrap_or("").to_string(),
            stderr: stderr_str.unwrap_or("").to_string(),
            fuel_remaining: self.store.get_fuel().unwrap_or(0),
            max_requested_memory_bytes: self.store.data().max_requested_memory_bytes,
            max_requested_table_elements: self.store.data().max_requested_table_elements,
            outbound_requests: self.store.data().requests.take(),
        }
    }
    pub async fn evaluate(
        mut self,
        code: &str,
        parameters: &[serde_json::Value],
        options: &EvaluateOptions,
    ) -> SandboxEvaluationResult {
        let parameters = match prepare_parameters(&parameters) {
            Ok(params) => params,
            Err(err) => {
                return SandboxEvaluationResult {
                    result: Err(err),
                    stdout: String::new(),
                    stderr: String::new(),
                    fuel_remaining: self.store.get_fuel().unwrap_or(0),
                    max_requested_memory_bytes: self.store.data().max_requested_memory_bytes,
                    max_requested_table_elements: self.store.data().max_requested_table_elements,
                    outbound_requests: self.store.data().requests.take(),
                };
            }
        };
        let result = match &options.mode {
            EvaluateMode::FunctionCall => {
                self.sandbox
                    .call_evaluate(
                        &mut self.store,
                        &code,
                        &parameters,
                        options.strip_typescript_types,
                    )
                    .await
            }
            EvaluateMode::ModuleMethod(method) => {
                self.sandbox
                    .call_evaluate_module(
                        &mut self.store,
                        &code,
                        &method,
                        &parameters,
                        options.strip_typescript_types,
                    )
                    .await
            }
        };
        self.handle_result(result)
    }
}

pub struct SandboxEvaluationResult {
    pub result: Result<serde_json::Value, EvaluateError>,
    pub stdout: String,
    pub stderr: String,
    pub fuel_remaining: u64,
    pub max_requested_memory_bytes: Option<usize>,
    pub max_requested_table_elements: Option<usize>,
    pub outbound_requests: Vec<(
        hyper::Uri,
        Option<std::net::SocketAddr>,
        crate::RequestValidationOutcome,
    )>,
}
fn prepare_parameters(parameters: &[serde_json::Value]) -> Result<Vec<String>, EvaluateError> {
    let parameters: Vec<_> = parameters
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<_, _>>()?;
    Ok(parameters)
}

fn take_memory_pipe_contents(pipe: MemoryOutputPipe) -> String {
    std::str::from_utf8(&pipe.contents())
        .map(|s| s.to_owned())
        .unwrap_or_else(|_| "<invalid utf8 output>".to_string())
}

// #[non_exhaustive]
#[derive(Clone)]
struct EvaluateOptions {
    pub mode: EvaluateMode,
    pub strip_typescript_types: bool,
}

impl Default for EvaluateOptions {
    fn default() -> Self {
        Self {
            mode: EvaluateMode::FunctionCall,
            strip_typescript_types: false,
        }
    }
}

#[derive(Clone)]
pub enum EvaluateMode {
    ModuleMethod(Box<str>),
    FunctionCall,
}
impl Default for EvaluateMode {
    fn default() -> Self {
        EvaluateMode::FunctionCall
    }
}
