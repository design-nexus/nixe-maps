use std::future::Future;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

pub static TOKIO: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("nixe-maps-io")
        .build()
        .expect("Failed to initialize background Tokio runtime")
});

pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    TOKIO.spawn(future);
}
