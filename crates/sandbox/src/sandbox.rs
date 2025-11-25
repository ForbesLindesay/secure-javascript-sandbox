use hyper::Uri;
use std::fmt;
use std::net::SocketAddr;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx};
use wasmtime_wasi_http::WasiHttpCtx;

use crate::state::SandboxState;
use crate::{HttpMode, MemoryLimits, RequestValidationOutcome};

macro_rules! impl_sandbox_engine {
    ($engine:ident, $instance:ident, $name:ident, $world:expr, $wit_path:expr, $file:expr, $enable_module_compiler:expr) => {
        mod bindings {
            // Generates bindings for the plugin world defined in the wit/plugin_implementation.wit file.
            wasmtime::component::bindgen!({
                world: $world,
                path: $wit_path,
                exports: {
                    default: async
                }
            });
        }

        pub struct $engine {
            engine: crate::sandbox::BaseSandboxEngine
        }

        impl $engine {
            pub fn new() -> anyhow::Result<Self> {
                let engine = crate::sandbox::BaseSandboxEngine::new(include_bytes!($file))?;
                Ok(Self { engine })
            }

            pub async fn build(&self, config: crate::sandbox::SandboxConfig) -> anyhow::Result<$instance> {
                let mut store = self.engine.build_store(config, $enable_module_compiler).await?;

                let sandbox =
                    bindings::$name::instantiate_async(&mut store, &self.engine.component, &self.engine.linker).await?;

                Ok($instance { store, sandbox })
            }
        }
        pub struct $instance {
            sandbox: bindings::$name,
            store: Store<SandboxState>,
        }
        impl crate::sandbox::SandboxInstanceBase for $instance {
            fn get_fuel_remaining(&self) -> u64 {
                self.store.get_fuel().unwrap_or(0)
            }
            fn get_max_requested_memory_bytes(&self) -> Option<usize> {
                self.store.data().max_requested_memory_bytes
            }
            fn get_max_requested_table_elements(&self) -> Option<usize> {
                self.store.data().max_requested_table_elements
            }
            fn take_requests(&self) -> Vec<(hyper::Uri, Option<std::net::SocketAddr>, crate::RequestValidationOutcome)> {
                self.store.data().requests.take()
            }
        }
    };
}
pub(crate) use impl_sandbox_engine;

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
    /// use secure_js_sandbox::{MemoryOutputPipe, WasiCtx};
    ///
    /// let stdout = MemoryOutputPipe::new(1024 * 1024); // 1 MB buffer
    /// let ctx: WasiCtx = WasiCtx::builder()
    ///     .stdin(tokio::io::stdin())
    ///     .stdout(stdout.clone())
    ///     .stderr(tokio::io::stderr())
    ///     .build();
    /// ```
    pub ctx: WasiCtx,
}

pub(crate) struct BaseSandboxEngine {
    engine: Engine,
    pub component: Component,
    pub linker: Linker<SandboxState>,
}

impl BaseSandboxEngine {
    pub fn new(bytes: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        let mut engine_config = Config::new();
        // engine_config.cache_config_load_default().unwrap();
        // engine_config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
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

        let component: Component = unsafe { Component::deserialize(&engine, bytes)? };

        Ok(Self {
            engine,
            component,
            linker,
        })
    }

    pub async fn build_store(
        &self,
        config: SandboxConfig,
        enable_module_compiler: bool,
    ) -> anyhow::Result<Store<SandboxState>> {
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
                enable_module_compiler,
            },
        );
        store.limiter(|s| s);
        store.set_fuel(config.cpu_fuel)?;

        Ok(store)
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

pub trait SandboxInstanceBase {
    fn get_fuel_remaining(&self) -> u64;
    fn get_max_requested_memory_bytes(&self) -> Option<usize>;
    fn get_max_requested_table_elements(&self) -> Option<usize>;
    fn take_requests(&self) -> Vec<(Uri, Option<SocketAddr>, RequestValidationOutcome)>;
}
