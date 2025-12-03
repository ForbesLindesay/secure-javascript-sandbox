#![deny(warnings)]

mod env;
mod evaluate;
mod evaluate_request;
mod evaluate_response;
mod server_config;
mod strip_types;

pub use crate::env::get_env;
pub use crate::evaluate::{create_evaluate_handler, evaluate};
pub use crate::evaluate_request::{EvaluateRequest, EvaluateRequestWithConfig};
pub use crate::evaluate_response::{EvaluateResponse, OutboundRequest};
pub use crate::server_config::{
    AllowRequestToConfigureSandbox, SandboxServerConfig, SandboxServerConfigTrait,
    SandboxServerMemoryLimits,
};
pub use crate::strip_types::{
    StripTypesRequest, StripTypesResponse, StripTypesResponseFailure, StripTypesResponseSuccess,
    create_strip_types_handler,
};
pub use secure_js_sandbox::{CustomHttpMode, HttpMode, MemoryLimits, MemoryOutputPipe};
