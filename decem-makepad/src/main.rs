//! Decem Chat 的 Makepad 版 —— 阶段 2：真实网络。
//!
//! 上游是 `https://api.valedecem.top:8443/api/chat`（nginx → 服务端，Key 在服务器侧）。
//! 网络走 `makepad-platform` 自带的**阻塞 HTTP**（`blocking_http`，HTTPS、只走一跳），
//! 响应体边到边写进一个 `std::io::Write`，那个 writer 就地喂 SSE 解析器。
//! 解析器与请求编排复用 `decem-core`（只取纯逻辑，不带 reqwest）。
//!
//! 线程模型照 `makepad/examples/aichat` 的 proven 模式：
//! 工作线程 → channel → `SignalToUI` 唤醒 UI → `handle_event` 里轮询 → 流式动画。

pub use makepad_widgets;

use makepad_widgets::desktop_style::{self, DesktopStyle, StyleSheet};
use makepad_widgets::makepad_platform::file_dialogs::{FileDialog, FileDialogAction};
use makepad_widgets::makepad_platform::makepad_network::blocking_http::{
    self, CancelToken, Limits, Request,
};
use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;

use decem_core::{
    parse_sse_line, ChatRequest, Message, MessageRole, SseDecoder, SseOutcome, DEFAULT_MODEL,
};
use makepad_base64::{base64_encode, BASE64_STANDARD};

use std::path::Path;
use std::sync::{mpsc, Arc, RwLock};
use std::time::Duration;

app_main!(App);

/// 角色。项目命名规范：Decem 一侧叫 char（不用 bot / assistant）。
#[derive(Clone, Copy, PartialEq)]
enum Role {
    User,
    Char,
}

/// 一张选好、准备随消息发出去的图。
///
/// 字节留着给界面显示（Makepad 自带 PNG/JPEG/WebP/GIF/BMP 解码，不需要 `image` crate），
/// data URL 留着发请求——两者都在选图那一刻算一次，之后不再碰。
struct ImageAttachment {
    bytes: Arc<[u8]>,
    /// `data:image/png;base64,...`，直接进 `messages[i].content` 的多模态数组
    data_url: String,
}

struct Msg {
    role: Role,
    text: String,
    image: Option<ImageAttachment>,
}

/// 全局聊天状态（照 aichat 的做法：`ChatList` 要在 `draw_walk` 里读它）。
struct Chat {
    messages: Vec<Msg>,
    streaming: String,
    is_streaming: bool,
}

static CHAT: RwLock<Chat> = RwLock::new(Chat {
    messages: Vec::new(),
    streaming: String::new(),
    is_streaming: false,
});

const WELCOME: &str = "🦊 狐狸来啦～ (≧▽≦)/～ ❤️ 你终于来找我玩啦！嗷呜～";

/// 往聊天里追一条 char 消息，并把流式状态收干净。
/// 现阶段用它呈现错误（之后要不要单独的错误样式再定）。
fn push_message(text: String) {
    let mut chat = CHAT.write().unwrap();
    chat.streaming.clear();
    chat.is_streaming = false;
    chat.messages.push(Msg {
        role: Role::Char,
        text,
        image: None,
    });
}

/// 单张图的上限。服务端把请求体放宽到 20MB，这里留足余量。
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// 按扩展名给 MIME。Makepad 解码靠魔数（不看你给的 MIME），这个 MIME 只进 data URL 给上游看。
fn mime_for(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    }
    .to_owned()
}

/// 图片按 data URL 内嵌进请求体（服务端为此把体积上限放宽到 20MB）。
fn to_data_url(mime: &str, bytes: &[u8]) -> String {
    let encoded = base64_encode(bytes, &BASE64_STANDARD);
    // base64 的输出一定是 ASCII，转换不会失败
    format!("data:{mime};base64,{}", String::from_utf8_lossy(&encoded))
}

/// 后端地址。API Key 在服务器侧，客户端不带任何鉴权头。
/// 与 `crates/decem-core`（`provider::API_URL`）及 `crates/decem-app` 的值保持一致。
const API_URL: &str = "https://api.valedecem.top:8443/api/chat";

/// 默认模型。模型表与能力查询都在 `decem_core::models` 里，阶段 3 再做切换面板。
const MODEL: &str = DEFAULT_MODEL;

// ---------------------------------------------------------------- 真实上游

