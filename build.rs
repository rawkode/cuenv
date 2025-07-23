use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=libcue-bridge/bridge.go");
    println!("cargo:rerun-if-changed=libcue-bridge/bridge.h");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let bridge_dir = PathBuf::from("libcue-bridge");

    // Create the bridge directory if it doesn't exist
    std::fs::create_dir_all(&bridge_dir).expect("Failed to create libcue-bridge directory");

    // Build the Go shared library with CGO
    let mut cmd = Command::new("go");
    cmd.current_dir(&bridge_dir)
        .arg("build");
    
    // Use vendor directory if it exists (for Nix builds)
    if bridge_dir.join("vendor").exists() {
        cmd.arg("-mod=vendor");
    }
    
    cmd.args(&[
        "-buildmode=c-archive",  // Use static linking instead
        "-o",
        out_dir.join("libcue_bridge.a").to_str().unwrap(),
        "bridge.go",
    ]);
    
    let status = cmd.status()
        .expect("Failed to build Go shared library. Make sure Go is installed.");

    if !status.success() {
        panic!("Failed to build libcue bridge");
    }

    // Tell Rust where to find the library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=cue_bridge");
    
    // Link system libraries that Go runtime needs
    let target = env::var("TARGET").unwrap();
    
    if target.contains("windows") {
        // Windows-specific libraries
        println!("cargo:rustc-link-lib=ws2_32");
        println!("cargo:rustc-link-lib=userenv");
        println!("cargo:rustc-link-lib=ntdll");
        println!("cargo:rustc-link-lib=winmm");
    } else {
        // Unix-like systems
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=dl");
        
        if target.contains("apple-darwin") {
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
        }
    }
}