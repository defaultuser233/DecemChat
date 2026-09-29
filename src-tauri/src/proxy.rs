//! `/api/chat` 透传：前端发来的请求原样转给上游，SSE 流原样回灌。

use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;

use crate::server::AppState;

/// 转发一次聊天请求。请求体已经是前端拼好的（含系统提示词、模型、历史），这里不改写。
pub async fn chat(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let mut request = state
        .client
        .post(&state.upstream)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "text/event-stream");
    if let Some(key) = state.api_key.as_deref() {
        request = request.bearer_auth(key);
    }

    let upstream = match request.body(body).send().await {
        Ok(response) => response,
        Err(err) => {
            return (StatusCode::BAD_GATEWAY, format!("无法连接上游接口：{err}")).into_response();
        }
    };

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    if !status.is_success() {
        let text = upstream.text().await.unwrap_or_default();
        return (status, text).into_response();
    }

    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("text/event-stream; charset=utf-8"));

    // 不攒 buffer：上游来多少就往回送多少，流式才是一个字一个字蹦
    let stream = upstream.bytes_stream().map_err(std::io::Error::other);
    let mut response = Body::from_stream(stream).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    response
}
