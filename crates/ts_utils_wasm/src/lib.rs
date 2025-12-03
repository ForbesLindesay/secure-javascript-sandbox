use secure_js_sandbox_ts_utils::strip_types_only;

mod bindings {
    //! This module contains generated code for implementing
    //! the `adder` world in `wit/world.wit`.
    //!
    //! The `path` option is actually not required,
    //! as by default `wit_bindgen::generate` will look
    //! for a top-level `wit` directory and use the files
    //! (and interfaces/worlds) there-in.

    use crate::TsUtils;
    wit_bindgen::generate!({
        path: "../../wit/tsutils.wit",
    });
    export!(TsUtils);
}

struct TsUtils;
impl bindings::Guest for TsUtils {
    fn striptypesonly(script: String) -> Result<String, String> {
        strip_types_only(script).map_err(|e| e.to_string())
    }
}