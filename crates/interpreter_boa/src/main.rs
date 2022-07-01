#![deny(warnings)]

use std::error::Error;
use std::io::stdin;
use std::io::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    for line in stdin().lock().lines() {
        println!("Will evaluate: {}", line.unwrap());
    }
    Ok(())
}