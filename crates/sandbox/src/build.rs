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

fn compile(input_path: &str, output_path: &str) -> Result<(), Box<dyn Error>> {
    let start = Instant::now();

    eprintln!("Reading Module From File ({:?} elapsed)", start.elapsed());
    let wasm_bytes = std::fs::read(input_path)?;

    eprintln!("Compiling Module ({:?} elapsed)", start.elapsed());
    // An engine stores and configures global compilation settings like
    // optimization level, enabled wasm features, etc.
    let engine = get_engine()?;
    let compiled_component = engine.precompile_component(&wasm_bytes)?;

    eprintln!("Writing Module To File ({:?} elapsed)", start.elapsed());
    std::fs::write(output_path, &compiled_component)?;

    eprintln!("Finished ({:?} elapsed)", start.elapsed());

    println!("cargo:rerun-if-changed={}", input_path);

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    compile("src/sandbox.wasm", "src/sandbox.bin")?;
    compile("src/modulesandbox.wasm", "src/modulesandbox.bin")?;
    Ok(())
}
