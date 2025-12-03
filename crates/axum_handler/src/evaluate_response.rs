use std::net::SocketAddr;

use secure_js_sandbox::{RequestValidationOutcome, Uri};
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
    pub outbound_requests: Vec<OutboundRequest>,
    pub result: serde_json::Value,
}

#[derive(Serialize)]
pub struct OutboundRequest {
    pub uri: String,
    pub socket_addr: Option<String>,
    pub outcome: RequestValidationOutcome,
}
impl From<(Uri, Option<SocketAddr>, RequestValidationOutcome)> for OutboundRequest {
    fn from(
        (uri, socket_addr, outcome): (Uri, Option<SocketAddr>, RequestValidationOutcome),
    ) -> Self {
        OutboundRequest {
            uri: uri.to_string(),
            socket_addr: socket_addr.map(|addr| addr.to_string()),
            outcome,
        }
    }
}