enum WorkerEvent {
    /// 一段增量文本。
    Delta(String),
    /// 流正常结束。
    Done,
    /// 失败原因，直接当作一条消息展示。
    Failed(String),
}

/// 把响应体喂给 SSE 解析器，再把增量推给 UI。
///
/// `blocking_http` 按网络到达的顺序调 `write`，所以这里看到的就是真实的**分包边界**：
/// 一个事件被拆到两个包、半个多字节字符落在包尾，都由 `SseDecoder` 兜着（它有单元测试）。
struct SseSink {
    decoder: SseDecoder,
    tx: mpsc::Sender<WorkerEvent>,
    cancel: CancelToken,
    /// 响应体开头的一段：非 2xx 时用它抠出可读的错误原因。
    preview: Vec<u8>,
}

/// 错误体保留上限，放得下一段 JSON 说明就够。
const ERROR_PREVIEW_BYTES: usize = 2048;

impl std::io::Write for SseSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.cancel.is_cancelled() {
            // Esc 中断：用 IO 错误让阻塞中的请求尽快收场，
            // 调用方靠 cancel token 区分「被取消」与「真失败」
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }
        if self.preview.len() < ERROR_PREVIEW_BYTES {
            let room = ERROR_PREVIEW_BYTES - self.preview.len();
            self.preview.extend_from_slice(&buf[..buf.len().min(room)]);
        }

        let lines = self.decoder.push(buf);
        for line in lines {
            match parse_sse_line(&line) {
                SseOutcome::Delta(text) | SseOutcome::Full(text) => {
                    if self.tx.send(WorkerEvent::Delta(text)).is_err() {
                        // UI 侧已经不要了（channel 断了）：让请求尽早停下来
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "receiver gone",
                        ));
                    }
                    SignalToUI::set_ui_signal();
                }
                SseOutcome::Done | SseOutcome::Ignore => {}
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// 从非 2xx 的响应体里抠出一句能给用户看的原因。
///
/// 服务端固定回 `{"error": "..."}`，这里顺带兼容 `{"error":{"message":...}}` 与 `{"message":...}`
/// （和 `decem-core` 的 `provider.rs`、网页版的 `aiApi.ts` 取法一致）。
fn upstream_error(status: u16, body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let reason = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| {
                    error
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| error.get("message")?.as_str().map(str::to_owned))
                })
                .or_else(|| value.get("message")?.as_str().map(str::to_owned))
        })
        .unwrap_or_else(|| text.trim().chars().take(200).collect());
    if reason.is_empty() {
        format!("上游返回 HTTP {status}")
    } else {
        format!("上游返回 HTTP {status}：{reason}")
    }
}

struct Worker {
    rx: mpsc::Receiver<WorkerEvent>,
    cancel: CancelToken,
}

impl Worker {
    /// 起一个线程发请求，SSE 增量边收边推给 UI。
    fn spawn_chat(body: Vec<u8>) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = CancelToken::new();
        let token = cancel.clone();

        std::thread::spawn(move || {
            let limits = Limits {
                // 默认 60s / 1MB 对长回复太紧；nginx 那边给的是 1 小时
                total_timeout: Duration::from_secs(600),
                max_body_bytes: 8 * 1024 * 1024,
                ..Limits::default()
            };
            let request = match Request::post(API_URL)
                .json_body(body)
                .map(|request| request.limits(limits).cancel_token(token.clone()))
            {
                Ok(request) => request,
                Err(err) => {
                    let _ = tx.send(WorkerEvent::Failed(format!("请求构造失败：{err}")));
                    SignalToUI::set_ui_signal();
                    return;
                }
            };

            let mut sink = SseSink {
                decoder: SseDecoder::new(),
                tx: tx.clone(),
                cancel: token.clone(),
                preview: Vec::new(),
            };
            let outcome = blocking_http::request_to_writer_no_redirect(request, &mut sink);

            match outcome {
                Ok(response) if (200..300).contains(&response.status) => {
                    // 流结束时补吐最后一段没有换行结尾的残留（否则最后一行会丢）
                    if let Some(line) = sink.decoder.finish() {
                        if let SseOutcome::Delta(text) | SseOutcome::Full(text) =
                            parse_sse_line(&line)
                        {
                            let _ = tx.send(WorkerEvent::Delta(text));
                        }
                    }
                    let _ = tx.send(WorkerEvent::Done);
                }
                Ok(response) => {
                    let _ = tx.send(WorkerEvent::Failed(upstream_error(
                        response.status,
                        &sink.preview,
                    )));
                }
                // 被取消：已流出的内容照样定格成一条消息
                Err(_) if token.is_cancelled() => {
                    let _ = tx.send(WorkerEvent::Done);
                }
                Err(err) => {
                    let _ = tx.send(WorkerEvent::Failed(format!("连接失败：{err}")));
                }
            }
            SignalToUI::set_ui_signal();
        });

