use axum::{
    Json,
    routing::{MethodRouter, post},
};
use secure_js_sandbox::SandboxEngine;
use serde::de::DeserializeOwned;
use std::sync::Arc;

use crate::{
    CustomSandboxServerConfig, EvaluateResponse,
    server_config::{EvaluateInput, set_request_body_limit},
};

pub async fn create_evaluate_handler<
    TRequest: DeserializeOwned + Send + 'static,
    TConfig: CustomSandboxServerConfig<TRequest>,
    T: Clone + Send + Sync + 'static,
>(
    config: TConfig,
) -> anyhow::Result<MethodRouter<T>> {
    let limit = config.get_api_request_body_limit();
    let config = Arc::new(config);
    let engine = Arc::new(SandboxEngine::new()?);
    let result: MethodRouter<T> = set_request_body_limit(
        post(
            async move |Json(request): Json<TRequest>| -> Json<EvaluateResponse> {
                Json(evaluate(&config, request, &engine).await)
            },
        ),
        limit,
    );
    Ok(result)
}

pub async fn evaluate<
    TRequest: DeserializeOwned + Send + 'static,
    TConfig: CustomSandboxServerConfig<TRequest>,
>(
    config: &TConfig,
    request: TRequest,
    engine: &SandboxEngine,
) -> EvaluateResponse {
    let EvaluateInput {
        code,
        parameters,
        config,
    } = config.get_evaluate_input(request);
    let initial_cpu_fuel: u64 = config.cpu_fuel.into();
    let result = engine.evaluate(&code, &parameters, config).await;
    EvaluateResponse {
        success: result.result.is_ok(),
        stdout: result.stdout,
        stderr: result.stderr,
        fuel_consumed: initial_cpu_fuel.saturating_sub(result.fuel_remaining),
        fuel_remaining: result.fuel_remaining,
        max_requested_memory_bytes: result.max_requested_memory_bytes.unwrap_or(0),
        max_requested_table_elements: result.max_requested_table_elements.unwrap_or(0),
        outbound_requests: result
            .outbound_requests
            .into_iter()
            .map(Into::into)
            .collect(),
        result: match result.result {
            Ok(value) => value,
            Err(err) => serde_json::json!({"error": err.to_string()}),
        },
    }
}
