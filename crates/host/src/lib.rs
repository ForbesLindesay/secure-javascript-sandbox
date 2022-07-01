#![deny(warnings)]

use lazy_static::lazy_static;
use std::error::Error;
use std::io;
use std::sync::Mutex;
///, any::Any, io::{self}};
use std::{collections::vec_deque::VecDeque, sync::Arc};
use wasi_common::WasiCtx; // , WasiFile, file::FileType
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Func};
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
    // let input = Input {
    //     script: "Hello World".to_string(),
    // };
    // let stdin = WasmFileInputBuffer(format!("{}\n", input.to_string()?).as_bytes().to_vec());
    // let stdin = ReadPipe::from(format!("{}\n", input.to_string()?));
    let mut stdin = StdinPipe::new();
    // stdin.write_line(&input.to_string()?)?;
    // let stdout = WritePipe::new_in_memory();
    let mut stdout = StdoutPipe::new();
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
                .stdout(Box::new(WritePipe::new(stdout.clone())))
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
    let run = instance
        .get_func(&mut store, "run")
        .expect("Missing \"run\" fn in WASM module");

    store.add_fuel(fuel)?;
    run_in_sandbox(&mut store, &run, &mut stdin, &mut stdout, "hello world")?;
    run_in_sandbox(&mut store, &run, &mut stdin, &mut stdout, "hello world")?;
    run_in_sandbox(&mut store, &run, &mut stdin, &mut stdout, "hello world")?;
    run_in_sandbox(&mut store, &run, &mut stdin, &mut stdout, "hello world")?;

    // store.add_fuel(fuel)?;
    // run.call(&mut store, &mut [], &mut [])?;

    // stdin.write_line(&input.to_string()?)?;
    // run.call(&mut store, &mut [], &mut [])?;
    // run.call(&mut store, &mut [], &mut [])?;
    // let contents = String::from_utf8(
    //     stdout
    //         // .try_into_inner()
    //         // .expect("sole remaining reference to WritePipe")
    //         .read_all()?,
    // )?;
    // println!("contents of stdout: {:?}", contents);
    Ok(())
}

pub struct StdoutPipe(Arc<Mutex<VecDeque<u8>>>);

impl Clone for StdoutPipe {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl StdoutPipe {
    fn new() -> StdoutPipe {
        StdoutPipe(Arc::new(Mutex::new(VecDeque::new())))
    }
    fn read_all(&mut self) -> io::Result<Vec<u8>> {
        match self.0.lock() {
            Ok(mut inner) => Ok(inner.drain(..).into_iter().collect()),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Unable to get lock",
            )),
        }
    }
}
impl io::Write for StdoutPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0.lock() {
            Ok(mut inner) => {
                inner.extend(buf);
                Ok(buf.len())
            }
            Err(_) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Unable to get lock",
            )),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct StdinPipe(Arc<Mutex<VecDeque<u8>>>);

impl Clone for StdinPipe {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl StdinPipe {
    fn new() -> StdinPipe {
        StdinPipe(Arc::new(Mutex::new(VecDeque::new())))
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self.0.lock() {
            Ok(mut inner) => {
                inner.extend(bytes);
                Ok(())
            }
            Err(_) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Unable to get lock",
            )),
        }
    }
    fn write_str(&mut self, str: &str) -> io::Result<()> {
        self.write(str.as_bytes())
    }
    fn write_line(&mut self, str: &str) -> io::Result<()> {
        self.write_str(&format!("{}\n", str))
    }
}
impl io::Read for StdinPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.lock() {
            Ok(mut inner) => {
                let byte_len = buf.len().min(inner.len());
                let bytes: Vec<u8> = inner.drain(..byte_len).into_iter().collect();
                buf[..byte_len].copy_from_slice(&bytes[..byte_len]);
                Ok(byte_len)
            }
            Err(_) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Unable to get lock",
            )),
        }
    }
}

pub fn run_in_sandbox<T>(store: &mut Store<T>, run: &Func, stdin: &mut StdinPipe, stdout: &mut StdoutPipe, script: &str) -> Result<(), Box<dyn Error>> {
    let input = Input {
        script: script.to_string()
    };
    stdin.write_line(&input.to_string()?)?;
    run.call(store, &mut [], &mut [])?;
    let contents = String::from_utf8(
        stdout
            // .try_into_inner()
            // .expect("sole remaining reference to WritePipe")
            .read_all()?,
    )?;
    println!("contents of stdout: {:?}", contents);
    Ok(())
}