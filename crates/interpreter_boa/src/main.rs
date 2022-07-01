#![deny(warnings)]

use boa_engine::*;
use lazy_static::lazy_static;
// use lazy_static::lazy_static;
use secure_js_sandbox_protocol::{Input, Output};
use std::error::Error;
use std::io::prelude::*;
use std::io::stdin;
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
    static ref JS_CONTEXT: SharedContext = {
        let ctx = Context::default();
        SharedContext(Arc::new(Mutex::new(ctx)))
    };
}

fn main() -> Result<(), Box<dyn Error>> {
    run_internal()
}

#[no_mangle]
pub fn run() -> () {
    // Run can be called multiple times, allowing you to continue execution
    // after adding more requests to stdin
    run_internal().unwrap()
}


fn run_internal() -> Result<(), Box<dyn Error>> {
    for line in stdin().lock().lines() {
        let line = line?;
        let input = Input::from_str(&line)?;
        let result = evaluate_js(&input.script);
        println!("{}", result.to_string()?);
    }
    Ok(())
}

fn evaluate_js(script: &str) -> Output {
    let mut ctx = JS_CONTEXT.0.lock().expect("Failed to get lock on JS context");
    match ctx.eval(script) {
        Ok(v) => match v.to_json(&mut ctx) {
            Ok(value) => Output::Ok { value },
            Err(_) => Output::Err {
                message: format!("Result {} could not be serialized to JSON", v.display()),
            },
        },
        Err(e) => match get_error_str(&e, &mut ctx) {
            Some(str) => Output::Err { message: str },
            None => Output::Err {
                message: format!("Non error thrown: {}", e.display()),
            },
        },
    }
}

// TODO: once Boa supports it, we should be able to update this to return a stack trace
fn get_error_str(err: &JsValue, ctx: &mut Context) -> Option<String> {
    let obj = match err.as_object() {
        Some(obj) => obj,
        None => return None,
    };
    if !obj.is_error() {
        return None;
    }
    let to_string = match obj.get("toString", &mut *ctx) {
        Ok(to_string) => to_string,
        Err(_) => return None,
    };
    let to_string = match to_string.as_callable() {
        Some(to_string) => to_string,
        None => return None,
    };
    let result = match to_string.call(&err, &[], &mut *ctx) {
        Ok(result) => result,
        Err(_) => return None,
    };
    match result.as_string() {
        Some(str) => Some(str.to_string()),
        None => None,
    }
}

struct SharedContext (Arc<Mutex<Context>>);
unsafe impl Send for SharedContext {}
unsafe impl Sync for SharedContext {}
