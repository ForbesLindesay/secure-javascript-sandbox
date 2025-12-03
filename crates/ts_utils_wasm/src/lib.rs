#[cfg(target_env="p2")]
use secure_js_sandbox_ts_utils::strip_types_only;

#[cfg(target_env="p2")]
mod bindings {
    use crate::TsUtils;
    wit_bindgen::generate!({
        path: "wit/ts-utils.wit",
    });
    export!(TsUtils);
}

#[cfg(target_env="p2")]
use bindings::exports::local::ts_utils::ts_utils_impl as bindings_impl;

#[cfg(target_env="p2")]
struct TsUtils;
#[cfg(target_env="p2")]
impl bindings_impl::Guest for TsUtils {
    fn strip_types_only(script: String) -> Result<String, String> {
        strip_types_only(script).map_err(|e| e.to_string())
    }

    fn compile_module_only(script: String) -> Result<bindings_impl::CompiledModule, String> {
        match secure_js_sandbox_ts_utils::compile_module(script) {
            Ok(compiled) => Ok(bindings_impl::CompiledModule {
                has_dynamic_import: compiled.has_dynamic_import,
                static_imports: compiled.static_imports,
                code: compiled.code,
            }),
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
