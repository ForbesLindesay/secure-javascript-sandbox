#![deny(warnings)]

use serde::{Deserialize, Serialize};
pub use serde_json::Error as JsonError;
pub use serde_json::Value as JsonValue;

macro_rules! json_methods {
    ($name:ident) => {
        impl $name {
            pub fn from_str(str: &str) -> Result<$name, JsonError> {
                serde_json::from_str(str)
            }
            pub fn to_string(&self) -> Result<String, JsonError> {
                serde_json::to_string(self)
            }
        }
    };
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Input {
    pub script: String,
    // TODO: Other call types?
}

json_methods!(Input);

#[derive(Serialize, Deserialize, Debug)]
pub enum Output {
    Ok { value: JsonValue },
    Err { message: String },
}

json_methods!(Output);