        Self { rx, cancel }
    }

    fn cancel(&self) {
        self.cancel.cancel();
    }

    fn poll(&self) -> Vec<WorkerEvent> {
        self.rx.try_iter().collect()
    }
}

// ---------------------------------------------------------------- 气泡定宽

/// `ChatList` 画每一项之前把它放进来，`Bubble` 在自己 `draw_walk` 里取。
/// 绘制是同一线程、逐项同步进行的，所以一个「当前项」槽位就够。
static BUBBLE_BODY: RwLock<Option<(String, bool)>> = RwLock::new(None);

/// 正文里有没有块级元素。代码块和表格的宽度不由正文字号决定，量文字没有意义。
fn has_block_content(text: &str) -> bool {
    text.contains("```") || text.contains("\n|")
}

/// 这条消息要不要用满宽。
/// 带图的要有地方放图；带代码块/表格的量文字没意义。其余按量出来的文字宽度收窄。
fn wants_full_width(msg: &Msg) -> bool {
    msg.image.is_some() || has_block_content(&msg.text)
}

/// 量宽容器：先按上限把正文量一遍，再用**量出来的实际宽度**给气泡定宽，
/// 于是短消息贴着文字、长消息到上限换行。
///
/// 为什么非自己量不可：`Markdown`/`TextFlow` 的换行宽度取的是外层 turtle 的内宽
/// （`widgets/families/rich_text/src/text_flow.rs:2554-2559`），而 `Fit` 的 turtle
/// 没有确定宽度（NaN）就不换行 —— 长文本会直接溢出被裁。Makepad 自带的
/// `ChatBubble` 用的是同一个办法（`widgets/families/extras/src/chat.rs:903-924`）：
/// 按上限把文字排一遍，拿用到的宽度决定气泡多宽。这里照它写，内容仍交给
/// `Markdown` 画（官方注释也说了：需要标题和链接的消息是「气泡形容器里的
/// Markdown」，不是更大的气泡 —— chat.rs:31-34）。
#[derive(Script, ScriptHook, Widget)]
pub struct Bubble {
    #[deref]
    view: View,
    /// 只用来量正文宽度。字体必须与 Markdown 正文一致
    #[live]
    draw_text: DrawText,
    /// 气泡最宽多少
    #[live(560.0)]
    cap_width: f64,
    /// 气泡的左右内边距之和（与模板里 RoundedView 的 padding 对应）
    #[live(28.0)]
    pad_x: f64,
}

impl Widget for Bubble {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let (body, blocky) = BUBBLE_BODY
            .read()
            .map(|slot| slot.clone())
            .unwrap_or(None)
            .unwrap_or_default();

        // 此刻最上层还是外层行（Fill），所以这个内宽是确定的
        let row_w = cx.turtle().inner_rect().size.x;
        let row_w = if row_w.is_finite() {
            row_w
        } else {
            self.cap_width
        };
        let cap = row_w.clamp(60.0, self.cap_width);
        let inner_cap = (cap - self.pad_x).max(24.0);

        let width = if blocky || body.is_empty() {
            cap
        } else {
            let scale = (self.draw_text.font_scale as f64).max(0.0001);
            let laid = self.draw_text.layout(
                cx,
                0.0,
                0.0,
                Some((inner_cap / scale) as f32),
                true,
                Align::default(),
                &body,
            );
            let used = laid.size_in_lpxs.width as f64 * scale;
            // 余量：这只测量实例与 Markdown 的渲染实例不是同一个，
            // emoji 与字距会有几像素出入，不够余量就会提前折行
            (used + self.pad_x + 8.0).clamp(60.0, cap)
        };

