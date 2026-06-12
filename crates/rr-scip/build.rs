#![allow(clippy::panic)]

const SCIP_PROTO_PATH: &str = "../../submodules/scip/scip.proto";

fn main() {
    println!("cargo:rerun-if-changed={SCIP_PROTO_PATH}");

    prost_build::Config::new()
        .disable_comments(["."])
        .compile_protos(&[SCIP_PROTO_PATH], &["../.."])
        .unwrap_or_else(|source| {
            panic!("failed to generate SCIP protobuf bindings from {SCIP_PROTO_PATH}: {source}");
        });
}
