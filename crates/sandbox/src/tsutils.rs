use serde::Deserialize;
use std::fmt;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx};
use wasmtime_wasi_http::WasiHttpCtx;

use crate::http::BlockAllHttp;
use crate::imports::ImportMapBlockAll;
use crate::state::{SandboxHttpState, SandboxState};
use crate::{CpuFuel, MemoryLimits, RequestLimit};

mod bindings {
    wasmtime::component::bindgen!({
        path: "src/tsutils",
        exports: {
            default: async
        }
    });
}
pub use bindings::exports::local::ts_utils::ts_utils_impl::{ModuleExport, StaticImport};

#[derive( Clone, Copy, Default, Debug)]
pub enum ValidateModuleMode {
    #[default]
    JavaScript,
    TypeScript,
}
impl<'a> Deserialize<'a> for ValidateModuleMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        match s {
            "JAVASCRIPT" => Ok(ValidateModuleMode::JavaScript),
            "TYPESCRIPT" => Ok(ValidateModuleMode::TypeScript),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid ValidateModuleMode: {}",
                s
            ))),
        }
    }
}

pub struct ValidateModuleResult {
    pub has_dynamic_import: bool,
    pub static_imports: Vec<StaticImport>,
    pub exports: Vec<ModuleExport>,
}

#[derive(Clone, Default)]
pub struct TsUtilsSandboxConfig {
    /// Limit of CPU instructions that can be executed in this sandbox.
    pub cpu_fuel: CpuFuel,
    /// Limit the memory that can be allocated by the sandbox.
    pub memory_limits: MemoryLimits,
}

pub struct TsUtilsEngine {
    engine: Engine,
    component: Component,
    linker: Linker<SandboxState<ImportMapBlockAll, BlockAllHttp>>,
}

impl TsUtilsEngine {
    pub fn new() -> anyhow::Result<Self> {
        let mut engine_config = Config::new();
        // engine_config.cache_config_load_default().unwrap();
        // engine_config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        engine_config.consume_fuel(true);

        // An engine stores and configures global compilation settings like
        // optimization level, enabled wasm features, etc.
        let engine = Engine::new(&engine_config).unwrap();
        let mut linker: Linker<SandboxState<ImportMapBlockAll, BlockAllHttp>> =
            Linker::new(&engine);

        // Wasi Provides support for accessing system APIs from the sandbox.
        // System APIs are only exposed based on the capabilities in the WasiCtx
        // on the store. We are enabling the APIs needed for Date.now() and Math.random()
        // to work from within JavaScript.
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        wasmtime_wasi_http::p2::add_only_http_to_linker_async(&mut linker)?;

        let component: Component =
            unsafe { Component::deserialize(&engine, include_bytes!("tsutils/tsutils.bin"))? };

        Ok(Self {
            engine,
            component,
            linker,
        })
    }

    pub async fn build(
        &self,
        config: TsUtilsSandboxConfig,
    ) -> wasmtime::Result<TsUtilsSandboxInstance> {
        let ctx: WasiCtx = WasiCtx::builder().inherit_stderr().inherit_stdout().build();
        let mut store = Store::new(
            &self.engine,
            SandboxState {
                wasi_ctx: ctx,
                wasi_http: WasiHttpCtx::new(),
                resource_table: ResourceTable::default(),
                memory_limits: config.memory_limits,
                http: SandboxHttpState {
                    http: BlockAllHttp,
                    request_limit: RequestLimit::Limited(0),
                    requests: Default::default(),
                    request_count: 0,
                },
                imports: ImportMapBlockAll,
                max_requested_memory_bytes: None,
                max_requested_table_elements: None,
            },
        );
        store.limiter(|s| s);
        store.set_fuel(config.cpu_fuel.into())?;
        let sandbox =
            bindings::TsUtils::instantiate_async(&mut store, &self.component, &self.linker).await?;
        Ok(TsUtilsSandboxInstance { store, sandbox })
    }
}

#[derive(Debug)]
pub enum TsUtilsEvaluateError {
    FuelExhausted,
    JavaScriptError(String),
    WasmError(wasmtime::Error),
}

impl fmt::Display for TsUtilsEvaluateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TsUtilsEvaluateError::FuelExhausted => write!(f, "CPU fuel exhausted"),
            TsUtilsEvaluateError::JavaScriptError(msg) => write!(f, "{}", msg),
            TsUtilsEvaluateError::WasmError(err) => write!(f, "Wasm error: {}", err),
        }
    }
}
impl From<String> for TsUtilsEvaluateError {
    fn from(msg: String) -> Self {
        TsUtilsEvaluateError::JavaScriptError(msg.trim_ascii().into())
    }
}
impl From<wasmtime::Error> for TsUtilsEvaluateError {
    fn from(err: wasmtime::Error) -> Self {
        TsUtilsEvaluateError::WasmError(err)
    }
}
pub struct TsUtilsSandboxInstance {
    sandbox: bindings::TsUtils,
    store: wasmtime::Store<crate::state::SandboxState<ImportMapBlockAll, BlockAllHttp>>,
}
impl TsUtilsSandboxInstance {
    pub fn set_fuel(&mut self, fuel: CpuFuel) -> anyhow::Result<()> {
        self.store.set_fuel(fuel.into())?;
        Ok(())
    }
    pub fn get_fuel_remaining(&self) -> u64 {
        self.store.get_fuel().unwrap_or(0)
    }
    pub async fn strip_types(
        &mut self,
        code: &str,
        filename: Option<&str>,
    ) -> Result<String, TsUtilsEvaluateError> {
        let result = self
            .sandbox
            .local_ts_utils_ts_utils_impl()
            .call_strip_types(&mut self.store, code, filename)
            .await;
        if result.is_err() && self.store.get_fuel().unwrap_or(0) == 0 {
            return Err(TsUtilsEvaluateError::FuelExhausted);
        }
        let result = result?;
        Ok(result?)
    }
    pub async fn validate_module(
        &mut self,
        code: &str,
        mode: ValidateModuleMode,
        filename: Option<&str>,
    ) -> Result<ValidateModuleResult, TsUtilsEvaluateError> {
        let sandbox = self.sandbox.local_ts_utils_ts_utils_impl();
        let result = match mode {
            ValidateModuleMode::JavaScript => {
                sandbox
                    .call_compile_module(&mut self.store, code, filename)
                    .await
            }
            ValidateModuleMode::TypeScript => {
                sandbox
                    .call_strip_types_and_compile_module(&mut self.store, code, filename)
                    .await
            }
        };
        if result.is_err() && self.store.get_fuel().unwrap_or(0) == 0 {
            return Err(TsUtilsEvaluateError::FuelExhausted);
        }
        let result = result?;
        let result = result?;
        Ok(ValidateModuleResult {
            has_dynamic_import: result.has_dynamic_import,
            static_imports: result.static_imports,
            exports: result.exports,
        })
    }
}
