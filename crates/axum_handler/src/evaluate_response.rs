use secure_js_sandbox::{OutboundRequest, RequestValidationOutcome};
use serde::Serialize;

#[derive(Serialize)]
pub struct EvaluateResponse {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub fuel_consumed: u64,
    pub fuel_remaining: u64,
    pub max_requested_memory_bytes: usize,
    pub max_requested_table_elements: usize,
    pub outbound_requests: Vec<SerializableOutboundRequest>,
    pub result: serde_json::Value,
}

#[derive(Serialize)]
pub struct SerializableOutboundRequest {
    pub uri: String,
    pub socket_addr: Option<String>,
    pub outcome: RequestValidationOutcome,
}
impl From<OutboundRequest> for SerializableOutboundRequest {
    fn from(OutboundRequest(uri, socket_addr, outcome): OutboundRequest) -> Self {
        SerializableOutboundRequest {
            uri: uri.to_string(),
            socket_addr: socket_addr.map(|addr| addr.to_string()),
            outcome,
        }
    }
}
