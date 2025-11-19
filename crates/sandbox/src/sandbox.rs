use std::fmt;
use std::net::SocketAddr;

use hyper::Uri;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, WasmBacktraceDetails};
use wasmtime_wasi::{ResourceTable, WasiCtx};
use wasmtime_wasi_http::WasiHttpCtx;

use crate::http::RequestValidationOutcome;
use crate::state::SandboxState;
use crate::{HttpMode, MemoryLimits};

mod bindings {
    // Generates bindings for the plugin world defined in the wit/plugin_implementation.wit file.
    wasmtime::component::bindgen!({
        world: "sandbox",
        path: "../../wit/sandbox.wit",
        exports: {
            default: async
        }
    });
}

pub struct SandboxConfig {
    /// Limit of CPU instructions that can be executed in this sandbox.
    pub cpu_fuel: u64,
    /// Limit the memory that can be allocated by the sandbox.
    pub memory_limits: MemoryLimits,
    /// Allow/block outbound http(s) requests.
    pub http: HttpMode,
    /// The context for WASI to define capabilities other than HTTP.
    ///
    /// Example:
    ///
    /// ```rust
    /// let stdout = MemoryOutputPipe::new(self.config.memory_limit_bytes);
    /// let ctx: WasiCtx = WasiCtx::builder()
    ///     .stdin(tokio::io::stdin())
    ///     .stdout(stdout.clone())
    ///     .stderr(tokio::io::stderr())
    ///     .build();
    /// ```
    pub ctx: WasiCtx,
}

pub struct SandboxEngine {
    engine: Engine,
    component: Component,
    linker: Linker<SandboxState>,
}

impl SandboxEngine {
    pub fn new() -> anyhow::Result<Self> {
        let mut engine_config = Config::new();
        // engine_config.cache_config_load_default().unwrap();
        engine_config.wasm_backtrace_details(WasmBacktraceDetails::Enable);
        engine_config.consume_fuel(true);
        engine_config.async_support(true);

        // An engine stores and configures global compilation settings like
        // optimization level, enabled wasm features, etc.
        let engine = Engine::new(&engine_config).unwrap();
        let mut linker: Linker<SandboxState> = Linker::new(&engine);

        // Wasi Provides support for accessing system APIs from the sandbox.
        // System APIs are only exposed based on the capabilities in the WasiCtx
        // on the store. We are enabling the APIs needed for Date.now() and Math.random()
        // to work from within JavaScript.
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

        let component = unsafe { Component::deserialize(&engine, include_bytes!("sandbox.bin"))? };

        Ok(Self {
            engine,
            component,
            linker,
        })
    }

    pub async fn build(&self, config: SandboxConfig) -> anyhow::Result<SandboxInstance> {
        let ctx = config.ctx;
        let mut store = Store::new(
            &self.engine,
            SandboxState {
                wasi_ctx: ctx,
                wasi_http: WasiHttpCtx::new(),
                resource_table: ResourceTable::default(),
                limits: config.memory_limits,
                http: config.http,
                max_requested_memory_bytes: None,
                max_requested_table_elements: None,
                requests: Default::default(),
            },
        );
        store.limiter(|s| s);
        store.set_fuel(config.cpu_fuel)?;

        let sandbox =
            bindings::Sandbox::instantiate_async(&mut store, &self.component, &self.linker).await?;

        Ok(SandboxInstance { store, sandbox })
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

pub struct SandboxInstance {
    sandbox: bindings::Sandbox,
    store: Store<SandboxState>,
}

impl SandboxInstance {
    pub fn get_fuel_remaining(&self) -> u64 {
        self.store.get_fuel().unwrap_or(0)
    }
    pub fn get_max_requested_memory_bytes(&self) -> Option<usize> {
        self.store.data().max_requested_memory_bytes
    }
    pub fn get_max_requested_table_elements(&self) -> Option<usize> {
        self.store.data().max_requested_table_elements
    }
    pub fn take_requests(&self) -> Vec<(Uri, Option<SocketAddr>, RequestValidationOutcome)> {
        self.store.data().requests.take()
    }
    pub async fn evaluate(
        &mut self,
        script: &str,
        parameters: &[serde_json::Value],
    ) -> Result<serde_json::Value, EvaluateError> {
        let parameters: Vec<_> = parameters
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<_, _>>()?;
        let result = self
            .sandbox
            .call_evaluate(&mut self.store, script, &parameters)
            .await;
        if result.is_err() && self.get_fuel_remaining() == 0 {
            return Err(EvaluateError::FuelExhausted);
        }
        let result = result?;
        let result = serde_json::from_str(&result?)?;
        Ok(result)
    }
}
