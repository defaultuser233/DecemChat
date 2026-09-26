//! 应用层消息类型，以及「组装成一次模型请求」的编排逻辑。

use serde::{Deserialize, Serialize};

use crate::models::{is_vision_model, supports_temperature};
use crate::prompt::SYSTEM_PROMPT;

/// 图片消息没有文字描述时使用的默认提示。
pub const DEFAULT_IMAGE_PROMPT: &str = "看看这张图片～";

/// 随请求发送的历史消息条数（不含本轮用户消息）。
pub const HISTORY_LIMIT: usize = 10;

/// 界面消息的角色：只可能是用户或 Decem。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// 用户。
    User,
    /// Decem。
    Assistant,
}

/// 下发给模型的消息角色，比 [`MessageRole`] 多一个 `system`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 角色设定。
    System,
    /// 用户。
    User,
    /// 模型。
    Assistant,
}

impl From<MessageRole> for Role {
    fn from(role: MessageRole) -> Self {
        match role {
            MessageRole::User => Role::User,
            MessageRole::Assistant => Role::Assistant,
        }
    }
}

/// 一条界面消息，可直接序列化进本地存储。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// 消息 ID，同时用作图片存储的键。
    pub id: String,
    /// 角色。
    pub role: MessageRole,
    /// 正文；Decem 消息流式生成期间可能为空。
    #[serde(default)]
    pub content: String,
    /// 毫秒时间戳。
    pub timestamp: i64,
    /// 图片（data URL 或远端 URL）。
    ///
    /// 只在内存里保留：图片单独存在本地图片库，JSON 里不写 base64，
    /// 因此这里用 `serde(skip)` 而不是「存之前先手动清空」。
    #[serde(skip)]
    pub image_url: Option<String>,
    /// 图片存在本地图片库中的标记。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    /// 只发给模型、不在界面展示的额外上下文。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Message {
    /// 构造一条纯文本消息。
    pub fn text(
        id: impl Into<String>,
        role: MessageRole,
        content: impl Into<String>,
        timestamp: i64,
    ) -> Self {
        Self {
            id: id.into(),
            role,
            content: content.into(),
            timestamp,
            image_url: None,
            image_path: None,
            hint: None,
        }
    }

    /// 供模型阅读的文本：正文为空时回退到默认图片提示，`hint` 另起一行追加。
    pub fn prompt_text(&self) -> String {
        let base = if self.content.is_empty() {
            DEFAULT_IMAGE_PROMPT
        } else {
            self.content.as_str()
        };
        match self.hint.as_deref().filter(|hint| !hint.is_empty()) {
            Some(hint) => format!("{base}\n{hint}"),
            None => base.to_owned(),
        }
    }
}

/// 取最近 `limit` 条消息；超长历史只保留尾部。
pub fn recent_history(messages: &[Message], limit: usize) -> &[Message] {
    &messages[messages.len().saturating_sub(limit)..]
}

/// 图片地址。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
    /// 支持 `data:image/...;base64,` 与公开 URL 两种形式。
    pub url: String,
}

/// 多模态消息的其中一段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// 文本段。
    Text {
        /// 文本内容。
        text: String,
    },
    /// 图片段。
    ImageUrl {
        /// 图片地址。
        image_url: ImageUrl,
    },
}

/// 一条下发给模型的消息内容：纯文本，或带图的多模态分段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    /// 纯文本。
    Text(String),
    /// 多模态分段。
    Parts(Vec<ContentPart>),
}

/// 下发给模型的一条消息。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 角色。
    pub role: Role,
    /// 内容。
    pub content: ChatContent,
}

impl ChatMessage {
    /// 构造纯文本消息。
    pub fn text(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: ChatContent::Text(content.into()),
        }
    }
}

/// 组装出完整的一轮请求消息：角色提示词 + 最近 10 条历史。
///
/// 视觉模型收到图片时用多模态分段；不支持视觉的模型则退化为 `[图片] …` 文本，
/// 与前端 `formatMessages` 的行为保持一致。
///
/// # Examples
///
/// ```
/// use decem_core::{Message, MessageRole, Role, format_messages};
///
/// let messages = [Message::text("1", MessageRole::User, "嗷呜～", 0)];
/// let formatted = format_messages(&messages, "qwen3.8-flash");
///
/// assert_eq!(formatted.len(), 2); // 角色提示词 + 1 条用户消息
/// assert_eq!(formatted[0].role, Role::System);
/// ```
pub fn format_messages(messages: &[Message], model: &str) -> Vec<ChatMessage> {
    let history = recent_history(messages, HISTORY_LIMIT);
    let vision = is_vision_model(model);

    let mut formatted = Vec::with_capacity(history.len() + 1);
    formatted.push(ChatMessage::text(Role::System, SYSTEM_PROMPT));

    for message in history {
        let role = Role::from(message.role);
        let text = message.prompt_text();
        let image_url = message.image_url.as_deref().filter(|url| !url.is_empty());

        let content = match image_url {
            Some(url) if vision => ChatContent::Parts(vec![
                ContentPart::Text { text },
                ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: url.to_owned(),
                    },
                },
            ]),
            // 非视觉模型看不到图，至少把「这里有一张图」的语义留给它
            Some(_) => ChatContent::Text(format!("[图片] {text}")),
            None => ChatContent::Text(text),
        };
        formatted.push(ChatMessage { role, content });
    }

    formatted
}

/// 一次聊天请求（客户端 → 服务端，以及服务端 → 服务商共用的字段）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    /// 模型 ID。
    pub model: String,
    /// 已编排好的消息序列。
    pub messages: Vec<ChatMessage>,
    /// 采样温度；`None` 表示不下发。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

