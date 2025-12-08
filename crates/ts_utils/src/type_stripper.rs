use anyhow::Result;
use std::sync::Arc;
use swc_common::{SourceMap, errors::Handler};
use swc_ts_fast_strip::{Mode, Options, operate};

/// Given a string of TypeScript or JavaScript source code,
/// returns a string of JavaScript source code with all type
/// annotations removed.
/// The annotations are replaced with whitespace to preserve
/// alignment with the original source code without the need
/// for source maps.
pub fn strip_types(input: String) -> Result<String> {
    let cm = Arc::<SourceMap>::default();
    let handler = Handler::with_emitter_writer(Box::new(std::io::stderr()), Some(cm.clone()));
    let options = Options {
        module: Some(true),
        filename: None,
        mode: Mode::StripOnly,
        ..Default::default()
    };

    let result = operate(&cm, &handler, input, options)?;

    Ok(result.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_types_only() {
        let src = "const x: number = 10;";
        let res = strip_types(src.to_string()).unwrap();
        println!("Result: {}", res);
        assert!(res.contains("const x         = 10"));
    }
}
