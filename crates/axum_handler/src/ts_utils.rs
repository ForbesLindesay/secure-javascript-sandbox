use std::sync::Arc;

use axum::{
    Json,
    routing::{MethodRouter, post},
};
use secure_js_sandbox::{
    ApiRequestBodyLimit, TsUtilsEngine, TsUtilsEvaluateError, TsUtilsSandboxConfig,
    TsUtilsSandboxInstance, ValidateModuleMode,
};
use serde::{Deserialize, Serialize};

use crate::{SandboxServerConfig, get_env, server_config::set_request_body_limit};

#[derive(Deserialize)]
pub struct StripTypesRequest {
    pub code: String,
    pub filename: Option<String>,
}

#[derive(Deserialize)]
pub struct ValidateModuleRequest {
    pub code: String,
    #[serde(default)]
    pub mode: ValidateModuleMode,
    pub filename: Option<String>,
}

#[derive(Serialize)]
pub struct StripTypesResponseSuccess {
    pub success: bool, // Always true
    pub code: String,
}

#[derive(Serialize)]
pub struct ValidateModuleResponseSuccess {
    pub success: bool, // Always true
    pub has_dynamic_import: bool,
    pub static_imports: Vec<StaticImport>,
    pub exports: Vec<ModuleExport>,
}

#[derive(Serialize)]
pub struct StaticImport {
    source: String,
    imported_names: Vec<String>,
    has_star_import: bool,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum ModuleExport {
    NAMED { name: String },
    STAR { source: String },
}

#[derive(Serialize)]
pub struct TsResponseFailure {
    pub success: bool, // Always false
    pub error: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum StripTypesResponse {
    Success(StripTypesResponseSuccess),
    Failure(TsResponseFailure),
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ValidateModuleResponse {
    Success(ValidateModuleResponseSuccess),
    Failure(TsResponseFailure),
}

#[derive(Clone)]
pub struct TsUtilsHandler {
    api_request_body_limit: ApiRequestBodyLimit,
    engine: Arc<TsUtilsEngine>,
    config: Arc<TsUtilsSandboxConfig>,
}
impl TsUtilsHandler {
    pub fn new(config: TsUtilsSandboxConfig) -> anyhow::Result<Self> {
        Self::new_with_limit(Default::default(), config)
    }
    pub fn new_with_limit(
        api_request_body_limit: ApiRequestBodyLimit,
        config: TsUtilsSandboxConfig,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            api_request_body_limit,
            engine: Arc::new(TsUtilsEngine::new()?),
            config: Arc::new(config),
        })
    }
    pub fn from_env() -> anyhow::Result<Self> {
        let SandboxServerConfig {
            api_request_body_limit,
            cpu_fuel,
            mut memory_limits,
            ..
        } = SandboxServerConfig::from_env()?;
        memory_limits.set_from_env("TS_UTILS")?;
        let config = TsUtilsSandboxConfig {
            cpu_fuel: get_env("TS_UTILS_CPU_FUEL")?.unwrap_or(cpu_fuel),
            memory_limits: memory_limits.to_memory_limits(),
        };
        Self::new_with_limit(
            get_env("TS_UTILS_API_REQUEST_BODY_LIMIT_BYTES")?.unwrap_or(api_request_body_limit),
            config,
        )
    }
    #[must_use]
    pub fn api_request_body_limit(&self) -> ApiRequestBodyLimit {
        self.api_request_body_limit
    }
    pub async fn build(&self) -> Result<TsUtilsSandboxInstance, TsUtilsEvaluateError> {
        self.engine
            .build(self.config.as_ref().clone())
            .await
            .map_err(Into::into)
    }
}

pub fn create_strip_types_handler<T: Clone + Send + Sync + 'static>(
    handler: TsUtilsHandler,
) -> MethodRouter<T> {
    let limit = handler.api_request_body_limit();
    let result: MethodRouter<T> = set_request_body_limit(
        post(
            async move |Json(request): Json<StripTypesRequest>| -> Json<StripTypesResponse> {
                Json(strip_types(&handler, request).await)
            },
        ),
        limit,
    );
    result
}

pub async fn strip_types(
    handler: &TsUtilsHandler,
    request: StripTypesRequest,
) -> StripTypesResponse {
    let result = match handler.build().await {
        Ok(mut sandbox) => {
            sandbox
                .strip_types(&request.code, request.filename.as_deref())
                .await
        }
        Err(err) => Err(err),
    };
    match result {
        Ok(code) => StripTypesResponse::Success(StripTypesResponseSuccess {
            success: true,
            code,
        }),
        Err(err) => StripTypesResponse::Failure(TsResponseFailure {
            success: false,
            error: err.to_string(),
        }),
    }
}

pub fn create_validate_module_handler<T: Clone + Send + Sync + 'static>(
    handler: TsUtilsHandler,
) -> MethodRouter<T> {
    let limit = handler.api_request_body_limit();
    let result: MethodRouter<T> = set_request_body_limit(post(
        async move |Json(request): Json<ValidateModuleRequest>| -> Json<ValidateModuleResponse> {
            Json(validate_module(&handler, request).await)
        },
    ), limit);
    result
}

pub async fn validate_module(
    handler: &TsUtilsHandler,
    request: ValidateModuleRequest,
) -> ValidateModuleResponse {
    let result = match handler.build().await {
        Ok(mut sandbox) => {
            sandbox
                .validate_module(&request.code, request.mode, request.filename.as_deref())
                .await
        }
        Err(err) => Err(err),
    };
    match result {
        Ok(result) => ValidateModuleResponse::Success(ValidateModuleResponseSuccess {
            success: true,
            has_dynamic_import: result.has_dynamic_import,
            static_imports: result
                .static_imports
                .into_iter()
                .map(|import| StaticImport {
                    source: import.source,
                    imported_names: import.names,
                    has_star_import: import.star,
                })
                .collect(),
            exports: result
                .exports
                .into_iter()
                .map(|export| match export {
                    secure_js_sandbox::ModuleExport::Named(name) => ModuleExport::NAMED { name },
                    secure_js_sandbox::ModuleExport::Star(source) => ModuleExport::STAR { source },
                })
                .collect(),
        }),
        Err(err) => ValidateModuleResponse::Failure(TsResponseFailure {
            success: false,
            error: err.to_string(),
        }),
    }
}
