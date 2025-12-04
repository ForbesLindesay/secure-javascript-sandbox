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

// /// Returns a vector of (input_path, output_path) for all wasm files to compile. It
// /// finds the files by looking for any .wasm files in the sandboxes directory.
// fn get_wasm_files() -> Vec<(String, String)> {
//     let mut files = Vec::new();

//     if let Ok(entries) = std::fs::read_dir("src/sandboxes") {
//         for entry in entries.flatten() {
//             if let Ok(path) = entry.path().canonicalize() {
//                 if let Some(extension) = path.extension() {
//                     if extension == "wasm" {
//                         let input = entry.file_name().to_string_lossy().to_string();
//                         let output = input.replace(".wasm", ".bin");
//                         files.push((input, output));
//                     }
//                 }
//             }
//         }
//     }

//     files
// }
// fn main() -> Result<(), Box<dyn Error>> {
//     for (input, output) in get_wasm_files() {
//         compile(&format!("src/sandboxes/{}", input), &format!("src/sandboxes/{}", output))?;
//     }
//     Ok(())
// }

fn main() -> Result<(), Box<dyn Error>> {
    compile("src/sandbox.wasm", "src/sandbox.bin")?;
    compile("src/tsutils.wasm", "src/tsutils.bin")?;
    Ok(())
}
