use wasmtime::Store;

use crate::EvaluateError;
use crate::sandbox::SandboxInstanceBase;
use crate::state::SandboxState;

crate::sandbox::impl_sandbox_engine!(
    FunctionSandboxEngine,
    FunctionSandboxInstance,
    Sandbox,
    "sandbox",
    "../../wit/sandbox.wit",
    "sandbox.bin",
    false
);

impl FunctionSandboxInstance {
    pub async fn evaluate(
        &mut self,
        script: &str,
        parameters: &[serde_json::Value],
    ) -> Result<serde_json::Value, EvaluateError> {
        let parameters: Vec<_> = parameters
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<_, _>>()?;
        let result = self
            .sandbox
            .call_evaluate(&mut self.store, script, &parameters)
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
