#![deny(warnings)]

use lazy_static::lazy_static;
use std::sync::RwLock;
///, any::Any, io::{self}};
use std::{collections::vec_deque::VecDeque, sync::Arc};
use std::error::Error;
use std::io;
use wasi_common::WasiCtx; // , WasiFile, file::FileType
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::sync::WasiCtxBuilder;
// use wasi_common::ErrorExt;
use wasi_common::pipe::{ReadPipe, WritePipe};

use secure_js_sandbox_protocol::Input;
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
    let input = Input {
        script: "Hello World".to_string(),
    };
    // let stdin = WasmFileInputBuffer(format!("{}\n", input.to_string()?).as_bytes().to_vec());
    // let stdin = ReadPipe::from(format!("{}\n", input.to_string()?));
    let mut stdin = InputReader::new();
    stdin.write(format!("{}\n", input.to_string()?).as_bytes());
    // let stdout = WritePipe::new_in_memory();
    let mut stdout =OutputWriter::new();
    // A `Store` is what will own instances, functions, globals, etc. All wasm
    // items are stored within a `Store`, and it's what we'll always be using to
    // interact with the wasm world. Custom data can be stored in stores but for
    // now we just use `()`.
    let mut store = Store::new(
        &WASM_ENGINE,
        StoreState {
            // TODO: we don't want to inherit stdio
            wasi: WasiCtxBuilder::new()
                .stdin(Box::new(ReadPipe::new(stdin.clone())))
                .stdout(Box::new( WritePipe::new(stdout.clone())))
                // .inherit_stdin()
                // .inherit_stdout()
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

    // drop(store);
    // let contents = String::from_utf8(stdout.try_into_inner().expect("sole remaining reference to WritePipe").into_inner())?;
    // let contents = String::from_utf8(
    //     stdout
    //         // .try_into_inner()
    //         // .expect("sole remaining reference to WritePipe")
    //         .read_all(),
    // );
    // println!("contents of stdout: {:?}", contents);
    // stdin.write(format!("{}\n", input.to_string()?).as_bytes());
    // start.call(&mut store, &mut [], &mut [])?;
    // start.call(&mut store, &mut [], &mut [])?;
    let contents = String::from_utf8(
        stdout
            // .try_into_inner()
            // .expect("sole remaining reference to WritePipe")
            .read_all(),
    );
    println!("contents of stdout: {:?}", contents);
    Ok(())
}

// TODO: compare io::Cursor::new(vec![]) vs. VecDeque<u8>
#[derive(Debug)]
struct OutputWriter(Arc<RwLock<VecDeque<u8>>>);

impl Clone for OutputWriter{
    fn clone(&self) -> Self {
        Self (self.0.clone())
    }
}

impl OutputWriter {
    fn new() -> OutputWriter {
        OutputWriter(Arc::new(RwLock::new(VecDeque::new())))
    }
    fn read_all(&mut self) -> Vec<u8> {
        RwLock::write(&self.0).expect("Failed to get lock").drain(..).into_iter().collect()
    }
}
impl io::Write for OutputWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match RwLock::write(&self.0){
            Ok(mut inner) => {
                inner.extend(buf);
                Ok(buf.len())
            }
            Err(_) => {
                Ok(0)
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}


// TODO: compare io::Cursor::new(vec![]) vs. VecDeque<u8>
#[derive(Debug)]
struct InputReader(Arc<RwLock<VecDeque<u8>>>);

impl Clone for InputReader{
    fn clone(&self) -> Self {
        Self (self.0.clone())
    }
}

impl InputReader {
    fn new() -> InputReader {
        InputReader(Arc::new(RwLock::new(VecDeque::new())))
    }
    fn write(&mut self, bytes: &[u8]) {
        RwLock::write(&self.0).expect("Failed to get lock").extend(bytes);
    }
}
impl io::Read for InputReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match RwLock::write(&self.0){
            Ok(mut inner) => {
                let byte_len = buf.len().min(inner.len());
                // if byte_len == 0 && buf.len() != 0 {
                //     return Err(io::Error::new(io::ErrorKind::WouldBlock, "No more data"));
                // }
                let bytes: Vec<u8> = inner.drain(..byte_len).into_iter().collect();
                buf[..byte_len].copy_from_slice(&bytes[..byte_len]);
                Ok(byte_len)
            }
            Err(_) => {
                Err(io::Error::new(io::ErrorKind::WouldBlock, "Unable to get lock"))
            }
        }
    }
}
