use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Note that there are a number of downsides to this approach, the comments
    // below detail how to improve the portability of these commands.
    Command::new("cargo")
        .args(&["wasix", "build"])
        .current_dir("../../driver/filestore")
        .status()
        .unwrap();

    println!("cargo::rerun-if-changed=../../driver/filestore/src/main.rs");
}
