extern crate prost_build;

fn main() {
    prost_build::compile_protos(
        &["src/gcm/checkin.proto", "src/mcs.proto"],
        &["src/", "src/gcm/"],
    )
    .unwrap();
}