impl ChatRequest {
    /// 由界面消息构造请求，并按模型能力决定是否带上 `temperature`。
    ///
    /// # Examples
    ///
    /// ```
    /// use decem_core::{ChatRequest, Message, MessageRole};
    ///
    /// let messages = [Message::text("1", MessageRole::User, "你好", 0)];
    ///
    /// // kimi-k3 不接受 temperature，因此这里不带该参数
    /// assert!(ChatRequest::new("kimi-k3", &messages).temperature.is_none());
    /// // 其它模型按默认 0.8 下发
    /// assert_eq!(ChatRequest::new("qwen3.8-flash", &messages).temperature, Some(0.8));
    /// ```
    pub fn new(model: impl Into<String>, messages: &[Message]) -> Self {
        let model = model.into();
        let temperature = supports_temperature(&model).then_some(0.8);
        let messages = format_messages(messages, &model);
        Self {
            model,
            messages,
            temperature,
        }
    }

    /// 校验必填字段，用于服务端入口。
    pub fn validate(&self) -> crate::Result<()> {
        if self.model.trim().is_empty() {
            return Err(crate::Error::InvalidRequest("缺少 model".to_owned()));
        }
        if self.messages.is_empty() {
            return Err(crate::Error::InvalidRequest("缺少 messages".to_owned()));
        }
        Ok(())
    }

    /// 转成发给服务商的请求体：补齐流式与长度上限等固定字段。
    pub fn to_upstream(&self) -> UpstreamRequest<'_> {
        UpstreamRequest {
            model: &self.model,
            messages: &self.messages,
            stream: true,
            max_tokens: 1500,
            enable_thinking: false,
            temperature: self.temperature,
        }
    }
}

/// 发往 DashScope 兼容接口的请求体。
#[derive(Debug, Serialize)]
pub struct UpstreamRequest<'a> {
    /// 模型 ID。
    pub model: &'a str,
    /// 消息序列。
    pub messages: &'a [ChatMessage],
    /// 固定为 `true`：全程流式，避免长回复超时。
    pub stream: bool,
    /// 单次回复的最大 token 数。
    pub max_tokens: u32,
    /// 关闭思考过程，保持回复风格稳定。
    pub enable_thinking: bool,
    /// 采样温度，按需下发。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_with_image(id: &str, content: &str) -> Message {
        Message {
            id: id.to_owned(),
            role: MessageRole::User,
            content: content.to_owned(),
            timestamp: 0,
            image_url: Some("data:image/png;base64,AAAA".to_owned()),
            image_path: Some("stored".to_owned()),
            hint: None,
        }
    }

    #[test]
    fn system_prompt_always_comes_first() {
        let messages = vec![Message::text("1", MessageRole::User, "嗷呜", 0)];
        let formatted = format_messages(&messages, "qwen3.8-flash");
        assert_eq!(formatted.len(), 2);
        assert_eq!(formatted[0].role, Role::System);
        assert_eq!(
            formatted[0].content,
            ChatContent::Text(SYSTEM_PROMPT.to_owned())
        );
    }

    #[test]
    fn history_is_truncated_to_last_ten() {
        let messages: Vec<Message> = (0..25)
            .map(|i| Message::text(i.to_string(), MessageRole::User, format!("m{i}"), i))
            .collect();
        let formatted = format_messages(&messages, "deepseek-v4-pro");
        assert_eq!(formatted.len(), HISTORY_LIMIT + 1);
        assert_eq!(
            formatted[1].content,
            ChatContent::Text("m15".to_owned()),
            "应保留最后 10 条"
        );
    }

    #[test]
    fn vision_model_receives_image_parts() {
        let messages = vec![user_with_image("1", "看这个")];
        let formatted = format_messages(&messages, "qwen3.8-flash");
        match &formatted[1].content {
            ChatContent::Parts(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(
                    parts[0],
                    ContentPart::Text {
                        text: "看这个".to_owned()
                    }
                );
                assert!(matches!(parts[1], ContentPart::ImageUrl { .. }));
            }
            other => panic!("应为多模态分段，实际为 {other:?}"),
        }
    }

    #[test]
    fn text_only_model_gets_image_placeholder() {
        let messages = vec![user_with_image("1", "看这个")];
        let formatted = format_messages(&messages, "deepseek-v4-pro");
        assert_eq!(
            formatted[1].content,
            ChatContent::Text("[图片] 看这个".to_owned())
        );
    }

    #[test]
    fn empty_content_and_hint_are_handled() {
        let mut message = user_with_image("1", "");
        message.hint = Some("仅模型的上下文".to_owned());
        let formatted = format_messages(&[message], "deepseek-v4-pro");
        assert_eq!(
            formatted[1].content,
            ChatContent::Text("[图片] 看看这张图片～\n仅模型的上下文".to_owned())
        );
    }

    #[test]
    fn temperature_is_dropped_for_models_without_support() {
        let messages = vec![Message::text("1", MessageRole::User, "hi", 0)];
        assert_eq!(ChatRequest::new("kimi-k3", &messages).temperature, None);
        assert_eq!(
            ChatRequest::new("qwen3.8-flash", &messages).temperature,
            Some(0.8)
        );
    }

    #[test]
    fn upstream_body_carries_fixed_stream_fields() {
        let request =
            ChatRequest::new("kimi-k3", &[Message::text("1", MessageRole::User, "hi", 0)]);
        let body = serde_json::to_value(request.to_upstream()).expect("序列化应成功");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 1500);
        assert_eq!(body["enable_thinking"], false);
        assert!(body.get("temperature").is_none());
        assert_eq!(body["messages"][0]["role"], "system");
    }

    #[test]
    fn validate_rejects_missing_fields() {
        let empty = ChatRequest {
            model: String::new(),
            messages: vec![],
            temperature: None,
        };
        assert!(empty.validate().is_err());
    }
}
