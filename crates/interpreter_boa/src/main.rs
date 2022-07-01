#![deny(warnings)]

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("hello world");
    Ok(())
}