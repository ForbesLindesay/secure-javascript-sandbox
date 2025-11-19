use serde::Deserialize;
use wasmtime::{DEFAULT_INSTANCE_LIMIT, DEFAULT_MEMORY_LIMIT, DEFAULT_TABLE_LIMIT};

fn default_instances() -> usize {
    DEFAULT_INSTANCE_LIMIT
}

fn default_table_elements() -> usize {
    DEFAULT_TABLE_LIMIT
}

fn default_memory_limit() -> usize {
    DEFAULT_MEMORY_LIMIT
}

fn default_trap_on_grow_failure() -> bool {
    false
}

fn default_stdio_max_bytes() -> usize {
    10 * 1024 * 1024 // 10 MB
}

#[derive(Clone, Debug, Deserialize)]
pub struct MemoryLimits {
    pub memory_size_bytes: Option<usize>,
    pub table_elements: Option<usize>,
    #[serde(default = "default_instances")]
    pub instances: usize,
    #[serde(default = "default_table_elements")]
    pub tables: usize,
    #[serde(default = "default_memory_limit")]
    pub memories: usize,
    #[serde(default = "default_trap_on_grow_failure")]
    pub trap_on_grow_failure: bool,

    #[serde(default = "default_stdio_max_bytes")]
    pub stdout_max_bytes: usize,
    #[serde(default = "default_stdio_max_bytes")]
    pub stderr_max_bytes: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            memory_size_bytes: Some(128 * 1024 * 1024), // 128 MB
            table_elements: Some(100_000),
            instances: DEFAULT_INSTANCE_LIMIT,
            tables: DEFAULT_TABLE_LIMIT,
            memories: DEFAULT_MEMORY_LIMIT,
            trap_on_grow_failure: false,
            stdout_max_bytes: default_stdio_max_bytes(),
            stderr_max_bytes: default_stdio_max_bytes(),
        }
    }
}
