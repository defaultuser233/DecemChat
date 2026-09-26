//! 基于 `reqwest` 的流式聊天客户端。
//!
//! 全程使用 rustls，不依赖系统 OpenSSL，因此 Windows / macOS / Linux（含 musl 镜像）
//! 都能用同一份代码构建。请求体、错误提取与 SSE 处理都与前端的 `aiApi.ts`、
//! 后端的 `server/index.ts` 保持同一契约。

use std::time::Duration;

use futures_util::stream::{self, Stream, StreamExt};
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::message::ChatRequest;
use crate::sse::{SseDecoder, SseOutcome, parse_sse_line};

/// DashScope 兼容模式接口地址。
pub const API_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

/// 依次尝试的环境变量名，先命中者生效。
pub const API_KEY_ENV_VARS: &[&str] = &["API_KEY", "DASHSCOPE_API_KEY", "NETLIFY_API_KEY"];

type ByteStream = Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>;

use std::pin::Pin;

/// 聊天客户端。
///
/// `api_key` 允许为空：把 `api_url` 指向自建代理（由代理持有 Key）时，
/// 请求就不带 `Authorization` —— 客户端不需要知道任何密钥。
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    api_key: String,
    api_url: String,
}

impl Client {
    /// 用显式提供的 API Key 创建客户端；空字符串表示不带鉴权头。
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("decemchat/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            http,
            api_key: api_key.into(),
            api_url: API_URL.to_owned(),
        })
    }

    /// 从环境变量读取 API Key 并创建客户端（**直连服务商**时用）。
    ///
    /// 顺序为 `API_KEY` → `DASHSCOPE_API_KEY` → `NETLIFY_API_KEY`，与前端部署说明一致。
    /// 走自建代理时不需要它：用 [`Client::new`] 传空字符串即可。
    pub fn from_env() -> Result<Self> {
        let api_key = api_key_from_env().ok_or(Error::MissingApiKey)?;
        Self::new(api_key)
    }

    /// 覆盖接口地址：便于自建网关或跑本地假上游。
    #[must_use]
    pub fn with_api_url(mut self, api_url: impl Into<String>) -> Self {
        self.api_url = api_url.into();
        self
    }

    /// 发起一次流式请求，返回可逐块消费的 [`ChatStream`]。
    ///
    /// 上游返回 SSE 时逐块透传；万一只返回 JSON，则退化为「一次给完整内容」的流。
    pub async fn open_stream(&self, request: &ChatRequest) -> Result<ChatStream> {
        request.validate()?;

        let mut builder = self.http.post(&self.api_url);
        if !self.api_key.trim().is_empty() {
            builder = builder.bearer_auth(self.api_key.trim());
        }
        let response = builder.json(&request.to_upstream()).send().await?;

        if !response.status().is_success() {
            return Err(upstream_error(response).await);
        }

        let is_sse = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("text/event-stream"));

        if is_sse {
            Ok(ChatStream::streaming(response.bytes_stream()))
        } else {
            let text = response.text().await.unwrap_or_default();
            extract_buffered_content(&text)
                .map(ChatStream::buffered)
                .ok_or(Error::EmptyContent)
        }
    }

    /// 一次性取回完整回复（内部仍然走流式，避免长回复超时）。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo() -> Result<(), decem_core::Error> {
    /// use decem_core::{ChatRequest, Client, Message, MessageRole};
    ///
    /// // 依次尝试 API_KEY → DASHSCOPE_API_KEY → NETLIFY_API_KEY
    /// let client = Client::from_env()?;
    /// let messages = [Message::text("1", MessageRole::User, "你好", 0)];
    /// let reply = client
    ///     .complete(&ChatRequest::new("qwen3.8-flash", &messages))
    ///     .await?;
    /// println!("{reply}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn complete(&self, request: &ChatRequest) -> Result<String> {
        self.open_stream(request).await?.collect_content().await
    }
}

/// 按约定顺序从环境变量里取 API Key；空白值视为未设置。
pub fn api_key_from_env() -> Option<String> {
    API_KEY_ENV_VARS.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    })
}

/// 一次流式对话。
pub struct ChatStream {
    upstream: ByteStream,
    decoder: SseDecoder,
    pending: std::collections::VecDeque<String>,
    full: String,
    finished: bool,
}

