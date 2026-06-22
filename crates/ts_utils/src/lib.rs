mod module_compiler;
mod module_visitor;
mod str_handler;
mod type_stripper;

#[cfg(target_env = "p2")]
mod bindings {
    use crate::wasm_implementation::TsUtils;
    wit_bindgen::generate!({
        path: "wit/ts-utils.wit",
    });
    export!(TsUtils);
}
#[cfg(not(target_env = "p2"))]
mod bindings {
    wit_bindgen::generate!({
        path: "wit/ts-utils.wit",
    });
}

#[cfg(target_env = "p2")]
pub(crate) mod wasm_implementation {
    use crate::bindings::exports::local::ts_utils::ts_utils_impl::Guest;
    use crate::implementation;
    pub(crate) struct TsUtils;
    impl Guest for TsUtils {
        fn strip_types(script: String, filename: Option<String>) -> Result<String, String> {
            implementation::strip_types(script, filename).map_err(|e| e.to_string())
        }

        fn compile_module(
            script: String,
            filename: Option<String>,
        ) -> Result<implementation::CompiledModule, String> {
            implementation::compile_module(script, filename).map_err(|e| e.to_string())
        }

        fn strip_types_and_compile_module(
            script: String,
            filename: Option<String>,
        ) -> Result<implementation::CompiledModule, String> {
            implementation::strip_types_and_compile_module(script, filename)
                .map_err(|e| e.to_string())
        }
    }
}

mod implementation {
    pub use crate::bindings::exports::local::ts_utils::ts_utils_impl::{
        CompiledModule, ModuleExport, StaticImport,
    };
    pub use crate::module_compiler::compile_module;
    pub use crate::type_stripper::strip_types;

    pub fn strip_types_and_compile_module(
        script: String,
        filename: Option<String>,
    ) -> anyhow::Result<CompiledModule> {
        // TODO: share AST for better efficiency?
        let stripped = strip_types(script, filename.clone())?;
        compile_module(stripped, filename)
    }
}

#[cfg(not(target_env = "p2"))]
pub use implementation::*;