        // 给子部件一个确定宽度：Markdown 的换行宽度就是从这里来的。
        // 子层的 step 自己走完再交回 done —— 这一层没有需要上层驱动的部件
        // （PortalList 之类），照 Widget::draw_all 的循环语义来
        let child = Walk {
            width: Size::Fixed(width),
            height: Size::fit(),
            ..walk
        };
        let mut step = self.view.draw_walk(cx, scope, child);
        while step.is_step() {
            step = self.view.draw_walk(cx, scope, child);
        }
        DrawStep::done()
    }
}

// ---------------------------------------------------------------- 消息列表

/// 包一层 PortalList：列表项的文字必须由 Rust 侧按数据逐项写入。
#[derive(Script, ScriptHook, Widget)]
pub struct ChatList {
    #[deref]
    view: View,
    /// 正在打字的那一项（只在它刚开始时重置动画）。
    #[rust]
    animating: Option<usize>,
}

impl Widget for ChatList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let chat = CHAT.read().unwrap();

        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                let count = chat.messages.len();
                let items_len = count + chat.is_streaming as usize;
                list.set_item_range(cx, 0, items_len);

                while let Some(item_id) = list.next_visible_item(cx) {
                    // 正在流式的那一条：同一个 item 反复重绘，交给 Markdown 的流式动画逐字显形
                    if chat.is_streaming && item_id == count {
                        let just_started = self.animating != Some(item_id);
                        if just_started {
                            self.animating = Some(item_id);
                        }
                        let (item_widget, _) = list.item_with_existed(cx, item_id, id!(Char));
                        let text = if chat.streaming.is_empty() {
                            "..."
                        } else {
                            &chat.streaming
                        };
                        let mut markdown = item_widget.markdown(cx, ids!(selectable));
                        markdown.set_text(cx, text);
                        if just_started {
                            markdown.reset_all_streaming_animations();
                        } else {
                            markdown.start_streaming_animation();
                        }
                        *BUBBLE_BODY.write().unwrap() =
                            Some((text.to_owned(), has_block_content(text)));
                        item_widget.draw_all_unscoped(cx);
                        *BUBBLE_BODY.write().unwrap() = None;
                        continue;
                    }

                    if let Some(msg) = chat.messages.get(item_id) {
                        let was_animating = self.animating == Some(item_id);
                        let template = match msg.role {
                            Role::User => id!(User),
                            Role::Char => id!(Char),
                        };
                        let (item_widget, existed) = list.item_with_existed(cx, item_id, template);
                        let mut markdown = item_widget.markdown(cx, ids!(selectable));
                        markdown.set_text(cx, &msg.text);
                        if was_animating {
                            markdown.stop_streaming_animation();
                        }
                        // 图片只在部件**新建**时解一次。`load_image_from_data` 每次都真解码，
                        // 而这条路径每帧都会跑到，无条件调就是每帧重解码。`existed` 帮我们
                        // 分清「新建」和「复用」；部件被回收后再滚回来会重建，于是自然重解一次。
                        if !existed && msg.role == Role::User {
                            let photo = item_widget.image(cx, ids!(photo));
                            match &msg.image {
                                // 选图那一刻已经解成功过一次，这里再失败就只是不显示
                                Some(attachment) => {
                                    let _ = photo.load_image_from_data(cx, &attachment.bytes);
                                }
                                None => photo.set_texture(cx, None),
                            }
                        }
                        *BUBBLE_BODY.write().unwrap() =
                            Some((msg.text.clone(), wants_full_width(msg)));
                        item_widget.draw_all_unscoped(cx);
                        *BUBBLE_BODY.write().unwrap() = None;
                        if was_animating && markdown.is_streaming_animation_done() {
                            self.animating = None;
                        }
                    }
                }
            }
        }
        DrawStep::done()
    }
}

// ---------------------------------------------------------------- 应用

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    worker: Option<Worker>,
    /// 选好但还没随消息发出去的图（预览在输入框左边那个小图上）。
    #[rust]
    pending_image: Option<ImageAttachment>,
    /// `DECEM_IMAGE` 启动钩子只查一次环境变量。
    #[rust]
    startup_image_checked: bool,
}

/// 文件对话框的 id，用来认出哪个对话框回了话。
const PICK_IMAGE: LiveId = live_id!(pick_image);

