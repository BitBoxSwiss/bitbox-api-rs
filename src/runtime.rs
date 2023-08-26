use async_trait::async_trait;

#[async_trait]
pub trait Runtime {
    async fn sleep(dur: std::time::Duration);
}

/// Assumes no particular async runtime. Uses std::thread::sleep to sleep.
/// Useful if using futures::executor::block_on() to run synchronously.
pub struct DefaultRuntime;

#[async_trait]
impl Runtime for DefaultRuntime {
    async fn sleep(dur: std::time::Duration) {
        std::thread::sleep(dur);
    }
}

#[cfg(feature = "tokio")]
pub struct TokioRuntime;

#[cfg(feature = "tokio")]
#[async_trait]
impl Runtime for TokioRuntime {
    async fn sleep(dur: std::time::Duration) {
        tokio::time::sleep(dur).await
    }
}
