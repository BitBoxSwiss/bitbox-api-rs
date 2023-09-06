use async_trait::async_trait;

#[cfg_attr(feature = "multithreaded", async_trait)]
#[cfg_attr(not(feature="multithreaded"), async_trait(?Send))]
pub trait Runtime {
    async fn sleep(dur: std::time::Duration);
}

/// Assumes no particular async runtime. Uses std::thread::sleep to sleep.
/// Useful if using futures::executor::block_on() to run synchronously.
pub struct DefaultRuntime;

#[cfg_attr(feature = "multithreaded", async_trait)]
#[cfg_attr(not(feature="multithreaded"), async_trait(?Send))]
impl Runtime for DefaultRuntime {
    async fn sleep(dur: std::time::Duration) {
        std::thread::sleep(dur);
    }
}

#[cfg(feature = "tokio")]
pub struct TokioRuntime;

#[cfg(feature = "tokio")]
#[cfg_attr(feature = "multithreaded", async_trait)]
#[cfg_attr(not(feature="multithreaded"), async_trait(?Send))]
impl Runtime for TokioRuntime {
    async fn sleep(dur: std::time::Duration) {
        tokio::time::sleep(dur).await
    }
}
