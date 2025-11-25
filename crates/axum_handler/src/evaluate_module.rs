use axum::{
    Json,
    routing::{MethodRouter, post},
};
use secure_js_sandbox::ModuleSandboxEngine;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    EvaluateResponse, SandboxServerConfig, SandboxServerConfigTrait,
    evaluate_response::EvaluateResponseBuilder,
};

pub struct EvaluateModuleInput {
    pub code: String,
    pub method: String,
    pub args: Vec<serde_json::Value>,
    pub config: SandboxServerConfig,
}

#[derive(Deserialize)]
pub struct EvaluateModuleRequest {
    pub code: String,
    pub method: String,
    pub args: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct EvaluateModuleRequestWithConfig {
    pub code: String,
    pub method: String,
    pub args: Vec<serde_json::Value>,
    #[serde(default)]
    pub config: SandboxServerConfig,
}

pub async fn create_evaluate_module_handler<
    TConfig: SandboxServerConfigTrait<EvaluateModuleInput>,
    T: Clone + Send + Sync + 'static,
>(
    config: TConfig,
) -> anyhow::Result<MethodRouter<T>> {
    let config = Arc::new(config);
    let engine = Arc::new(ModuleSandboxEngine::new()?);
    let result: MethodRouter<T> = post(
        async move |Json(request): Json<TConfig::RequestType>| -> Json<serde_json::Value> {
            match evaluate_module(&config, request, &engine).await {
                Ok(response) => Json(serde_json::to_value(response).unwrap()),
                Err(err) => Json(serde_json::json!({"error": err.to_string()})),
            }
        },
    );
    Ok(result)
}

pub async fn evaluate_module<TConfig: SandboxServerConfigTrait<EvaluateModuleInput>>(
    config: &TConfig,
    request: TConfig::RequestType,
    engine: &ModuleSandboxEngine,
) -> anyhow::Result<EvaluateResponse> {
    let EvaluateModuleInput {
        code,
        method,
        args,
        config,
    } = config.get_evaluate_input(request);
    let code = if config.enable_typescript_support {
        secure_js_sandbox_ts_utils::strip_types_only(code)?
    } else {
        code
    };

    let (config, response_builder) = EvaluateResponseBuilder::new(config);
    let mut sandbox = engine.build(config).await?;
    let result = sandbox.evaluate(&code, &method, &args).await;
    Ok(response_builder.build_response(result, sandbox))
}
