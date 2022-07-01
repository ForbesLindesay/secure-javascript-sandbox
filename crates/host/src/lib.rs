#![deny(warnings)]

use lazy_static::lazy_static;
use std::error::Error;
use wasi_common::WasiCtx;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::sync::WasiCtxBuilder;

struct StoreState {
    wasi: WasiCtx,
    limits: StoreLimits,
}

lazy_static! {
    static ref WASM_ENGINE: Engine = {
        let mut config = Config::new();
        config.consume_fuel(true);

        // An engine stores and configures global compilation settings like
        // optimization level, enabled wasm features, etc.
        Engine::new(&config).unwrap()
    };

    static ref WASM_MODULE: Module =
        unsafe { Module::deserialize(&WASM_ENGINE, include_bytes!("secure_js_sandbox_interpreter_boa.bin")).unwrap() };

    static ref WASM_LINKER: Linker<StoreState> = {
        let mut linker: Linker<StoreState> = Linker::new(&WASM_ENGINE);

        // Wasi Provides support for accessing system APIs from the sandbox.
        // System APIs are only exposed based on the capabilities in the WasiCtx
        // on the store. We are enabling the APIs needed for Date.now() and Math.random()
        // to work from within JavaScript.
        wasmtime_wasi::snapshots::preview_1::add_wasi_snapshot_preview1_to_linker(&mut linker, |s| &mut s.wasi).unwrap();

        linker
    };
}

#[derive(Debug)]
pub struct MemoryLimits {
    pub max_bytes: usize,
    pub max_table_elements: u32,
}
pub fn main(limits: &MemoryLimits, fuel: u64) -> Result<(), Box<dyn Error>> {
    // A `Store` is what will own instances, functions, globals, etc. All wasm
    // items are stored within a `Store`, and it's what we'll always be using to
    // interact with the wasm world. Custom data can be stored in stores but for
    // now we just use `()`.
    let mut store = Store::new(
        &WASM_ENGINE,
        StoreState {
            // TODO: we don't want to inherit stdio
            wasi: WasiCtxBuilder::new()
                .inherit_stdin()
                .inherit_stdout()
                .inherit_stderr()
                .build(),
            limits: StoreLimitsBuilder::new()
                .memory_size(limits.max_bytes)
                .table_elements(limits.max_table_elements)
                .build(),
        },
    );
    store.limiter(|s| &mut s.limits);

    // an `Instance` which we can actually poke at functions on.
    // With a compiled `Module` we can then instantiate it, creating
    // Using the linker to instantiate it lets us make functions available by name
    let instance = WASM_LINKER.instantiate(&mut store, &WASM_MODULE)?;
    let start = instance
        .get_func(&mut store, "_start")
        .expect("Missing _start fn in WASM module");

    store.add_fuel(fuel)?;
    start.call(&mut store, &mut [], &mut [])?;

    Ok(())
}
