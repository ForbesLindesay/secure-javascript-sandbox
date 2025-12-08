#![deny(warnings)]

mod http;
mod imports;
mod ip_utils;
mod limit_values;
mod memory;
mod sandbox;
mod state;
mod tsutils;

pub use http::{CustomHttpMode, HttpMode, RequestValidationOutcome};
pub use hyper::{Request, Uri};

pub use imports::{CustomImportMap, ImportMap, ResolvedModule, StaticImportSource};
pub use limit_values::{
    CpuFuel, MemoryLimitBytes, MemorySizeBytes, RequestLimit, ResourceLimit, TableLimit,
};
pub use memory::MemoryLimits;
pub use sandbox::{EvaluateError, EvaluateMode, SandboxConfig, SandboxEngine};
pub use tsutils::{
    ModuleExport, StaticImport, StaticImportUsage, TsUtilsEngine, TsUtilsEvaluateError,
    TsUtilsSandboxConfig, TsUtilsSandboxInstance, ValidateModuleMode,
};
pub use wasmtime::{StoreLimits, StoreLimitsBuilder};
pub use wasmtime_wasi::WasiCtx;
pub use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
pub use wasmtime_wasi_http::types::OutgoingRequestConfig;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::*;

    #[tokio::test]
    async fn test_simple_function() {
        let engine = SandboxEngine::new().unwrap();
        let result = engine
            .evaluate(
                "function (a, b) { return a + b; }",
                &vec![json!(40), json!(2)],
                Default::default(),
            )
            .await
            .result
            .unwrap();
        assert_eq!(result, json!(42));
    }

    #[tokio::test]
    async fn test_async_function() {
        let engine = SandboxEngine::new().unwrap();
        let result = engine.evaluate(
            "async function (a, b) { await new Promise(r => setTimeout(r, 100)); return a + b; }",
            &vec![json!(40), json!(2)],
            Default::default(),
        ).await.result.unwrap();
        assert_eq!(result, json!(42));
    }

    #[tokio::test]
    async fn test_typescript_function() {
        let engine = SandboxEngine::new().unwrap();
        let result = engine
            .evaluate(
                "function (a: number, b: number): number { return a + b; }",
                &vec![json!(40), json!(2)],
                SandboxConfig {
                    strip_typescript_types: true,
                    ..Default::default()
                },
            )
            .await
            .result
            .unwrap();
        assert_eq!(result, json!(42));
    }

    #[tokio::test]
    async fn test_simple_module() {
        let engine = SandboxEngine::new().unwrap();
        let result = engine
            .evaluate(
                "export function run(a, b) { return a + b; }",
                &vec![json!(40), json!(2)],
                SandboxConfig {
                    mode: EvaluateMode::ModuleMethod("run".into()),
                    ..Default::default()
                },
            )
            .await
            .result
            .unwrap();
        assert_eq!(result, json!(42));
    }

    #[tokio::test]
    async fn test_typescript_module() {
        let engine = SandboxEngine::new().unwrap();
        let result = engine
            .evaluate(
                "export function run(a: number, b: number): number { return a + b; }",
                &vec![json!(40), json!(2)],
                SandboxConfig {
                    mode: EvaluateMode::ModuleMethod("run".into()),
                    strip_typescript_types: true,
                    ..Default::default()
                },
            )
            .await
            .result
            .unwrap();
        assert_eq!(result, json!(42));
    }

    #[tokio::test]
    async fn test_typescript_module_import() {
        let config = SandboxConfig {
            mode: EvaluateMode::ModuleMethod("run".into()),
            strip_typescript_types: true,
            http: HttpMode::AllowAll,
            ..SandboxConfig::default()
        };
        let engine = SandboxEngine::new().unwrap();
        let code = "import * as ft from 'https://unpkg.com/funtypes@5.1.2/lib/index.mjs'; export function run(input: string): number { const result = ft.Array(ft.String).safeParse(JSON.parse(input)); return result.success ? result : { success: false, reason: ft.showError(result) };}";
        let result = engine
            .evaluate(code, &vec![json!("[\"a\", \"b\", \"c\"]")], config.clone())
            .await
            .result
            .unwrap();
        assert_eq!(result, json!({ "success": true, "value": ["a", "b", "c"] }));
        let result = engine
            .evaluate(code, &vec![json!("[\"a\", 42, \"c\"]")], config)
            .await
            .result
            .unwrap();
        assert_eq!(
            result,
            json!({ "success": false, "reason": "Unable to assign [\"a\", 42, \"c\"] to string[]\n  The types of [1] are not compatible\n    Expected string, but was 42" })
        );
    }

    // Fetching from data URIs is currently not supported.
    // #[tokio::test]
    // async fn test_fetch_data_uri() {
    //     let config = SandboxConfig {
    //         http: HttpMode::AllowAll,
    //         ..SandboxConfig::default()
    //     };
    //     let engine = SandboxEngine::new().unwrap();
    //     let mut instance = engine.build(config).await.unwrap();
    //     let result = instance.evaluate(FunctionInput {
    //         code: "async function () { const res = await fetch('data:text/html,hello world'); return await res.text(); }".to_string(),
    //         parameters: vec![],
    //         strip_typescript_types: false,
    //     }).await;
    //     println!("requests: {:?}", instance.take_requests());
    //     let result = result.unwrap();
    //     assert_eq!(result, json!("hello world"));
    // }
}
