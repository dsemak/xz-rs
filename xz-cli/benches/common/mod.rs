pub mod datasets;
pub mod fixtures;
pub mod targets;

pub use datasets::{DatasetKind, MIB};
pub use fixtures::{
    prepare_compress_fixture, prepare_decode_file_fixture, prepare_decode_stdout_fixture,
    run_checked,
};
pub use targets::{benchmark_targets, locate_cargo_binary, locate_system_binary, CodecFormat};
