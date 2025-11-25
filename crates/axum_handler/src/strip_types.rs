use axum::{
    Json,
    routing::{MethodRouter, post},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct StripTypesRequest {
    pub script: String,
}

#[derive(Serialize)]
pub struct StripTypesResponseSuccess {
    pub success: bool, // Always true
    pub script: String,
}

#[derive(Serialize)]
pub struct StripTypesResponseFailure {
    pub success: bool, // Always false
    pub error: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum StripTypesResponse {
    Success(StripTypesResponseSuccess),
    Failure(StripTypesResponseFailure),
}

pub fn create_strip_types_handler<T: Clone + Send + Sync + 'static>() -> MethodRouter<T> {
    let result: MethodRouter<T> = post(
        async move |Json(request): Json<StripTypesRequest>| -> Json<StripTypesResponse> {
            let result = secure_js_sandbox_ts_utils::strip_types_only(request.script);
            Json(match result {
                Ok(stripped_script) => StripTypesResponse::Success(StripTypesResponseSuccess {
                    success: true,
                    script: stripped_script,
                }),
                Err(err) => StripTypesResponse::Failure(StripTypesResponseFailure {
                    success: false,
                    error: err.to_string(),
                }),
            })
        },
    );
    result
}