impl ChatStream {
    fn streaming(
        upstream: impl Stream<Item = reqwest::Result<bytes::Bytes>> + Send + 'static,
    ) -> Self {
        Self {
            upstream: Box::pin(upstream),
            decoder: SseDecoder::new(),
            pending: std::collections::VecDeque::new(),
            full: String::new(),
            finished: false,
        }
    }

    fn buffered(content: String) -> Self {
        Self {
            upstream: Box::pin(stream::empty()),
            decoder: SseDecoder::new(),
            pending: std::iter::once(content.clone()).collect(),
            full: content,
            finished: false,
        }
    }

    /// 取下一个增量文本；返回 `None` 表示流正常结束。
    ///
    /// 全程一个 token 都没收到时报 [`Error::EmptyContent`]。
    pub async fn next_chunk(&mut self) -> Result<Option<String>> {
        loop {
            if let Some(delta) = self.pending.pop_front() {
                return Ok(Some(delta));
            }
            if self.finished {
                return self.finish();
            }

            match self.upstream.next().await {
                Some(Ok(bytes)) => {
                    let lines = self.decoder.push(&bytes);
                    self.absorb(lines);
                }
                Some(Err(err)) => return Err(Error::Transport(err)),
                None => {
                    self.finished = true;
                    if let Some(tail) = self.decoder.finish() {
                        self.absorb(std::iter::once(tail));
                    }
                }
            }
        }
    }

    /// 到目前为止累计的完整回复。
    pub fn content(&self) -> &str {
        &self.full
    }

    /// 把剩余增量全部读完，返回完整回复。
    pub async fn collect_content(mut self) -> Result<String> {
        while self.next_chunk().await?.is_some() {}
        Ok(self.full)
    }

    fn absorb<I: IntoIterator<Item = String>>(&mut self, lines: I) {
        for line in lines {
            match parse_sse_line(&line) {
                SseOutcome::Delta(delta) => {
                    self.full.push_str(&delta);
                    self.pending.push_back(delta);
                }
                // 整段内容覆盖此前累计的增量
                SseOutcome::Full(full) => self.full = full,
                SseOutcome::Done => self.finished = true,
                SseOutcome::Ignore => {}
            }
        }
    }

    fn finish(&self) -> Result<Option<String>> {
        if self.full.is_empty() {
            Err(Error::EmptyContent)
        } else {
            Ok(None)
        }
    }
}

/// 非流式回退：从 JSON 响应体里挖出内容。
fn extract_buffered_content(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    let candidate = value
        .get("content")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
        })
        .or_else(|| value.pointer("/choices/0/text").and_then(Value::as_str))?;
    (!candidate.is_empty()).then(|| candidate.to_owned())
}

/// 把上游的错误响应翻译成可展示的错误。
async fn upstream_error(response: reqwest::Response) -> Error {
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "AI provider request failed".to_owned());

    Error::Upstream { status, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_buffered_json_fallbacks() {
        assert_eq!(
            extract_buffered_content(r#"{"content":"甲"}"#).as_deref(),
            Some("甲")
        );
        assert_eq!(
            extract_buffered_content(r#"{"choices":[{"message":{"content":"乙"}}]}"#).as_deref(),
            Some("乙")
        );
        assert_eq!(
            extract_buffered_content(r#"{"choices":[{"text":"丙"}]}"#).as_deref(),
            Some("丙")
        );
        assert_eq!(
            extract_buffered_content(r#"{"choices":[{"message":{"content":""}}]}"#),
            None
        );
        assert_eq!(extract_buffered_content("not json"), None);
    }

    #[test]
    fn rejects_empty_api_key_from_env() {
        // 不依赖真实环境变量：只验证空字符串不会被当成有效 Key
        assert!(
            API_KEY_ENV_VARS
                .iter()
                .all(|name| std::env::var(name).map_or(true, |value| !value.trim().is_empty()))
                || api_key_from_env().is_some()
        );
    }

    #[tokio::test]
    async fn buffered_stream_yields_content_then_ends() {
        let mut stream = ChatStream::buffered("嗷呜～".to_owned());
        assert_eq!(
            stream.next_chunk().await.unwrap().as_deref(),
            Some("嗷呜～")
        );
        assert_eq!(stream.next_chunk().await.unwrap(), None);
        assert_eq!(stream.content(), "嗷呜～");
    }

    #[tokio::test]
    async fn empty_stream_reports_empty_content() {
        let mut stream = ChatStream::streaming(stream::empty());
        let err = stream.next_chunk().await.expect_err("空流应报错");
        assert!(matches!(err, Error::EmptyContent));
    }
}
