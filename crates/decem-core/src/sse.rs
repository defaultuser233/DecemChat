//! DashScope 兼容模式 SSE 流的增量解析。
//!
//! 上游是按行推送的 `data: {json}\n\n`，网络分包会把一行切断，
//! 因此这里按「字节进、整行进」的方式解码：[`SseDecoder`] 负责拼行，
//! [`parse_sse_line`] 负责把一行翻译成 [`SseOutcome`]。

use serde::Deserialize;

/// 一行的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseOutcome {
    /// 增量文本，逐字追加到已有回复后面。
    Delta(String),
    /// 整段内容（部分非流式响应只给 `message.content`）。
    Full(String),
    /// `data: [DONE]`，流正常结束。
    Done,
    /// 与本应用无关的行：空行、注释、非法 JSON、没有内容的 chunk。
    Ignore,
}

/// 增量式行解码器：吃字节，吐完整行。
///
/// UTF-8 多字节字符可能被网络分包切断，但 `\n` 是 ASCII 且 UTF-8 自同步，
/// 因此「以 `\n` 结尾的一行」一定是完整的合法 UTF-8 序列。
#[derive(Debug, Default)]
pub struct SseDecoder {
    buf: Vec<u8>,
}

impl SseDecoder {
    /// 新建解码器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 送入一段字节，返回其中已经完整成行的部分（不含行尾换行符与 `\r`）。
    ///
    /// # Examples
    ///
    /// ```
    /// use decem_core::SseDecoder;
    ///
    /// let mut decoder = SseDecoder::new();
    /// assert!(decoder.push(b"data: {\"a\"").is_empty()); // 半行先攒着
    /// assert_eq!(decoder.push(b":1}\n"), ["data: {\"a\":1}"]);
    /// ```
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);

        let mut lines = Vec::new();
        let mut consumed = 0;
        for (index, byte) in self.buf.iter().enumerate() {
            if *byte == b'\n' {
                lines.push(Self::to_line(&self.buf[consumed..index]));
                consumed = index + 1;
            }
        }
        if consumed > 0 {
            self.buf.drain(..consumed);
        }
        lines
    }

    /// 流结束：取出最后一段没有换行结尾的残留内容。
    pub fn finish(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let line = Self::to_line(&self.buf);
        self.buf.clear();
        Some(line)
    }

    fn to_line(bytes: &[u8]) -> String {
        let line = String::from_utf8_lossy(bytes);
        line.strip_suffix('\r').unwrap_or(&line).to_owned()
    }
}

/// 上游 chunk 结构，只声明我们真正用到的字段。
#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Option<StreamContent>,
    #[serde(default)]
    message: Option<StreamContent>,
}

#[derive(Debug, Deserialize)]
struct StreamContent {
    #[serde(default)]
    content: Option<String>,
}

/// 解析一行 SSE 文本。
///
/// 与前端 `consumeSSE` 一致：先看 `delta.content`，再看 `message.content`。
///
/// # Examples
///
/// ```
/// use decem_core::{SseOutcome, parse_sse_line};
///
/// let delta = r#"data: {"choices":[{"delta":{"content":"嗷"}}]}"#;
/// assert_eq!(parse_sse_line(delta), SseOutcome::Delta("嗷".to_owned()));
/// assert_eq!(parse_sse_line("data: [DONE]"), SseOutcome::Done);
/// assert_eq!(parse_sse_line(": keep-alive"), SseOutcome::Ignore);
/// ```
pub fn parse_sse_line(line: &str) -> SseOutcome {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("data:") else {
        return SseOutcome::Ignore;
    };
    let data = rest.trim();
    if data == "[DONE]" {
        return SseOutcome::Done;
    }

    let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) else {
        return SseOutcome::Ignore;
    };
    let Some(choice) = chunk.choices.into_iter().next() else {
        return SseOutcome::Ignore;
    };

    if let Some(delta) = choice
        .delta
        .and_then(|d| d.content)
        .filter(|d| !d.is_empty())
    {
        return SseOutcome::Delta(delta);
    }
    if let Some(full) = choice
        .message
        .and_then(|m| m.content)
        .filter(|c| !c.is_empty())
    {
        return SseOutcome::Full(full);
    }
    SseOutcome::Ignore
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reassembles_lines_split_across_packets() {
        let mut decoder = SseDecoder::new();
        assert!(decoder.push(b"data: {\"a\"").is_empty());
        let lines = decoder.push(b":1}\n\ndata: [DONE]\n");
        assert_eq!(lines, vec!["data: {\"a\":1}", "", "data: [DONE]"]);
        assert_eq!(decoder.finish(), None);
    }

    #[test]
    fn keeps_partial_multibyte_character_until_complete() {
        let mut decoder = SseDecoder::new();
        let text = "嗷呜".as_bytes();
        // 故意在第二个汉字中间切断
        assert!(decoder.push(&text[..5]).is_empty());
        let lines = decoder.push(&[text[5], b'\n']);
        assert_eq!(lines, vec!["嗷呜"]);
    }

    #[test]
    fn strips_carriage_return_and_flushes_tail() {
        let mut decoder = SseDecoder::new();
        assert_eq!(decoder.push(b"data: x\r\n"), vec!["data: x"]);
        assert!(decoder.push(b"data: y").is_empty());
        assert_eq!(decoder.finish().as_deref(), Some("data: y"));
        assert_eq!(decoder.finish(), None);
    }

    #[test]
    fn parses_delta_and_message_content() {
        let delta = r#"data: {"choices":[{"delta":{"content":"嗷"}}]}"#;
        assert_eq!(parse_sse_line(delta), SseOutcome::Delta("嗷".to_owned()));

        let full = r#"data: {"choices":[{"message":{"content":"整段"}}]}"#;
        assert_eq!(parse_sse_line(full), SseOutcome::Full("整段".to_owned()));
    }

    #[test]
    fn ignores_noise() {
        assert_eq!(parse_sse_line(""), SseOutcome::Ignore);
        assert_eq!(parse_sse_line(": keep-alive"), SseOutcome::Ignore);
        assert_eq!(parse_sse_line("data: not-json"), SseOutcome::Ignore);
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[]}"#),
            SseOutcome::Ignore
        );
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[{"delta":{"content":""}}]}"#),
            SseOutcome::Ignore
        );
    }

    #[test]
    fn recognises_done_marker() {
        assert_eq!(parse_sse_line("data: [DONE]"), SseOutcome::Done);
        assert_eq!(parse_sse_line("  data:  [DONE]  "), SseOutcome::Done);
    }
}
