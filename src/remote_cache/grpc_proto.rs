//! gRPC proto definitions for Remote Execution API
//! This module contains the compiled protobuf definitions

pub mod proto {
    tonic::include_proto!("build.bazel.remote.execution.v2");
    
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("remote_execution_descriptor");
}