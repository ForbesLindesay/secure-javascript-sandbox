#![deny(warnings)]

use secure_js_sandbox_host as host;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    host::main(
        &host::MemoryLimits {
            max_bytes: 50 * 1024 * 1024,
            max_table_elements: 10_000,
        },
        440_000_000,
    )
}
