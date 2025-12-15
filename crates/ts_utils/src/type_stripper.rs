use anyhow::Result;
use std::sync::Arc;
use swc_common::SourceMap;
use swc_ts_fast_strip::{Mode, Options, operate};

use crate::str_handler::StringHandlerOutput;

/// Given a string of TypeScript or JavaScript source code,
/// returns a string of JavaScript source code with all type
/// annotations removed.
/// The annotations are replaced with whitespace to preserve
/// alignment with the original source code without the need
/// for source maps.
pub fn strip_types(input: String, filename: Option<String>) -> Result<String> {
    let cm = Arc::<SourceMap>::default();
    let (handler, handler_output) = StringHandlerOutput::new(Some(cm.clone()));
    let options = Options {
        module: Some(true),
        filename,
        mode: Mode::StripOnly,
        ..Default::default()
    };

    match operate(&cm, &handler, input, options) {
        Ok(result) => Ok(result.code),
        Err(e) => {
            let err_output = handler_output.into_string();
            if !err_output.is_empty() {
                Err(anyhow::anyhow!(err_output))
            } else {
                Err(anyhow::anyhow!(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_types_only() {
        let src = "const x: number = 10;";
        let res = strip_types(src.to_string(), None).unwrap();
        println!("Result: {}", res);
        assert!(res.contains("const x         = 10"));
    }
}
