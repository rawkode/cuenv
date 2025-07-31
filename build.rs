use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=libcue-bridge/bridge.go");
    println!("cargo:rerun-if-changed=libcue-bridge/bridge.h");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set by cargo"));
    let bridge_dir = PathBuf::from("libcue-bridge");

    // Create the bridge directory if it doesn't exist
    std::fs::create_dir_all(&bridge_dir).expect("Failed to create libcue-bridge directory");

    // Build the Go shared library with CGO
    let mut cmd = Command::new("go");
    cmd.current_dir(&bridge_dir).arg("build");

    // Use vendor directory if it exists (for Nix builds)
    if bridge_dir.join("vendor").exists() {
        cmd.arg("-mod=vendor");
    }

    // Check if we're building for musl
    let target = env::var("TARGET").unwrap_or_default();
    let output_path = out_dir.join("libcue_bridge.a");
    let output_str = output_path
        .to_str()
        .expect("Failed to convert output path to string");

    if target.contains("musl") {
        // Set musl-specific environment variables
        // Use CC from environment if set, otherwise default to musl-gcc
        if let Ok(cc) = env::var("CC") {
            cmd.env("CC", cc);
        } else {
            cmd.env("CC", "musl-gcc");
        }
        cmd.env("CGO_ENABLED", "1");

        cmd.args([
            "-buildmode=c-archive",
            "-tags",
            "netgo,osusergo,static_build",
            "-ldflags",
            "-extldflags '-static'",
            "-o",
            output_str,
            "bridge.go",
        ]);
    } else {
        cmd.args(["-buildmode=c-archive", "-o", output_str, "bridge.go"]);
    }

    let status = cmd
        .status()
        .expect("Failed to build Go shared library. Make sure Go is installed.");

    if !status.success() {
        panic!("Failed to build libcue bridge");
    }

    // Tell Rust where to find the library
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=cue_bridge");

    // Link system libraries that Go runtime needs
    let target = env::var("TARGET").expect("TARGET not set by cargo");

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
<<<<<<< HEAD
||||||| parent of 51c29a8 (feat: add TUI for interactive task execution with fallback output)

    // Compile protobuf for remote cache server
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .file_descriptor_set_path(out_dir.join("remote_execution_descriptor.bin"))
        .compile(
            &["src/remote_cache/remote_execution.proto"],
            &["src/remote_cache"],
        )
        .expect("Failed to compile protobuf");
=======

    // Compile protobuf for remote cache server (optional if protoc is available)
    if let Ok(_protoc_path) = env::var("PROTOC") {
        println!("cargo:rustc-cfg=feature=\"remote-cache\"");
        tonic_build::configure()
            .build_server(true)
            .build_client(true)
            .file_descriptor_set_path(out_dir.join("remote_execution_descriptor.bin"))
            .compile(
                &["src/remote_cache/remote_execution.proto"],
                &["src/remote_cache"],
            )
            .expect("Failed to compile protobuf");
    } else if Command::new("protoc").arg("--version").output().is_ok() {
        println!("cargo:rustc-cfg=feature=\"remote-cache\"");
        tonic_build::configure()
            .build_server(true)
            .build_client(true)
            .file_descriptor_set_path(out_dir.join("remote_execution_descriptor.bin"))
            .compile(
                &["src/remote_cache/remote_execution.proto"],
                &["src/remote_cache"],
            )
            .expect("Failed to compile protobuf");
    } else {
        println!("cargo:warning=protoc not found - remote cache functionality will be disabled");
        println!("cargo:warning=Install protobuf-compiler or set PROTOC environment variable to enable remote cache");
    }
>>>>>>> 51c29a8 (feat: add TUI for interactive task execution with fallback output)
}
