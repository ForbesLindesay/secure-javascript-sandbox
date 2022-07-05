#![deny(warnings)]

use anyhow::Result;
use lazy_static::lazy_static;
use secure_js_sandbox_protocol::{EvaluationResult, JsonValue};
use std::any::Any;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::{collections::vec_deque::VecDeque, sync::Arc};
use wasi_common::file::{FdFlags, Filestat, OFlags};
use wasi_common::pipe::{ReadPipe, WritePipe};
use wasi_common::{WasiCtx, WasiDir, WasiFile}; // , WasiFile, file::FileType
use wasmtime::{Config, Engine, Func, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Memory};
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

#[derive(Debug)]
pub enum JsRunOutput {
    Ok {
        ctx: JsSandboxContext,
        result: Option<JsonValue>,
        stdout: String,
        stderr: String,
    },
    RuntimeError {
        ctx: JsSandboxContext,
        message: String,
        stdout: String,
        stderr: String,
    },
    OutOfFuel {
        stdout: String,
        stderr: String,
    },
    OutOfMemory {
        stdout: String,
        stderr: String,
    },
}

pub struct JsSandboxContext {
    store: Store<StoreState>,
    stdin: StdinPipe,
    stdout: StdoutPipe,
    stderr: StdoutPipe,
    output: StdoutPipe,
    run: Func,
    memory: Memory,
}
impl std::fmt::Debug for JsSandboxContext {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "JsSandboxContext")
    }
}

impl JsSandboxContext {
    pub fn new(limits: &MemoryLimits) -> JsSandboxContext {
        let stdin = StdinPipe::new();
        let stdout = StdoutPipe::new();
        let stderr = StdoutPipe::new();

        let mut ctx = WasiCtxBuilder::new()
            .stdin(Box::new(ReadPipe::new(stdin.clone())))
            .stdout(Box::new(WritePipe::new(stdout.clone())))
            .stderr(Box::new(WritePipe::new(stderr.clone())))
            .arg("/output.json")
            .unwrap()
            .build();

        let mut files: HashMap<String, StdoutPipe> = HashMap::with_capacity(1);
        let output = StdoutPipe::new();
        files.insert("output.json".to_string(), output.clone());
        ctx.push_preopened_dir(Box::new(VirtualDirectory(files)), "/")
            .unwrap();

        let mut store = Store::new(
            &WASM_ENGINE,
            StoreState {
                wasi: ctx,
                limits: StoreLimitsBuilder::new()
                    .memory_size(limits.max_bytes)
                    .table_elements(limits.max_table_elements)
                    .build(),
            },
        );
        store.limiter(|s| &mut s.limits);
        let instance = WASM_LINKER.instantiate(&mut store, &WASM_MODULE).unwrap();
        let run = instance
            .get_func(&mut store, "run")
            .expect("Missing \"run\" fn in WASM module");
        let memory = instance.get_memory(&mut store, "memory")
        .expect("Missing \"memory\" in WASM module");
        JsSandboxContext {
            store,
            stdin,
            stdout,
            stderr,
            output,
            run,
            memory,
        }
    }

    pub fn add_fuel(&mut self, fuel: u64) -> () {
        self.store.add_fuel(fuel).unwrap()
    }
    pub fn fuel_consumed(&mut self) -> u64 {
        self.store.fuel_consumed().unwrap()
    }
    pub fn fuel_remaining(&mut self) -> u64 {
        match self.store.consume_fuel(0) {
            Ok(v) => v,
            Err(_) => 0,
        }
    }
    pub fn memory_consumed(&mut self) -> usize {
        self.memory.data_size(&mut self.store)
    }

