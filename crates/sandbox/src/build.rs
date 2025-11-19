#![deny(warnings)]

use std::{error::Error, time::Instant};
use wasmtime::{Config, Engine};

fn get_engine() -> Result<Engine, Box<dyn Error>> {
    let mut engine_config = Config::new();
    // engine_config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
    engine_config.consume_fuel(true);
    engine_config.async_support(true);

    // An engine stores and configures global compilation settings like
    // optimization level, enabled wasm features, etc.
    let engine = Engine::new(&engine_config)?;
    Ok(engine)
}

fn main() -> Result<(), Box<dyn Error>> {
    let input_path = "src/sandbox.wasm";
    let start = Instant::now();

    eprintln!("Reading Module From File ({:?} elapsed)", start.elapsed());
    let wasm_bytes = std::fs::read(input_path)?;

    eprintln!("Compiling Module ({:?} elapsed)", start.elapsed());
    // An engine stores and configures global compilation settings like
    // optimization level, enabled wasm features, etc.
    let engine = get_engine()?;
    let compiled_component = engine.precompile_component(&wasm_bytes)?;

    eprintln!("Wring Module To File ({:?} elapsed)", start.elapsed());
    std::fs::write("src/sandbox.bin", &compiled_component)?;

    eprintln!("Finished ({:?} elapsed)", start.elapsed());

    println!("cargo:rerun-if-changed={}", input_path);
    Ok(())
}