impl App {
    /// 弹系统文件对话框挑一张图。结果走平台动作异步回来（见 `handle_file_dialog`）。
    /// `want_bytes(true)` 让平台在 worker 线程把字节读好，省得自己碰文件系统。
    fn pick_image(&mut self, cx: &mut Cx) {
        let extensions: Vec<String> = ["png", "jpg", "jpeg", "gif", "webp", "bmp"]
            .iter()
            .map(|ext| (*ext).to_owned())
            .collect();
        let dialog = FileDialog::new()
            .set_id(PICK_IMAGE)
            .set_title("选一张图".to_owned())
            .add_filter("图片".to_owned(), extensions);
        cx.open_select_file_dialog(dialog);
    }

    /// 文件对话框的结果是平台动作，不走 widget action，所以单独接。
    ///
    /// 用 `want_bytes(false)`（默认）拿路径、自己读字节：平台那条 `want_bytes` 通路
    /// 回给应用的是一个 `pub(crate)` 的包装动作（`FileDialogLoadAction`），
    /// 外部命名不到，全仓库也没有消费它的示例。
    fn handle_file_dialog(&mut self, cx: &mut Cx, actions: &Actions) {
        for action in actions {
            let Some(FileDialogAction::FileSelected { id, paths }) =
                action.downcast_ref::<FileDialogAction>()
            else {
                continue;
            };
            if *id != PICK_IMAGE {
                continue;
            }
            if let Some(path) = paths.first() {
                self.accept_image(cx, path);
            }
            break;
        }
    }

    /// 把一张图读进来放进待发槽位（解码 + 算 data URL + 更新预览）。
    /// 文件对话框与 `DECEM_IMAGE` 启动钩子都走这里。
    fn accept_image(&mut self, cx: &mut Cx, path: &Path) {
        let attachment = match Self::read_image(path) {
            Ok(attachment) => attachment,
            Err(reason) => {
                push_message(format!("⚠️ {reason}"));
                return;
            }
        };
        // 先解码显示，成功才算真的收下了
        let photo = self.ui.image(cx, ids!(pending_photo));
        if let Err(err) = photo.load_image_from_data(cx, &attachment.bytes) {
            push_message(format!("⚠️ 这张图读不出来：{err}"));
            return;
        }
        self.pending_image = Some(attachment);
        cx.redraw_all();
    }

