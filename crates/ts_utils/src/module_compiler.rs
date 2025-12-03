use crate::module_visitor::{Export, IdentifierVisitor, ModuleVisitor, Replacement};
use anyhow::{Context, Result};
use serde::Serialize;
use std::sync::Arc;
use swc_common::{FileName, SourceMap, comments::SingleThreadedComments, errors::Handler};
use swc_ecma_ast::*;
use swc_ecma_parser::{Parser, StringInput, Syntax, lexer::Lexer};
use swc_ecma_visit::VisitWith;

#[derive(Serialize)]
pub struct CompiledModule {
    pub has_dynamic_import: bool,
    pub static_imports: Vec<String>,
    pub code: String,
}

/// Given a string of JavaScript representing a module, convert it into
/// an async JavaScript function.
///
/// As much as possible, line numbers and columns will be preserved to
/// allow stack traces to match the original source code without the
/// need for source maps.
///
/// Static imports and a dynamic import function may need to be passed as
/// arguments to the resulting function. The returned object represents
/// the exports of the module.
pub fn compile_module(input: String) -> Result<CompiledModule> {
    let cm = Arc::<SourceMap>::default();
    let handler = Handler::with_emitter_writer(Box::new(std::io::stderr()), Some(cm.clone()));

    let fm = cm.new_source_file(Arc::new(FileName::Anon), input);
    let comments = SingleThreadedComments::default();

    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        EsVersion::latest(),
        StringInput::from(&*fm),
        Some(&comments),
    );

    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|e| {
            e.into_diagnostic(&handler).emit();
            anyhow::anyhow!("failed to parse module")
        })
        .context("failed to parse module")?;

    let mut identifiers = IdentifierVisitor::new();
    module.visit_with(&mut identifiers);

    let mut visitor = ModuleVisitor::new(identifiers);
    module.visit_with(&mut visitor);

    let source = fm.src.clone();
    let mut code = fm.src.to_string().into_bytes();
    for (span, replacement) in visitor.replacements {
        let mut replacement = match replacement {
            Replacement::Whitespace => {
                // Replace with whitespace to preserve line and column numbers
                Vec::new()
            }
            Replacement::ExportDefaultExpression(ident) => {
                format!("var {}=", ident).bytes().collect()
            }
            Replacement::ImportFnReference(ident) => ident.bytes().collect(),
        };
        let mut replacement_idx = 0;
        // TODO:
        // pub enum Replacement {
        //     Whitespace,
        //     ExportDefaultExpression,
        //     ImportFnReference,
        // }
        let (start, end) = (span.lo.0 as usize - 1, span.hi.0 as usize - 1);

        for (i, c) in source[start..end].char_indices() {
            let i = start + i;
            let replacement_char = replacement.get(replacement_idx);
            replacement_idx += 1;
            if let Some(r) = replacement_char {
                if c.len_utf8() != 1 {
                    return Err(anyhow::anyhow!("replacement character length mismatch"));
                }
                code[i] = *r;
            }
            match c {
                // https://262.ecma-international.org/#sec-white-space
                '\u{0009}' | '\u{0000B}' | '\u{000C}' | '\u{FEFF}' => {}
                // Space_Separator
                '\u{0020}' | '\u{00A0}' | '\u{1680}' | '\u{2000}' | '\u{2001}' | '\u{2002}'
                | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}' | '\u{2007}' | '\u{2008}'
                | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => {}
                // https://262.ecma-international.org/#sec-line-terminators
                '\u{000A}' | '\u{000D}' | '\u{2028}' | '\u{2029}' => {
                    if replacement_char.is_some() {
                        replacement.push(b'\n');
                    }
                }
                _ => {
                    if !replacement_char.is_some() {
                        match c.len_utf8() {
                            1 => {
                                // Space 0020
                                code[i] = 0x20;
                            }
                            2 => {
                                // No-Break Space 00A0
                                code[i] = 0xc2;
                                code[i + 1] = 0xa0;
                            }
                            3 => {
                                // En Space 2002
                                code[i] = 0xe2;
                                code[i + 1] = 0x80;
                                code[i + 2] = 0x82;
                            }
                            4 => {
                                // We do not have a 4-byte space character in the Unicode standard.

                                // Space 0020
                                code[i] = 0x20;
                                // ZWNBSP FEFF
                                code[i + 1] = 0xef;
                                code[i + 2] = 0xbb;
                                code[i + 3] = 0xbf;
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }
    // SAFETY: We've already validated that the source is valid utf-8
    // and our operations are limited to character-level string replacements.
    let result = unsafe { String::from_utf8_unchecked(code) };

    let has_dynamic_import = visitor.import_fn_identifier.is_some();
    let mut static_imports: Vec<String> = Vec::new();
    let mut full_result = String::new();
    full_result.push_str("async (");
    let mut is_first_import = true;
    if let Some(ident) = visitor.import_fn_identifier {
        is_first_import = false;
        full_result.push_str(ident.as_str());
    }
    for import in visitor.imports {
        if is_first_import {
            is_first_import = false;
        } else {
            full_result.push(',');
        }
        full_result.push_str(&format!("{}", import.pattern));
        let Some(src) = import.source.value.as_str() else {
            return Err(anyhow::anyhow!("import source is not a string"));
        };
        static_imports.push(src.to_string());
    }
    full_result.push_str(")=>{");
    full_result.push_str(&result);
    full_result.push_str(";return {");
    let mut is_first_export = true;
    for export in visitor.exports {
        if is_first_export {
            is_first_export = false;
        } else {
            full_result.push(',');
        }
        match export {
            Export::ExportNamed { exported, local } => {
                full_result.push_str(&format!("{}:{}", exported, local.as_str()));
            }
            Export::ExportAll { local } => {
                full_result.push_str(&format!("...{}", local.as_str()));
            }
        };
    }
    full_result.push('}');
    full_result.push('}');
    Ok(CompiledModule {
        has_dynamic_import,
        static_imports,
        code: full_result,
    })
}

#[cfg(test)]
mod tests {
    use secure_js_sandbox::SandboxConfig;

    use super::*;

    #[tokio::test]
    async fn test_handle_module() {
        let src = r#"
            import theAnswer from "the-answer";
            
            export const asyncTheAnswer = (await import("the-answer")).default;

            export function x() {
                return theAnswer;
            }
            export const y = 42;
            export default () => {
                return 4;
            }
        "#;
        let res = compile_module(src.to_string()).unwrap();
        println!("Result: {}", res.code);
        let mut arguments: Vec<String> = Vec::new();
        if res.has_dynamic_import {
            arguments.push(
                r#"
                async function (source) {
                    if (source === "the-answer") {
                        return { default: 42 };
                    } else {
                        throw new Error("Unknown module: " + source);
                    }
                }
            "#
                .to_string(),
            );
        }
        for import in res.static_imports {
            assert_eq!(import, "the-answer");
            arguments.push(r#"{ default: 42 }"#.to_string());
        }
        let code = res.code;
        let arguments = arguments.join(",");
        let engine = secure_js_sandbox::SandboxEngine::new().unwrap();
        engine
            .evaluate(
                &format!(
                    r#"
                        async function () {{
                            const result = await ({code})({arguments});
                            const _assert = (condition, message) => {{
                                if (!condition) {{
                                    throw new Error("Assertion failed: " + message);
                                }}
                            }};
                            console.log(result);
                            _assert(result.x() === 42, "x() should return 42");
                            _assert(result.y === 42, "y should be 42");
                            _assert(result.default() === 4, "default() should return 4");
                            _assert(result.asyncTheAnswer === 42, "default() should return 4");
                        }}
                    "#
                ),
                &vec![],
                SandboxConfig {
                    cpu_fuel: 500_000_000_000,
                    memory_limits: Default::default(),
                    http: secure_js_sandbox::HttpMode::BlockAll,
                    ..Default::default()
                }
            )
            .await
            .result
            .unwrap();
    }
}
