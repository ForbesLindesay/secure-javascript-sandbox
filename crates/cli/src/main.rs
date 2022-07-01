#![deny(warnings)]

use std::error::Error;
use secure_js_sandbox_host as host;

fn main() -> Result<(), Box<dyn Error>> {
    host::main()
}