    /// 读文件字节并算好 data URL，顺带做个体积上限。
    /// 失败返回**给用户看的原因**，不要把错误吃掉。
    fn read_image(path: &Path) -> Result<ImageAttachment, String> {
        let bytes = std::fs::read(path).map_err(|err| format!("读不了这个文件：{err}"))?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(format!(
                "图片太大（{:.1} MB），上限 {} MB",
                bytes.len() as f64 / 1_048_576.0,
                MAX_IMAGE_BYTES / 1_048_576
            ));
        }
        let bytes: Arc<[u8]> = Arc::from(bytes);
        Ok(ImageAttachment {
            data_url: to_data_url(&mime_for(path), &bytes),
            bytes,
        })
    }

    /// 一次性启动钩子：`DECEM_IMAGE=<路径>` 时把这张图直接放进待发槽位。
    ///
    /// 存在的理由与 `DECEM_STRESS` 一样：系统文件对话框是**模态**窗口，
    /// 自动化脚本驱动不了它（焦点会被 CU 覆盖层截走），
    /// 有了这个钩子，「解码 → 预览 → 发送 → 多模态请求」这半条链就能单独验。
    fn load_startup_image(&mut self, cx: &mut Cx) {
        if self.startup_image_checked {
            return;
        }
        self.startup_image_checked = true;
        if let Ok(path) = std::env::var("DECEM_IMAGE") {
            self.accept_image(cx, Path::new(&path));
        }
    }

    fn send(&mut self, cx: &mut Cx) {
        let input = self.ui.text_input(cx, ids!(input));
        let text = input.text().trim().to_owned();
        // 只有图片、没有文字也可以发（`decem-core` 会用默认图片提示补上文字）
        if text.is_empty() && self.pending_image.is_none() {
            return;
        }
        input.set_text(cx, "");
        let image = self.pending_image.take();

        // 组请求体与拿列表长度都在同一把锁里做完，避免中途被 UI 线程插队
        let (payload, items_len) = {
            let mut chat = CHAT.write().unwrap();
            chat.messages.push(Msg {
                role: Role::User,
                text,
                image,
            });
            chat.streaming.clear();
            chat.is_streaming = true;

            // system 提示词 + 最近 10 条由 `ChatRequest::new` 负责编排；
            // 图片在 `format_messages` 里按模型是否支持视觉决定拼成多模态数组还是降级成文字
            let history: Vec<Message> = chat
                .messages
                .iter()
                .enumerate()
                .map(|(index, msg)| {
                    let role = match msg.role {
                        Role::User => MessageRole::User,
                        Role::Char => MessageRole::Assistant,
                    };
                    let mut message = Message::text(index.to_string(), role, msg.text.as_str(), 0);
                    message.image_url = msg.image.as_ref().map(|image| image.data_url.clone());
                    message
                })
                .collect();
            let request = ChatRequest::new(MODEL, &history);
            let payload = serde_json::to_vec(&request.to_upstream())
                .map_err(|err| format!("请求体序列化失败：{err}"));
            (payload, chat.messages.len() + 1)
        };

        // 预览图跟着清掉
        self.ui.image(cx, ids!(pending_photo)).set_texture(cx, None);

        match payload {
            Ok(body) => self.worker = Some(Worker::spawn_chat(body)),
            Err(reason) => {
                push_message(format!("⚠️ {reason}"));
                self.worker = None;
            }
        }

        let list = self
            .ui
            .widget(cx, ids!(chat_list))
            .portal_list(cx, ids!(list));
        list.set_tail_range(true);
        list.set_first_id_and_scroll(items_len.saturating_sub(1), 0.0);
        cx.redraw_all();
    }

    /// Esc 中断：把已经流出来的部分定格成一条消息。
    fn cancel(&mut self, cx: &mut Cx) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.finish_stream();
        self.worker = None;
        cx.redraw_all();
    }

    fn finish_stream(&self) {
        let mut chat = CHAT.write().unwrap();
        let text = std::mem::take(&mut chat.streaming);
        if !text.is_empty() {
            chat.messages.push(Msg {
                role: Role::Char,
                text,
                image: None,
            });
        }
        chat.is_streaming = false;
    }

    fn poll_worker(&mut self, cx: &mut Cx) {
        let events = match &self.worker {
            Some(worker) => worker.poll(),
            None => return,
        };
        let had_events = !events.is_empty();
        let mut done = false;
        let mut failed = None;
        for event in events {
            match event {
                WorkerEvent::Delta(delta) => {
                    CHAT.write().unwrap().streaming.push_str(&delta);
                }
                WorkerEvent::Done => done = true,
                WorkerEvent::Failed(reason) => failed = Some(reason),
            }
        }
        if let Some(reason) = failed {
            // 已经流出来的部分照样定格，失败原因单独作为一条消息
            self.finish_stream();
            push_message(format!("⚠️ {reason}"));
            self.worker = None;
        } else if done {
            self.finish_stream();
            self.worker = None;
        }
        if had_events {
            cx.redraw_all();
        }
    }
}

impl MatchEvent for App {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self
            .ui
            .glass_button(cx, ids!(attach_button))
            .clicked(actions)
        {
            self.pick_image(cx);
        }
        if self.ui.glass_button(cx, ids!(send_button)).clicked(actions) {
            self.send(cx);
        }
        if self
            .ui
            .text_input(cx, ids!(input))
            .returned(actions)
            .is_some()
        {
            self.send(cx);
        }
        if self.ui.text_input(cx, ids!(input)).escaped(actions) {
            self.cancel(cx);
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        // 近黑 + 橙的风格（Makepad 自带），与 Decem 的调性接近
        desktop_style::install(vm, StyleSheet::load(DesktopStyle::BlackOrange));
        self::script_mod(vm)
    }

    fn after_new_from_script(_vm: &mut ScriptVm, app: &mut Self) {
        let mut chat = CHAT.write().unwrap();
        chat.messages.push(Msg {
            role: Role::Char,
            text: WELCOME.to_owned(),
            image: None,
        });
        // 压测：DECEM_STRESS=1 时塞几百条，观察长列表滚动
        if std::env::var("DECEM_STRESS").is_ok() {
            for i in 0..300 {
                let role = if i % 2 == 0 { Role::User } else { Role::Char };
                chat.messages.push(Msg {
                    role,
                    text: format!(
                        "压测第 {i} 条：中文、全角标点？！～、emoji 🦊，以及一段用来观察换行的稍长文本。"
                    ),
                    image: None,
                });
            }
        }
        drop(chat);
        let _ = app;
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.load_startup_image(cx);
        self.match_event(cx, event);
        if let Event::Actions(actions) = event {
            self.handle_file_dialog(cx, actions);
        }
        self.ui.handle_event(cx, event, &mut Scope::empty());
        self.poll_worker(cx);
    }
}

