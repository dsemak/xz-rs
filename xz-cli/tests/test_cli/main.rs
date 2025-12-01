pub mod common;
mod unxz;
mod xz;
mod xzcat;
mod xzdec;

const KB: usize = 1024;
const MB: usize = 1024 * KB;

const MAX_DURATION: std::time::Duration = std::time::Duration::from_secs(30);

/// Macro to generate an async test case with a timeout
#[macro_export]
macro_rules! add_test {
    ($name:ident, $test:expr) => {
        #[tokio::test(flavor = "current_thread")]
        async fn $name() {
            tokio::time::timeout($crate::MAX_DURATION, $test)
                .await
                .expect("timeout expired");
        }
    };
}
