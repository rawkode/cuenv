// Build script for workspace-level configuration
// The actual FFI bridge building is now handled by cuenv-libcue-ffi-bridge crate

fn main() {
    // Workspace-level build configuration can go here if needed
    println!("cargo:rerun-if-changed=build.rs");
}
