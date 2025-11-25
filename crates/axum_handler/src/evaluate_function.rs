use axum::{
    Json,
    routing::{MethodRouter, post},
};
use secure_js_sandbox::FunctionSandboxEngine;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    EvaluateResponse, SandboxServerConfig, SandboxServerConfigTrait,
    evaluate_response::EvaluateResponseBuilder,
};

pub struct EvaluateFunctionInput {
    pub script: String,
    pub args: Vec<serde_json::Value>,
    pub config: SandboxServerConfig,
}

#[derive(Deserialize)]
pub struct EvaluateFunctionRequest {
    pub script: String,
    pub args: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct EvaluateFunctionRequestWithConfig {
    pub script: String,
    pub args: Vec<serde_json::Value>,
    #[serde(default)]
    pub config: SandboxServerConfig,
}

pub async fn create_evaluate_function_handler<
    TConfig: SandboxServerConfigTrait<EvaluateFunctionInput>,
    T: Clone + Send + Sync + 'static,
>(
    config: TConfig,
) -> anyhow::Result<MethodRouter<T>> {
    let config = Arc::new(config);
    let engine = Arc::new(FunctionSandboxEngine::new()?);
    let result: MethodRouter<T> = post(
        async move |Json(request): Json<TConfig::RequestType>| -> Json<serde_json::Value> {
            match evaluate_function(&config, request, &engine).await {
                Ok(response) => Json(serde_json::to_value(response).unwrap()),
                Err(err) => Json(serde_json::json!({"error": err.to_string()})),
            }
        },
    );
    Ok(result)
}

pub async fn evaluate_function<TConfig: SandboxServerConfigTrait<EvaluateFunctionInput>>(
    config: &TConfig,
    request: TConfig::RequestType,
    engine: &FunctionSandboxEngine,
) -> anyhow::Result<EvaluateResponse> {
    let EvaluateFunctionInput {
        script,
        args,
        config,
    } = config.get_evaluate_input(request);
    let script = if config.enable_typescript_support {
        secure_js_sandbox_ts_utils::strip_types_only(script)?
    } else {
        script
    };

    let (config, response_builder) = EvaluateResponseBuilder::new(config);
    let mut sandbox = engine.build(config).await?;
    let result = sandbox.evaluate(&script, &args).await;
    Ok(response_builder.build_response(result, sandbox))
}
