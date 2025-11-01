fn main() {
    prost_build::compile_protos(&["src/agent.proto"], &["src"]).unwrap();
}
