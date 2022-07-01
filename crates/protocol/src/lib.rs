#![deny(warnings)]

use serde::{Deserialize, Serialize};
pub use serde_json::Error as JsonError;
pub use serde_json::Value as JsonValue;

#[derive(Serialize, Deserialize, Debug)]
pub enum EvaluationResult {
    Ok(Option<JsonValue>),
    Err(String),
}

impl EvaluationResult {
    pub fn from_str(str: &str) -> Result<EvaluationResult, JsonError> {
        serde_json::from_str(str)
    }
    pub fn to_string(&self) -> Result<String, JsonError> {
        serde_json::to_string(self)
    }
}
