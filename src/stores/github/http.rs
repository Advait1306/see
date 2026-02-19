use std::sync::OnceLock;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Returns a static tokio runtime for HTTP operations.
/// GPUI uses smol as its async executor, so reqwest (which depends on tokio)
/// needs a dedicated runtime. This follows the same pattern as Zed's ReqwestClient.
pub fn http_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Failed to initialize HTTP runtime")
    })
}

pub fn http_client() -> reqwest::Client {
    let _guard = http_runtime().enter();
    reqwest::Client::new()
}
