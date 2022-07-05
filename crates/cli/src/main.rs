#![deny(warnings)]

use anyhow::Result;
use clap::{Parser};
use host::{JsRunOutput, JsSandboxContext, MemoryLimits};
use secure_js_sandbox_host as host;
use std::{io::Write, process::{ExitCode, Termination}};

#[derive(Parser, Clone)]
struct Cli {
    /// Limit to 50MB per sandbox by default
    #[clap(long, value_parser, default_value_t = 50 * 1024 * 1024)]
    memory_limit_bytes: usize,

    /// I think this limits number of methods/exports in table, defaults to 10,000
    #[clap(long, value_parser, default_value_t = 10_000)]
    max_table_elements: u32,

    /// The "fuel" for CPU operations. 440 million is approximately 100ms on
    /// my MacBook Pro.
    #[clap(long, value_parser, default_value_t = 440_000_000)]
    fuel: u64,

    /// Do not print output
    #[clap(short, long, value_parser, default_value_t = false)]
    quiet: bool,

    /// The script to run
    #[clap(short = 's', long = "script")]
    script: String,

    /// How many times to run the script before exiting (defaults to 1)
    #[clap(long, value_parser, default_value_t = 1)]
    repeat: u32,

    /// How many threads to use (defaults to 1)
    #[clap(long, value_parser, default_value_t = 1)]
    threads: u32,

    /// Reuse sandbox between repeated invocations of the script (on a single thread)
    #[clap(long, value_parser, default_value_t = false)]
    reuse: bool,
}

fn main() -> JsCliResult {
    let cli_options = Cli::parse();

    match run(cli_options) {
        Ok(_) => JsCliResult::Ok,
        Err(e) => {
            
            match e.downcast::<JsCliResult>() {
                Ok(result) => result,
                Err(e) => JsCliResult::UnknownError(e),
            }
        }
    }
}

fn run(
    Cli {
        memory_limit_bytes,
        max_table_elements,
        fuel,
        quiet,
        script,
        repeat,
        threads,
        reuse,
    }: Cli,
) -> Result<()> {
    if threads > 1 {
        let per_thread = {
            let d = repeat / threads;
            let r = repeat % threads;
            if r > 0 {
                d + 1
            } else {
                d
            }
        };
        let mut threads = Vec::new();
        let mut repeat_remaining = repeat;
        while repeat_remaining > 0 {
            let thread_cli_options = Cli {
                memory_limit_bytes,
                max_table_elements,
                fuel,
                quiet,
                script: script.clone(),
                repeat: if repeat_remaining > per_thread {
                    repeat_remaining = repeat_remaining - per_thread;
                    per_thread
                } else {
                    let res = repeat_remaining;
                    repeat_remaining = 0;
                    res
                },
                threads: 1,
                reuse,
            };
            threads.push(std::thread::spawn(move || {
                run(thread_cli_options).unwrap();
            }));
        }
        for t in threads {
            t.join().unwrap();
        }
        return Ok(());
    }

    let limits = MemoryLimits{ max_bytes: memory_limit_bytes, max_table_elements };
    if reuse {
        let mut sandbox = JsSandboxContext::new(&limits);
        sandbox.add_fuel(fuel * (repeat as u64));
        for _ in 0..repeat {
            sandbox = run_once_in_sandbox(sandbox, &script, &quiet)?;
        }
    } else {
        for _ in 0..repeat {
            let mut sandbox = JsSandboxContext::new(&limits);
            sandbox.add_fuel(fuel);
            run_once_in_sandbox(sandbox, &script, &quiet)?;
        }
    }

    Ok(())
}

fn run_once_in_sandbox(
    sandbox: host::JsSandboxContext,
    script: &str,
    quiet: &bool,
) -> Result<host::JsSandboxContext> {
    let result = sandbox.run(script)?;
    let ctx = match result {
        JsRunOutput::Ok {
            ctx,
            result,
            stdout,
            stderr,
        } => {
            if !quiet {
                std::io::stderr().write(stdout.as_bytes())?;
                std::io::stderr().write(stderr.as_bytes())?;
                println!(
                    "{}",
                    match result {
                        Some(v) => v.to_string(),
                        None => "".to_string(),
                    }
                );
            }
            Ok(ctx)
        },
        JsRunOutput::RuntimeError {
            ctx: _,
            message,
            stdout,
            stderr,
        } => {
            Err(JsCliResult::RuntimeError { message, stdout, stderr })
        },
        JsRunOutput::OutOfFuel { stdout, stderr } => {
            Err(JsCliResult::OutOfFuel { stdout, stderr })
        },
        JsRunOutput::OutOfMemory { stdout, stderr } => {
            Err(JsCliResult::OutOfMemory { stdout, stderr })
        },
    }?;
    Ok(ctx)
}

#[derive(Debug)]
enum JsCliResult {
    Ok,
    UnknownError(anyhow::Error),
    RuntimeError {
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
impl std::error::Error for JsCliResult {}
impl std::fmt::Display for JsCliResult {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl Termination for JsCliResult {
    fn report(self) -> ExitCode {
        match self {
            JsCliResult::Ok => ExitCode::from(0),
            JsCliResult::UnknownError(e) => {
                eprintln!("{}", e);
                ExitCode::from(1)
            },
            JsCliResult::RuntimeError {
                message,
                stdout,
                stderr,
            } => {
                std::io::stderr().write(stdout.as_bytes()).unwrap();
                std::io::stderr().write(stderr.as_bytes()).unwrap();
                eprintln!("");
                eprintln!("## Runtime Error ##");
                eprintln!("");
                eprintln!("{}", message);
                ExitCode::from(100)
            },
            JsCliResult::OutOfFuel { stdout, stderr } => {
                std::io::stderr().write(stdout.as_bytes()).unwrap();
                std::io::stderr().write(stderr.as_bytes()).unwrap();
                eprintln!("Ran out of fuel");
                ExitCode::from(101)
            },
            JsCliResult::OutOfMemory { stdout, stderr } => {
                std::io::stderr().write(stdout.as_bytes()).unwrap();
                std::io::stderr().write(stderr.as_bytes()).unwrap();
                eprintln!("Ran out of memory");
                ExitCode::from(102)
            },
        }
    }
}
