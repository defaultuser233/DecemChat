//! 应用状态与主循环。
//!
//! 关键设计：
//!
//! - 网络请求全在 [`crate::worker`] 的后台线程里，界面线程只处理事件与绘制；
//! - 界面产生的交互统一变成 [`Action`]，在每帧绘制结束后执行，避免绘制中途改状态；
//! - 回复先整段收完，再按 8ms/字 逐字显示，与网页版的打字机效果一致。

use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use decem_core::{
    ChatRequest, ChatSettings, DEFAULT_MODEL, Message, MessageRole, api_key_from_env,
    default_settings, find_model, random_char_avatar, random_user_avatar,
};

use crate::assets;
use crate::media;
use crate::store::{Store, StoredConfig};
use crate::theme::{self, Palette};
use crate::ui::{self, Action};
use crate::worker::{Worker, WorkerEvent};

/// 解析接口地址：配置 → 环境变量 `DECEMCHAT_API_URL` → 默认代理地址。
///
/// 界面与 `--selftest --live` 共用这一处逻辑，避免两边行为不一致。
pub fn resolve_api_url(saved: Option<&StoredConfig>) -> String {
    saved
        .and_then(|config| config.api_url.clone())
        .or_else(|| {
            std::env::var(API_URL_ENV)
                .ok()
                .filter(|url| !url.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_API_URL.to_owned())
}

/// 默认接口地址：网页版自己的服务端（Key 留在服务器的 `.env` 里，客户端不需要填）。
///
/// - 想直连百炼：把配置里的 `apiUrl` 设为空字符串，并填写 API Key
/// - 想换服务器：改 `apiUrl`，或用环境变量 `DECEMCHAT_API_URL` 覆盖
pub const DEFAULT_API_URL: &str = "https://api.valedecem.top:8443/api/chat";

/// 代码高亮主题：内容取自网页版用的 highlight.js `tokyo-night-dark`
/// （见 `assets/decem-code.tmTheme`）。
pub const CODE_THEME_NAME: &str = "decem-code";
/// 主题文件内容。
const CODE_THEME: &[u8] = include_bytes!("../assets/decem-code.tmTheme");

/// 气泡宽度缓存的键：内容哈希 + DPI 百分数 + 排版环境指纹。
///
/// 排版环境（字体，尤其是还没生效的字体）变了，量出来的宽度就不可信，
/// 指纹一并用进键里，免得把错的宽度一直用下去。
pub type BubbleWidthKey = (u64, u32, u32);

/// 把 egui 判定「点击」的阈值放宽到跟浏览器一致。
///
/// egui 默认把「按住超过 0.8 秒」或「按下期间手抖超过 6px」当成拖动 —— 这一下就不算点击
/// （`egui-0.36.2/src/input_state/mod.rs:1122` 的 `has_moved_too_much_for_a_click`，
/// 以及同一处的 `max_click_duration`）。用户报的「有时选不中」就是这么来的：
/// 菜单项按住想一下再松开，什么都没发生。
/// 浏览器是「同一控件上按下再松开就算点击」，这里把两个阈值放宽到人手够用。
pub fn relax_click_thresholds(ctx: &egui::Context) {
    ctx.memory_mut(|memory| {
        memory.options.input_options.max_click_dist = 12.0;
        memory.options.input_options.max_click_duration = 3.0;
    });
}

/// 建一个注册好内置主题的 Markdown 缓存。
///
/// 正文渲染和「量气泡宽度」各用一份，互不干扰。
fn markdown_cache() -> egui_commonmark::CommonMarkCache {
    let mut cache = egui_commonmark::CommonMarkCache::default();
    if let Err(err) = cache.add_syntax_theme_from_bytes(CODE_THEME_NAME, CODE_THEME) {
        log::warn!("代码高亮主题加载失败，回退到内置主题：{err}");
    }
    cache
}

/// 覆盖接口地址的环境变量。
pub const API_URL_ENV: &str = "DECEMCHAT_API_URL";

/// 打字机速度：每个字 8 毫秒（网页版发消息时的 `typingSpeed`）。
const TYPING_SPEED: Duration = Duration::from_millis(8);
/// 重新生成时的打字机速度（网页版那里用的是 12 毫秒）。
const REGENERATE_TYPING_SPEED: Duration = Duration::from_millis(12);
/// 提示气泡存活时长。
const TOAST_TTL: Duration = Duration::from_secs(6);
/// 欢迎语。
const WELCOME_ID: &str = "welcome";
/// 欢迎语内容。
const WELCOME_CONTENT: &str = "🦊狐狐来啦～(≧▽≦)/～💗你终于来找我玩啦！嗷呜～";

/// 待发送的图片。
pub struct PendingImage {
    /// 用于纹理缓存的键。
    pub id: String,
    /// 原始文件名（仅用于提示）。
    pub display_name: String,
    /// 实际发送给模型的 data URL（GIF 为抽帧拼接图）。
    pub data_url: String,
    /// GIF 抽帧数；`None` 表示不是 GIF。
    pub gif_frames: Option<usize>,
}

/// 正在等待回复的Decem 消息。
struct Streaming {
    /// 请求序号，用来丢弃过期事件。
    request_id: u64,
    /// 占位消息 ID。
    message_id: String,
    /// 这一段回复的打字速度。
    speed: Duration,
}

/// 正在逐字显示的回复。
struct Typing {
    /// 目标消息 ID。
    message_id: String,
    /// 完整回复。
    full: String,
    /// 开始时间，用它换算已显示字数，避免累积误差。
    started: Instant,
    /// 打字速度：发消息 8ms/字，重新生成 12ms/字（与网页版一致）。
    speed: Duration,
}

/// 提示气泡类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// 普通提示。
    Info,
    /// 出错提示。
    Error,
}

/// 顶部提示气泡。
pub struct Toast {
    /// 文本。
    pub text: String,
    /// 类型。
    pub kind: ToastKind,
    /// 创建时间。
    pub created: Instant,
}

/// 桌面端应用。
pub struct DecemApp {
    store: Store,
    /// 界面设置。
    pub settings: ChatSettings,
    /// API Key（设置面板可直接编辑）。
    pub api_key: String,
    /// 自定义接口地址，空字符串表示用官方地址。
    pub api_url: String,
    /// 配置文件里**原本记着**的接口地址，保存时照原样写回。
    ///
    /// 记的是从 `config.json` 里读到的那一份，不是 [`resolve_api_url`] 解析出来的结果。
    /// 否则内置默认地址会被固化进用户的配置文件，以后换默认地址，老用户的旧值永远压着新值。
    api_url_from_config: Option<String>,
    /// 聊天消息。
    pub messages: Vec<Message>,
    /// 输入框内容。
    pub input: String,
    /// 待发送图片。
    pub pending_image: Option<PendingImage>,
    /// 图片纹理缓存。
    pub textures: ui::textures::TextureCache,
    /// Markdown 渲染缓存。
    pub markdown: egui_commonmark::CommonMarkCache,
    /// 量气泡宽度专用的 Markdown 缓存（与正文渲染分开，免得互相干扰）。
    pub measure: egui_commonmark::CommonMarkCache,
    /// 设置抽屉里当前悬停的模型 ID（悬停高亮要提前知道，只能记上一帧的结果）。
    pub hovered_model: Option<String>,
    /// 各段内容的自然宽度缓存，见 [`BubbleWidthKey`]。
    pub bubble_widths: std::collections::HashMap<BubbleWidthKey, f32>,
    /// 顶部提示。
    pub toasts: Vec<Toast>,
    /// 设置面板是否展开。
    pub show_settings: bool,
    /// 「确认清空」弹窗是否展开。
    pub confirm_clear: bool,
    /// 首次配置 API Key 的弹窗是否展开。
    pub show_key_prompt: bool,
    /// 每条消息首次被绘制的时间（秒），用于入场动画。
    pub seen_at: std::collections::HashMap<String, f64>,
    /// 中文字体路径，`None` 表示没找到。
    pub font_path: Option<String>,

    worker: Worker,
    streaming: Option<Streaming>,
    typing: Option<Typing>,
    next_request_id: u64,
    /// 本次请求完成后逐字显示的速度。
    pending_typing_speed: Duration,
    applied_dark: Option<bool>,
    messages_dirty: bool,
    /// 是否需要把消息列表滚到底部（发送、收到增量、打字时都会置位）。
    pub stick_to_bottom: bool,
}

impl DecemApp {
    /// 搭建界面、加载本地数据并启动后台线程。
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self> {
        let fonts = crate::fonts::install(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let (store, storage_warning) = match Store::open() {
            Ok(store) => (store, None),
            Err(err) => {
                // 配置目录不可用时仍然让用户能用：退到临时目录
                let fallback = std::env::temp_dir().join("decemchat");
                let store = Store::at(&fallback)
                    .with_context(|| format!("无法创建任何配置目录（{err}）"))?;
                (
                    store,
                    Some(format!(
                        "配置目录不可用，本次数据暂存在 {}：{err}",
                        fallback.display()
                    )),
                )
            }
        };

        Self::build(store, &cc.egui_ctx, fonts, storage_warning)
    }

    /// 从 `store` 读配置与历史并装配状态。
    ///
    /// 与 [`Self::new`] 分开，是为了让测试能注入临时目录，不必依赖真实配置目录。
    pub(crate) fn build(
        store: Store,
        ctx: &egui::Context,
        fonts: crate::fonts::LoadedFonts,
        storage_warning: Option<String>,
    ) -> Result<Self> {
        // 点击判定放宽要放在 build 里：new 和测试都经由它
        relax_click_thresholds(ctx);

        let saved = store.load_config();
        let settings = normalize_settings(
            saved
                .as_ref()
                .map(|config| config.settings.clone())
                .unwrap_or_else(default_settings),
        );
        let api_key = saved
            .as_ref()
            .and_then(|config| config.api_key.clone())
            .or_else(api_key_from_env)
            .unwrap_or_default();
        let api_url = resolve_api_url(saved.as_ref());
        // 配置文件里原本写的是什么地址，就记下什么，落盘时原样写回
        let api_url_from_config = saved.as_ref().and_then(|config| config.api_url.clone());

        let mut messages = store.load_messages();
        if messages.is_empty() {
            messages.push(Message::text(
                WELCOME_ID,
                MessageRole::Assistant,
                WELCOME_CONTENT,
                now_ms(),
            ));
        }
        restore_images(&store, &mut messages);

        let worker = Worker::spawn(ctx.clone())?;
        // 默认走网页版自己的服务端（Key 在服务器上），只有显式改成直连又没填 Key 时才需要弹窗
        let show_key_prompt = api_url.trim().is_empty() && api_key.trim().is_empty();

        let mut app = Self {
            store,
            settings,
            api_key,
            api_url,
            api_url_from_config,
            messages,
            input: String::new(),
            pending_image: None,
            textures: ui::textures::TextureCache::new(),
            markdown: markdown_cache(),
            measure: markdown_cache(),
            hovered_model: None,
            bubble_widths: std::collections::HashMap::new(),
            toasts: Vec::new(),
            show_settings: false,
            confirm_clear: false,
            show_key_prompt,
            seen_at: std::collections::HashMap::new(),
            font_path: fonts.cjk,
            worker,
            streaming: None,
            typing: None,
            next_request_id: 1,
            pending_typing_speed: TYPING_SPEED,
            applied_dark: None,
            messages_dirty: false,
            stick_to_bottom: true,
        };

        if app.font_path.is_none() {
            app.toast_info(format!(
                "没有找到中文字体，界面可能显示为方块。可设置 {}=字体文件路径",
                crate::fonts::FONT_ENV
            ));
        }
        if let Some(warning) = storage_warning {
            app.toast_error(warning);
        }
        app.messages_dirty = true;
        Ok(app)
    }

    /// 是否必须先填 API Key：只有「直连服务商」（没配代理地址）时才需要。
    fn needs_api_key(&self) -> bool {
        self.api_url.trim().is_empty() && self.api_key.trim().is_empty()
    }

    /// 当前配色。
    pub fn palette(&self) -> Palette {
        theme::palette(self.settings.is_dark_mode)
    }

    /// 是否有请求在途或正在打字。
    pub fn is_busy(&self) -> bool {
        self.streaming.is_some() || self.typing.is_some()
    }

    /// 正在等待回复的Decem 消息 ID（还没收到任何内容）。
    pub fn streaming_message_id(&self) -> Option<&str> {
        self.streaming
            .as_ref()
            .map(|streaming| streaming.message_id.as_str())
    }

    /// 正在逐字显示的Decem 消息 ID。
    pub fn typing_message_id(&self) -> Option<&str> {
        self.typing
            .as_ref()
            .map(|typing| typing.message_id.as_str())
    }

    /// 提示：普通信息。
    pub fn toast_info(&mut self, text: impl Into<String>) {
        self.push_toast(text.into(), ToastKind::Info);
    }

    /// 提示：错误。
    pub fn toast_error(&mut self, text: impl Into<String>) {
        self.push_toast(text.into(), ToastKind::Error);
    }

    fn push_toast(&mut self, text: String, kind: ToastKind) {
        log::warn!("提示（{kind:?}）：{text}");
        self.toasts.push(Toast {
            text,
            kind,
            created: Instant::now(),
        });
        // 最多同时显示 3 条
        while self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }

    /// 发送当前输入（含可选图片）。
    fn send_message(&mut self) {
        if self.is_busy() {
            return;
        }
        let content = self.input.trim().to_owned();
        let image = self.pending_image.take();
        if content.is_empty() && image.is_none() {
            return;
        }

        let timestamp = now_ms();
        let user_id = timestamp.to_string();
        let mut user_message =
            Message::text(user_id.clone(), MessageRole::User, content, timestamp);
        if let Some(image) = image {
            // 先落盘再整体移进消息，省掉一份 base64 复制
            if let Err(err) = self.store.save_image(&user_id, &image.data_url) {
                self.toast_error(format!("图片保存失败（仅影响重启后恢复）：{err:#}"));
            }
            user_message.image_url = Some(image.data_url);
            user_message.image_path = Some("stored".to_owned());
            user_message.hint = image
                .gif_frames
                .map(|frames| format!("[此图为GIF动图抽取的{frames}帧拼接，按时间先后顺序排列]"));
        }

        self.messages.push(user_message);
        self.input.clear();
        self.stick_to_bottom = true;
        self.messages_dirty = true;

        let char_id = (timestamp + 1).to_string();
        if self.needs_api_key() {
            self.show_key_prompt = true;
            return;
        }

        // 直接借用现成列表拼请求（内部只取最近若干条），不必克隆整段历史：
        // 此时 `self.messages` 正好是「历史 + 本轮用户消息」，还没加Decem 占位
        let request = ChatRequest::new(&self.settings.model, &self.messages);
        self.messages.push(Message::text(
            char_id.clone(),
            MessageRole::Assistant,
            "",
            now_ms(),
        ));
        self.start_request(request, char_id);
    }

    /// 重新生成最后一条回复。
    fn regenerate(&mut self) {
        if self.is_busy() {
            return;
        }
        let Some(index) = self
            .messages
            .iter()
            .rposition(|message| message.role == MessageRole::User)
        else {
            return;
        };
        // 丢掉该用户消息之后的Decem 回复
        self.messages.truncate(index + 1);

        let char_id = format!("{}r", now_ms());
        if self.needs_api_key() {
            self.show_key_prompt = true;
            return;
        }

        // 借用现成列表拼请求：此时列表正好以该用户消息结尾
        let request = ChatRequest::new(&self.settings.model, &self.messages);
        self.messages.push(Message::text(
            char_id.clone(),
            MessageRole::Assistant,
            "",
            now_ms(),
        ));
        self.stick_to_bottom = true;
        self.messages_dirty = true;
        self.set_typing_speed(REGENERATE_TYPING_SPEED);
        self.start_request(request, char_id);
    }

    /// 设置本次请求的打字速度（发消息 8ms/字，重新生成 12ms/字）。
    fn set_typing_speed(&mut self, speed: Duration) {
        self.pending_typing_speed = speed;
    }

    fn start_request(&mut self, request: ChatRequest, char_id: String) {
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        self.streaming = Some(Streaming {
            request_id,
            message_id: char_id,
            speed: self.pending_typing_speed,
        });

        let api_url = self.api_url.trim();
        if let Err(err) = self.worker.start_chat(
            request_id,
            request,
            self.api_key.trim().to_owned(),
            (!api_url.is_empty()).then(|| api_url.to_owned()),
        ) {
            self.streaming = None;
            self.toast_error(format!("无法发起请求：{err:#}"));
        }
    }

    /// 清空聊天记录并删除本地数据。
    fn clear_chat(&mut self) {
        self.streaming = None;
        self.typing = None;
        self.messages.clear();
        self.messages.push(Message::text(
            WELCOME_ID,
            MessageRole::Assistant,
            WELCOME_CONTENT,
            now_ms(),
        ));
        self.stick_to_bottom = true;
        self.messages_dirty = true;
        if let Err(err) = self.store.clear_images() {
            self.toast_error(format!("图片清理失败：{err:#}"));
        }
        self.flush_messages();
    }

    /// 选择一张图片作为待发送附件。
    fn pick_image(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("选择要发送的图片")
            .add_filter("图片", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        match load_attachment(&path) {
            Ok(image) => {
                if image.gif_frames.is_some() {
                    self.toast_info("GIF 已抽帧拼接，发送时将以网格图让模型识别");
                }
                self.pending_image = Some(image);
            }
            Err(err) => self.toast_error(format!("图片读取失败：{err:#}")),
        }
    }

    /// 选择一张本地图片作为用户头像（统一压成 256px JPEG）。
    fn pick_avatar(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("选择头像图片")
            .add_filter("图片", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        match std::fs::read(&path).map_err(anyhow::Error::from) {
            Ok(bytes) => match media::avatar_data_url(&bytes) {
                Some(data_url) => {
                    self.settings.user_avatar = data_url;
                    self.persist_config();
                }
                None => self.toast_error("这个文件无法识别为图片"),
            },
            Err(err) => self.toast_error(format!("头像读取失败：{err:#}")),
        }
    }

    /// 保存配置到磁盘。
    fn persist_config(&mut self) {
        let api_key = self.api_key.trim();
        let config = StoredConfig {
            settings: self.settings.clone(),
            api_key: (!api_key.is_empty()).then(|| api_key.to_owned()),
            // 原样写回配置文件里原本的值：环境变量与内置默认地址都不固化，
            // 这样以后换默认地址，老用户不改文件也能跟上
            api_url: self.api_url_from_config.clone(),
        };
        if let Err(err) = self.store.save_config(&config) {
            self.toast_error(format!("配置保存失败：{err:#}"));
        }
    }

    pub(crate) fn flush_messages(&mut self) {
        if !self.messages_dirty {
            return;
        }
        match self.store.save_messages(&self.messages) {
            Ok(()) => self.messages_dirty = false,
            Err(err) => {
                self.messages_dirty = false;
                self.toast_error(format!("聊天记录保存失败：{err:#}"));
            }
        }
    }

    /// 消化后台事件。
    pub(crate) fn pump_worker(&mut self) {
        for event in self.worker.drain() {
            match event {
                WorkerEvent::Done {
                    request_id,
                    content,
                } => {
                    let Some(streaming) = self.take_streaming(request_id) else {
                        continue;
                    };
                    if content.trim().is_empty() {
                        self.messages
                            .retain(|message| message.id != streaming.message_id);
                        self.toast_error("服务器未返回有效内容");
                        continue;
                    }
                    self.typing = Some(Typing {
                        message_id: streaming.message_id,
                        full: content,
                        started: Instant::now(),
                        speed: streaming.speed,
                    });
                }
                WorkerEvent::Failed {
                    request_id,
                    message,
                } => {
                    // request_id == 0 表示后台线程自身的故障，与具体请求无关
                    if request_id == 0 {
                        self.toast_error(message);
                        continue;
                    }
                    let Some(streaming) = self.take_streaming(request_id) else {
                        continue;
                    };
                    self.messages.retain(|item| item.id != streaming.message_id);
                    self.toast_error(message);
                }
            }
        }
    }

    /// 取出与 `request_id` 匹配的进行中请求。
    ///
    /// 事件可能是上一次请求的遗留（后台线程被新请求顶替时的竞态），
    /// 不匹配时原样保留当前状态并返回 `None`。
    fn take_streaming(&mut self, request_id: u64) -> Option<Streaming> {
        match self.streaming.as_ref() {
            Some(streaming) if streaming.request_id == request_id => self.streaming.take(),
            _ => None,
        }
    }

    /// 推进打字机动画。
    pub(crate) fn pump_typing(&mut self, ctx: &egui::Context) {
        let Some(typing) = self.typing.take() else {
            return;
        };

        let total = typing.full.chars().count();
        let elapsed = typing.started.elapsed().as_millis();
        let shown = ((elapsed / typing.speed.as_millis().max(1)) as usize).min(total);
        let partial: String = typing.full.chars().take(shown).collect();
        set_message_content(&mut self.messages, &typing.message_id, partial);
        self.stick_to_bottom = true;

        if shown < total {
            let speed = typing.speed;
            self.typing = Some(typing);
            ctx.request_repaint_after(speed);
        } else {
            self.messages_dirty = true;
        }
    }

    fn pump_toasts(&mut self, ctx: &egui::Context) {
        self.toasts
            .retain(|toast| toast.created.elapsed() < TOAST_TTL);
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    pub(crate) fn apply(&mut self, action: Action) {
        match action {
            Action::Send => self.send_message(),
            Action::Regenerate => self.regenerate(),
            Action::ClearChat => {
                self.confirm_clear = false;
                self.clear_chat();
            }
            Action::DropPendingImage => self.pending_image = None,
            Action::PickImage => self.pick_image(),
            Action::PickAvatar => self.pick_avatar(),
            Action::SetModel(model) => {
                self.settings.model = model;
                self.persist_config();
            }
            Action::SetDarkMode(dark) => {
                self.settings.is_dark_mode = dark;
                self.persist_config();
            }
            Action::ToggleSettings => self.show_settings = !self.show_settings,
            Action::ShowClearConfirm(show) => self.confirm_clear = show,
            Action::SaveApiKey => {
                self.persist_config();
                self.show_key_prompt = false;
            }
            Action::CloseKeyPrompt => self.show_key_prompt = false,
        }
    }
}

impl DecemApp {
    /// 绘制一整帧界面，返回这一帧产生的交互动作。
    ///
    /// 刻意与 [`eframe::App::ui`] 分开：测试可以在无头 `egui::Context` 上直接调用它，
    /// 不需要真实窗口，也能覆盖「控件 → Action」这一段接线。
    pub(crate) fn draw(&mut self, ui: &mut egui::Ui) -> Vec<Action> {
        let ctx = ui.ctx().clone();
        let palette = self.palette();
        if self.applied_dark != Some(palette.dark) {
            theme::apply(&ctx, palette);
            self.applied_dark = Some(palette.dark);
        }

        // 入场动画只需要最近出现的消息，顺手清掉已经不存在的记录
        self.seen_at
            .retain(|id, _| self.messages.iter().any(|message| &message.id == id));

        let mut actions = Vec::new();
        ui::header::show(ui, self, palette, &mut actions);
        if self.show_settings {
            ui::settings::show(ui, self, palette, &mut actions);
        }
        ui::composer::show(ui, self, palette, &mut actions);
        ui::messages::show(ui, self, palette, &mut actions);
        ui::toast::show(&ctx, &self.toasts, palette);
        if self.confirm_clear {
            ui::settings::show_clear_confirm(&ctx, palette, &mut actions);
        }
        if self.show_key_prompt {
            ui::settings::show_key_prompt(&ctx, palette, &mut self.api_key, &mut actions);
        }
        actions
    }
}

impl eframe::App for DecemApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // 等待回复时没有别的输入会触发重绘，这里主动刷新让转圈动画动起来
        if self.streaming.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
        self.pump_worker();
        self.pump_typing(&ctx);
        self.pump_toasts(&ctx);

        for action in self.draw(ui) {
            self.apply(action);
        }

        self.stick_to_bottom = false;
        if !self.is_busy() {
            self.flush_messages();
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        // 每帧都设一遍：eframe 会持久化 egui 的 Memory，光在建应用时设一次会被恢复出来的旧值盖掉
        relax_click_thresholds(ctx);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.flush_messages();
    }
}

/// 读取本地图片，必要时做 GIF 抽帧。
fn load_attachment(path: &Path) -> Result<PendingImage> {
    let bytes = std::fs::read(path).with_context(|| format!("读取 {} 失败", path.display()))?;
    if bytes.len() > media::MAX_UPLOAD_BYTES {
        anyhow::bail!("图片大小不能超过 5MB");
    }

    let display_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let id = format!("attach-{}", now_ms());

    // GIF 先抽帧拼成网格图；抽帧失败则回退为原图（模型只会看到首帧）
    if media::mime_from_path(path) == "image/gif"
        && let Some(grid) = media::extract_gif_grid(&bytes, media::GIF_MAX_FRAMES)
    {
        return Ok(PendingImage {
            id,
            display_name,
            data_url: grid.data_url,
            gif_frames: Some(grid.frame_count),
        });
    }

    Ok(PendingImage {
        id,
        display_name,
        data_url: media::to_data_url(media::mime_from_path(path), &bytes),
        gif_frames: None,
    })
}

/// 恢复历史消息里的图片（图片单独存在磁盘上，不进 JSON）。
fn restore_images(store: &Store, messages: &mut [Message]) {
    for message in messages.iter_mut() {
        if message.image_path.is_some() && message.image_url.is_none() {
            message.image_url = store.load_image(&message.id);
        }
    }
}

/// 与网页版一致的启动归一化：
///
/// - 内置头像每次启动重新随机；
/// - 用户上传的头像（data URL）保留；
/// - 已下架的模型 ID 回退到默认模型。
fn normalize_settings(mut settings: ChatSettings) -> ChatSettings {
    if !assets::is_builtin_avatar(&settings.char_avatar) {
        settings.char_avatar = random_char_avatar();
    }
    if !settings.user_avatar.starts_with("data:") {
        settings.user_avatar = random_user_avatar();
    }
    if find_model(&settings.model).is_none() {
        settings.model = DEFAULT_MODEL.to_owned();
    }
    settings
}

fn set_message_content(messages: &mut [Message], id: &str, content: String) {
    if let Some(message) = messages.iter_mut().find(|message| message.id == id) {
        message.content = content;
    }
}

/// 毫秒时间戳。
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use decem_core::default_settings;

    #[test]
    fn settings_are_normalized_on_startup() {
        let mut settings = default_settings();
        settings.model = "已下架的模型".to_owned();
        settings.user_avatar = "data:image/jpeg;base64,AAAA".to_owned();
        settings.char_avatar = "随便一个路径".to_owned();

        let normalized = normalize_settings(settings);
        assert_eq!(normalized.model, DEFAULT_MODEL);
        // 上传的头像要保留
        assert_eq!(normalized.user_avatar, "data:image/jpeg;base64,AAAA");
        assert!(assets::is_builtin_avatar(&normalized.char_avatar));
    }

    #[test]
    fn builtin_avatars_are_embedded() {
        assert!(assets::is_builtin_avatar("./images/char_01.jpg"));
        assert!(assets::is_builtin_avatar("./images/user_02.jpg"));
        assert!(!assets::is_builtin_avatar("https://example.com/a.jpg"));
    }

    #[test]
    fn typing_speed_matches_the_web_version() {
        // 网页版：发消息 8ms/字，重新生成 12ms/字
        assert_eq!(TYPING_SPEED.as_millis(), 8);
        assert_eq!(REGENERATE_TYPING_SPEED.as_millis(), 12);
    }

    #[test]
    fn message_content_updates_by_id() {
        let mut messages = vec![Message::text("a", MessageRole::Assistant, "", 0)];
        set_message_content(&mut messages, "a", "嗷呜～".to_owned());
        assert_eq!(messages[0].content, "嗷呜～");
        // 找不到就什么都不做
        set_message_content(&mut messages, "b", "x".to_owned());
        assert_eq!(messages[0].content, "嗷呜～");
    }
}