    pub fn run(mut self, script: &str) -> Result<JsRunOutput> {
        self.stdin.write_str(script)?;
        match self.run.call(&mut self.store, &mut [], &mut []) {
            Ok(_) => {},
            Err(err) => {
                let stdout = self.stdout.read_all_to_string()?;
                let stderr = self.stderr.read_all_to_string()?;
                if err.to_string().contains("all fuel consumed by WebAssembly") {
                    return Ok(JsRunOutput::OutOfFuel {
                        stdout,
                        stderr,
                    });
                }
                if err.to_string().contains("rust_oom") {
                    return Ok(JsRunOutput::OutOfMemory {
                        stdout,
                        stderr,
                    });
                }
                return Err(err.into());
            }
        }

        let stdout = self.stdout.read_all_to_string()?;
        let stderr = self.stderr.read_all_to_string()?;
        let output = EvaluationResult::from_str(&self.output.read_all_to_string()?)?;

        let output = match output {
            EvaluationResult::Ok(value) => JsRunOutput::Ok {
                ctx: self,
                result: value,
                stdout,
                stderr,
            },
            EvaluationResult::Err(message) => {
                if message.contains("memory allocation failed because the memory allocator returned a error") {
                    JsRunOutput::OutOfMemory { stdout, stderr }
                } else {
                    JsRunOutput::RuntimeError {
                        ctx: self,
                        message,
                        stdout,
                        stderr,
                    }
                }
            },
        };
        Ok(output)
    }
}
struct StdoutPipe(Arc<Mutex<VecDeque<u8>>>);

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
    fn read_all_to_string(&mut self) -> Result<String> {
        let bytes = self.read_all()?;
        let str = String::from_utf8(bytes)?;
        Ok(str)
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

struct StdinPipe(Arc<Mutex<VecDeque<u8>>>);

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
struct VirtualDirectory(HashMap<String, StdoutPipe>);

#[wiggle::async_trait]
impl WasiDir for VirtualDirectory {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn open_file(
        &self,
        _symlink_follow: bool,
        path: &str,
        _oflags: OFlags,
        _read: bool,
        _write: bool,
        _fdflags: FdFlags,
    ) -> Result<Box<dyn WasiFile>, wasi_common::Error> {
        match self.0.get(&path.to_string()) {
            Some(file) => Ok(Box::new(WritePipe::new(Box::new(file.clone())))),
            None => Err(wasi_common::Error::msg("Access denied")),
        }
        // Err(wasi_common::Error::msg("Access denied"))
        // panic!("Not implemented open_file({:?})", (_symlink_follow, path, _oflags,_read, _write, _fdflags))
        // Ok(Box::new(WritePipe::new(Box::new(StdoutPipe::new()))))
    }
    async fn open_dir(
        &self,
        _symlink_follow: bool,
        _path: &str,
    ) -> Result<Box<dyn WasiDir>, wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn create_dir(&self, _path: &str) -> Result<(), wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn readdir(
        &self,
        _cursor: wasi_common::dir::ReaddirCursor,
    ) -> Result<
        Box<
            dyn Iterator<Item = Result<wasi_common::dir::ReaddirEntity, wasi_common::Error>> + Send,
        >,
        wasi_common::Error,
    > {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn symlink(&self, _old_path: &str, _new_path: &str) -> Result<(), wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn remove_dir(&self, _path: &str) -> Result<(), wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn unlink_file(&self, _path: &str) -> Result<(), wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn read_link(&self, _path: &str) -> Result<PathBuf, wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn get_filestat(&self) -> Result<Filestat, wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn get_path_filestat(
        &self,
        _path: &str,
        _follow_symlinks: bool,
    ) -> Result<Filestat, wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn rename(
        &self,
        _path: &str,
        _dest_dir: &dyn WasiDir,
        _dest_path: &str,
    ) -> Result<(), wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn hard_link(
        &self,
        _path: &str,
        _target_dir: &dyn WasiDir,
        _target_path: &str,
    ) -> Result<(), wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
    async fn set_times(
        &self,
        _path: &str,
        _atime: Option<wasi_common::SystemTimeSpec>,
        _mtime: Option<wasi_common::SystemTimeSpec>,
        _follow_symlinks: bool,
    ) -> Result<(), wasi_common::Error> {
        Err(wasi_common::Error::msg("Access denied"))
    }
}
