use std::sync::{Arc, OnceLock};

use futures::FutureExt as _;
use gpui_http_client::{AsyncBody, HttpClient, Inner};

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

/// A GPUI-compatible HTTP client wrapping reqwest.
/// Register via `cx.set_http_client(Arc::new(GpuiHttpClient::new()))`.
pub struct GpuiHttpClient {
    client: reqwest::Client,
    handle: tokio::runtime::Handle,
}

impl GpuiHttpClient {
    pub fn new() -> Self {
        let handle = http_runtime().handle().clone();
        let _guard = handle.enter();
        Self {
            client: reqwest::Client::new(),
            handle,
        }
    }

    pub fn as_arc() -> Arc<dyn HttpClient> {
        Arc::new(Self::new())
    }
}

impl HttpClient for GpuiHttpClient {
    fn type_name(&self) -> &'static str {
        "GpuiHttpClient"
    }

    fn user_agent(&self) -> Option<&gpui_http_client::http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&gpui_http_client::Url> {
        None
    }

    fn send(
        &self,
        req: gpui_http_client::http::Request<AsyncBody>,
    ) -> futures::future::BoxFuture<
        'static,
        anyhow::Result<gpui_http_client::http::Response<AsyncBody>>,
    > {
        let (parts, body) = req.into_parts();
        let uri = parts.uri.to_string();
        log::info!("[GpuiHttpClient] send: {} {}", parts.method, uri);

        let mut request = self.client.request(parts.method, &uri);
        request = request.headers(parts.headers);

        let request = request.body(match body.0 {
            Inner::Empty => reqwest::Body::default(),
            Inner::Bytes(cursor) => cursor.into_inner().into(),
            Inner::AsyncReader(_) => reqwest::Body::default(),
        });

        let handle = self.handle.clone();
        async move {
            let response = match handle.spawn(async { request.send().await }).await? {
                Ok(r) => r,
                Err(e) => {
                    log::error!("[GpuiHttpClient] request failed for {}: {}", uri, e);
                    return Err(e.into());
                }
            };

            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string();
            let headers = response.headers().clone();

            log::info!(
                "[GpuiHttpClient] response: {} status={} content-type={}",
                uri,
                status,
                content_type
            );

            let bytes = match handle.spawn(async { response.bytes().await }).await? {
                Ok(b) => b,
                Err(e) => {
                    log::error!("[GpuiHttpClient] body read failed for {}: {}", uri, e);
                    return Err(e.into());
                }
            };

            log::info!(
                "[GpuiHttpClient] got {} bytes for {}",
                bytes.len(),
                uri
            );

            let mut builder = gpui_http_client::http::Response::builder().status(status);
            *builder.headers_mut().unwrap() = headers;
            builder
                .body(AsyncBody::from(bytes.to_vec()))
                .map_err(|e| anyhow::anyhow!(e))
        }
        .boxed()
    }
}
