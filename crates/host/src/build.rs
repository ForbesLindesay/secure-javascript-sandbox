#![deny(warnings)]

use std::{error::Error, time::Instant};
use wasmtime::{Config, Engine};

fn get_engine() -> Result<Engine, Box<dyn Error>> {
    let mut config = Config::new();
    config.consume_fuel(true);

    // An engine stores and configures global compilation settings like
    // optimization level, enabled wasm features, etc.
    let engine = Engine::new(&config)?;
    Ok(engine)
}

fn main() -> Result<(), Box<dyn Error>> {
  let input_path = "../../target/wasm32-wasi/release/secure_js_sandbox_interpreter_boa.wasm";
    let start = Instant::now();

    eprintln!("Reading Module From File ({:?} elapsed)", start.elapsed());
    let wasm_bytes =
        std::fs::read(input_path)?;

    eprintln!("Compiling Module ({:?} elapsed)", start.elapsed());
    // An engine stores and configures global compilation settings like
    // optimization level, enabled wasm features, etc.
    let engine = get_engine()?;
    let compiled_module = engine.precompile_module(&wasm_bytes)?;

    eprintln!("Wring Module To File ({:?} elapsed)", start.elapsed());
    std::fs::write(
        "src/secure_js_sandbox_interpreter_boa.bin",
        &compiled_module,
    )?;

    eprintln!("Finished ({:?} elapsed)", start.elapsed());

    println!("cargo:rerun-if-changed={}", input_path);
    Ok(())
}
