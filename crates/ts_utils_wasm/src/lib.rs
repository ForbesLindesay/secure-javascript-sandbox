#[cfg(target_env="p2")]
mod bindings {
    use crate::implementation::TsUtils;
    wit_bindgen::generate!({
        path: "wit/ts-utils.wit",
    });
    export!(TsUtils);
}

#[cfg(target_env="p2")]
mod implementation {
    use secure_js_sandbox_ts_utils::{strip_types_only, compile_module, Export};
    use crate::bindings::exports::local::ts_utils::ts_utils_impl as bindings_impl;

    pub(crate) struct TsUtils;
    impl bindings_impl::Guest for TsUtils {
        fn strip_types_only(script: String) -> Result<String, String> {
            strip_types_only(script).map_err(|e| e.to_string())
        }

        fn compile_module_only(script: String) -> Result<bindings_impl::CompiledModule, String> {
            match compile_module(script) {
                Ok(compiled) => {
                    let mut named_exports: Vec<String> = Vec::new();
                    let mut star_exports: Vec<String> = Vec::new();
                    for export in compiled.exports {
                        match export {
                            Export::ExportNamed { exported, .. } => {
                                named_exports.push(exported.to_string());
                            }
                            Export::ExportAll { source, .. } => {
                                let Some(src) = source.value.as_str() else {
                                    return Err("import source is not a string".to_string());
                                };
                                star_exports.push(src.to_string());
                            }
                        }
                    }
                    Ok(bindings_impl::CompiledModule {
                        has_dynamic_import: compiled.has_dynamic_import,
                        static_imports: compiled.static_imports,
                        code: compiled.code,
                        named_exports,
                        star_exports,
                    })
                },
                Err(e) => Err(e.to_string()),
            }
        }

        fn strip_types_and_compile_module(
            script: String,
        ) -> Result<bindings_impl::CompiledModule, String> {
            let stripped = TsUtils::strip_types_only(script)?;
            TsUtils::compile_module_only(stripped)
        }
    }
}