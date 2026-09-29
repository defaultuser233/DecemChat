//! 本地服务：提供内嵌的前端产物，并把 `/api/chat` 交给 [`crate::proxy`]。

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;

/// 前端构建产物，编译期内嵌（生成文件见 `build.rs`）。
mod assets {
    include!(concat!(env!("OUT_DIR"), "/dist.rs"));
}

/// 请求体上限：图片以 base64 塞在 JSON 里，与线上服务端同样放宽。
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// 本地服务的共享状态。
pub struct AppState {
    /// 上游接口地址。
    pub upstream: String,
    /// 直连上游（例如百炼）时用的 Key；走本项目服务端代理时不需要。
    pub api_key: Option<String>,
    /// 复用的 HTTP 客户端。
    pub client: reqwest::Client,
}

impl AppState {
    /// 按上游地址建状态：Key 从环境变量读（`API_KEY` / `DASHSCOPE_API_KEY` / `NETLIFY_API_KEY`）。
    pub fn new(upstream: String) -> Self {
        Self {
            upstream,
            api_key: decem_core::api_key_from_env(),
            client: reqwest::Client::new(),
        }
    }
}

/// 组装路由：一个代理接口 + 其余全是静态产物。
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/chat", post(crate::proxy::chat))
        .fallback(serve_asset)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

/// 静态产物：找不到具体文件就回 `index.html`（SPA 回退，线上服务端也是这么做的）。
async fn serve_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = assets::DIST.get_file(path) {
        return file_response(path, file.contents());
    }
    match assets::DIST.get_file("index.html") {
        Some(index) => file_response("index.html", index.contents()),
        None => (StatusCode::NOT_FOUND, "前端产物缺失").into_response(),
    }
}

/// 按扩展名给出 Content-Type。
fn file_response(path: &str, bytes: &'static [u8]) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut response = Body::from(bytes).into_response();
    if let Ok(value) = HeaderValue::from_str(mime.as_ref()) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
}
