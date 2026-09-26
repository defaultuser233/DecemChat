//! Decem Chat 的跨平台核心库。
//!
//! 本 crate 不依赖任何平台特有 API（无网络服务端口绑定、无文件系统路径假设），
//! 只提供三端（服务端 / 桌面端 / 命令行）共用的纯逻辑：
//!
//! - [`models`]：可用模型表与用户设置
//! - [`message`]：消息类型与「去往 AI 服务商」的请求编排
//! - [`prompt`]：Decem 的人格提示词
//! - [`sse`]：DashScope 兼容模式 SSE 流的增量解析
//! - [`provider`]：基于 `reqwest` 的流式聊天客户端（rustls，无 OpenSSL 依赖）

pub mod error;
pub mod message;
pub mod models;
pub mod prompt;
pub mod provider;
pub mod sse;

pub use error::{Error, Result};
pub use message::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, DEFAULT_IMAGE_PROMPT, HISTORY_LIMIT,
    ImageUrl, Message, MessageRole, Role, UpstreamRequest, format_messages, recent_history,
};
pub use models::{
    AIModel, AVAILABLE_MODELS, CHAR_AVATARS, ChatSettings, DEFAULT_MODEL, USER_AVATARS,
    default_settings, find_model, is_vision_model, model_display_name, random_char_avatar,
    random_user_avatar, supports_temperature,
};
pub use prompt::SYSTEM_PROMPT;
pub use provider::{API_URL, ChatStream, Client, api_key_from_env};
pub use sse::{SseDecoder, SseOutcome, parse_sse_line};
