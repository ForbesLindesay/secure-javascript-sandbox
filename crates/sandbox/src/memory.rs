use serde::Deserialize;

use crate::{MemoryLimitBytes, MemorySizeBytes, ResourceLimit, TableLimit};

#[derive(Clone, Debug, Deserialize)]
pub struct MemoryLimits {
    #[serde(default)]
    pub memory_size_bytes: MemoryLimitBytes,
    #[serde(default)]
    pub table_elements: TableLimit,
    #[serde(default)]
    pub instances: ResourceLimit,
    #[serde(default)]
    pub tables: ResourceLimit,
    #[serde(default)]
    pub memories: ResourceLimit,
    #[serde(default)]
    pub trap_on_grow_failure: bool,
    pub stdout_bytes: MemorySizeBytes,
    pub stderr_bytes: MemorySizeBytes,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            memory_size_bytes: Default::default(),
            table_elements: Default::default(),
            instances: Default::default(),
            tables: Default::default(),
            memories: Default::default(),
            trap_on_grow_failure: Default::default(),
            stdout_bytes: Default::default(),
            stderr_bytes: Default::default(),
        }
    }
}
