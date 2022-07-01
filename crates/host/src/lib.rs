#![deny(warnings)]

use std::error::Error;

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("hello world");
    Ok(())
}