script_mod! {
    use mod.prelude.widgets.*
    // 代码块用 makepad-widgets 自带的 CodeBlock：它在 Fit 宽度下会量最长行。
    // 别用 CodeView —— 那属于独立的 makepad-code-editor crate，不引它模板会静默失效。
    use mod.widgets.CodeBlock

    // 气泡定宽容器：自己量正文宽度，短消息贴文字、长消息到上限换行。
    // 见 docs/makepad-migration.md §6.2
    let Bubble = #(Bubble::register_widget(vm)) {
        width: Fit
        height: Fit
        // 量宽用的字体必须与 Markdown 正文一致（markdown.rs:101-102）
        draw_text +: { text_style: theme.font_regular{font_size: theme.font_size_p} }
        cap_width: 560.0
        pad_x: 28.0
    }

    let ChatList = #(ChatList::register_widget(vm)) {
        width: Fill
        height: Fill

        list := PortalList {
            width: Fill
            height: Fill
            flow: Down
            auto_tail: true
            smooth_tail: true
            reuse_items: false

            // Decem 一侧：左对齐、深色气泡
            Char := View {
                width: Fill
                height: Fit
                align: Align{x: 0.0}
                padding: Inset{top: 6 bottom: 6 left: 4 right: 64}

                bubble := Bubble {
                    RoundedView {
                        width: Fill
                        height: Fit
                        show_bg: true
                        draw_bg.color: #x24242c
                        draw_bg.border_radius: uniform(16.)
                        padding: Inset{left: 14 top: 10 right: 14 bottom: 10}

                        selectable := Markdown {
                            // 这里必须是 Fill：Bubble 已经把确定宽度给了外层，
                            // Fill 才能拿到它并按它换行
                            width: Fill
                            height: Fit
                            body: ""
                            use_code_block_widget: true
                            use_math_widget: true
                            code_block := View {
                                width: Fill
                                height: Fit
                                code_view := CodeBlock {
                                    width: Fill
                                    height: Fit
                                    show_line_numbers: false
                                }
                            }
                        }
                    }
                }
            }

            // 用户一侧：右对齐、橙色气泡（与网页版同一个橙 #E95620）
            User := View {
                width: Fill
                height: Fit
                align: Align{x: 1.0}
                padding: Inset{top: 6 bottom: 6 left: 64 right: 4}

                bubble := Bubble {
                    RoundedView {
                        width: Fill
                        height: Fit
                        flow: Down
                        show_bg: true
                        draw_bg.color: #xE95620
                        draw_bg.border_radius: uniform(16.)
                        padding: Inset{left: 14 top: 10 right: 14 bottom: 10}

                        // 用户发的图。居中显示，没有纹理时高度收成 0、不占位
                        photo := Image {
                            width: Fit
                            height: Fit
                            max_height: FitBound.Abs(320)
                            fit: ImageFit.Smallest
                            align: Align{x: 0.5}
                        }

                        selectable := Markdown {
                            width: Fill
                            height: Fit
                            body: ""
                        }
                    }
                }
            }
        }
    }

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Decem Chat"
                window.inner_size: vec2(980, 760)
                body +: {
                    View{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 10.
                        padding: Inset{left: 14 top: 10 right: 14 bottom: 12}
                        show_bg: true
                        draw_bg.color: theme.color_bg_app

                        chat_list := ChatList {}

                        // 待发图片的预览。没有图片时没有纹理，高度收成 0、不占位
                        pending_photo := Image {
                            width: Fit
                            height: Fit
                            max_height: FitBound.Abs(120)
                            fit: ImageFit.Smallest
                        }

                        View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 8.
                            align: Align{y: 1.0}

                            attach_button := glass.GlassButtonProminent {
                                text: "＋图"
                                width: 72
                                height: 44
                            }

                            input := glass.TextInput {
                                width: Fill
                                height: 44
                                empty_text: "发个消息聊聊呗～（Enter 发送，Esc 中断）"
                            }

                            send_button := glass.GlassButtonProminent {
                                text: "发送"
                                width: 84
                                height: 44
                            }
                        }
                    }
                }
            }
        }
    }
}
