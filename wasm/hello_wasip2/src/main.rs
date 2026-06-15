#![allow(warnings)]
#![allow(unused)]

use std::{env, io};
use std::io::BufRead;

fn main() {
    let stdin = io::stdin();
    let handle = stdin.lock();

    // Read line by line
    let name = if let Some(Ok(line)) = handle.lines().next() {
        line
    } else {
        "wasip2".to_string()
    };

    println!("Hello {}", name);
}

