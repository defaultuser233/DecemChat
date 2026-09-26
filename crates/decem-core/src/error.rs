//! 统一错误类型。
//!
//! 库代码不 `panic`：所有可预期失败都走 [`Error`]，由调用方决定如何呈现。

/// 核心库与上层应用共用的错误类型。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// 环境变量里找不到任何可用的 API Key。
    #[error("服务端未配置 API Key：API_KEY / DASHSCOPE_API_KEY / NETLIFY_API_KEY 均未设置")]
    MissingApiKey,

    /// 与 AI 服务商的网络层交互失败（DNS、TLS、连接中断等）。
    #[error("无法连接 AI 服务商：{0}")]
    Transport(#[from] reqwest::Error),

    /// 服务商返回非 2xx，附带从响应体中提取到的可读原因。
    #[error("AI 服务商返回错误（HTTP {status}）：{message}")]
    Upstream {
        /// 上游 HTTP 状态码；无法获知时为 502。
        status: u16,
        /// 可直接展示给用户的原因。
        message: String,
    },

    /// 流结束但整段回复为空。
    #[error("服务器未返回有效内容")]
    EmptyContent,

    /// 调用方传来的请求体不合法。
    #[error("请求不合法：{0}")]
    InvalidRequest(String),

    /// 本地持久化（设置 / 聊天记录 / 图片）失败。
    #[error("本地存储失败：{0}")]
    Storage(String),
}

/// 本 crate 使用的 `Result` 别名。
pub type Result<T> = std::result::Result<T, Error>;
