#![deny(warnings)]

use secure_js_sandbox_protocol::{Input, Output};
use std::error::Error;
use std::io::prelude::*;
use std::io::stdin;

fn main() -> Result<(), Box<dyn Error>> {
    for line in stdin().lock().lines() {
        let line = line?;
        let input = Input::from_str(&line)?;
        let result = Output::Err {
            message: format!("Not implemented: {:?}", input),
        };
        println!("{}", result.to_string()?);
    }
    Ok(())
}
