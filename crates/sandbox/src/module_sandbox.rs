use secure_js_sandbox_ts_utils::compile_module;
use wasmtime::Store;

use crate::EvaluateError;
use crate::sandbox::SandboxInstanceBase;
use crate::state::SandboxState;

crate::sandbox::impl_sandbox_engine!(
    ModuleSandboxEngine,
    ModuleSandboxInstance,
    Modulesandbox,
    "modulesandbox",
    "../../wit/modulesandbox.wit",
    "modulesandbox.bin",
    true
);

impl ModuleSandboxInstance {
    pub async fn evaluate(
        &mut self,
        script: &str,
        method: &str,
        parameters: &[serde_json::Value],
    ) -> Result<serde_json::Value, EvaluateError> {
        let module = compile_module(script.to_owned())?;
        let parameters: Vec<_> = parameters
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<_, _>>()?;
        let result = self
            .sandbox
            .call_evaluate(
                &mut self.store,
                &module.code,
                module.has_dynamic_import,
                &module.static_imports,
                method,
                &parameters,
            )
            .await;
        if result.is_err() && self.get_fuel_remaining() == 0 {
            return Err(EvaluateError::FuelExhausted);
        }
        let result = result?;
        let result = result?;
        let result = serde_json::from_str(&result)?;
        Ok(result)
    }
}
