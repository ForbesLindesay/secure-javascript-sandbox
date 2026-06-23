use std::str::FromStr;

pub fn get_env<T: FromStr>(var: &str) -> anyhow::Result<Option<T>> {
    match std::env::var(var) {
        Ok(s) => match s.parse() {
            Ok(value) => Ok(Some(value)),
            Err(_) => Err(anyhow::anyhow!("Failed to parse env var {var}: {s}")),
        },
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to read env var {var}: {e}")),
    }
}
