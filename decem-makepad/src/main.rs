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
    is_vision_model, model_display_name, parse_sse_line, ChatRequest, Message, MessageRole,
    SseDecoder, SseOutcome, AVAILABLE_MODELS, DEFAULT_MODEL,
};
use makepad_base64::{base64_encode, BASE64_STANDARD};

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, OnceLock, RwLock};
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
    /// 本地时间 `HH:MM`，在消息创建时定格（网页版同样是消息生成那一刻的时间）。
    time: String,
}

/// 当前本地时间，24 小时制 `HH:MM`。
///
/// Windows 上直接调 kernel32 的 `GetLocalTime`（和 Makepad 自己的
/// `tools/makepad_builder/src/tui.rs:2651-2655` 同一个写法）：
/// `makepad_platform` 虽然 re-export 了 `windows` crate（`platform/src/lib.rs:169-170`），
/// 但它没有开 `Win32_System_Time` feature，拿不到 `GetLocalTime` 绑定；
/// 自己声明 extern 不引入任何新依赖。非 Windows 退回 UTC（本项目只发 Windows 包）。
fn now_hhmm() -> String {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            // SYSTEMTIME: year, month, day_of_week, day, hour, minute, second, ms
            fn GetLocalTime(time: *mut [u16; 8]);
        }
        let mut time = [0u16; 8];
        // SAFETY: GetLocalTime 会在指针处写满 8 个 u16，和 SYSTEMTIME 的内存布局一致
        unsafe { GetLocalTime(&mut time) };
        format!("{:02}:{:02}", time[4], time[5])
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0);
        format!("{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60)
    }
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
        time: now_hhmm(),
    });
}

/// 默认头像**嵌进二进制**。
///
/// 之前按进程 cwd 找 `public/images/…`，结果双击 exe（cwd = `target\release\`）时
/// 两张图都找不到，界面上只剩两个空环；Android 上更是不可能存在这种路径。
/// 这两张默认图很小（合计约 180KB），Embed 进去后任何平台、任何启动方式都有图。
const DEFAULT_CHAR_AVATAR: &[u8] = include_bytes!("../../public/images/char_01.jpg");
const DEFAULT_USER_AVATAR: &[u8] = include_bytes!("../../public/images/user_01.jpg");

fn default_char_avatar() -> Arc<[u8]> {
    Arc::from(DEFAULT_CHAR_AVATAR)
}

fn default_user_avatar() -> Arc<[u8]> {
    Arc::from(DEFAULT_USER_AVATAR)
}

/// 用户头像。用户可以自己换，所以不能塞进 `OnceLock`：上传后要能整体替换。
struct UserAvatar {
    /// 永远有值：默认图已经嵌进二进制，用户上传的图替换它。
    bytes: Option<Arc<[u8]>>,
    /// 是否来自用户上传（持久化文件或本次会话选的）。顶栏据此决定画谁。
    custom: bool,
}

/// 启动时由 `load_initial_user_avatar` 决定（持久化文件优先于内嵌默认图），
/// 上传时整体替换。预览、顶栏、后续消息行都从这里取。
static USER_AVATAR: RwLock<Option<UserAvatar>> = RwLock::new(None);

static CHAR_AVATAR: OnceLock<Arc<[u8]>> = OnceLock::new();

fn avatar_bytes(role: Role) -> Option<Arc<[u8]>> {
    match role {
        Role::User => {
            let state = USER_AVATAR.read().unwrap();
            match state.as_ref() {
                Some(state) => state.bytes.clone(),
                // 正常路径下 `after_new_from_script` 已经初始化；这里只是兜底
                None => Some(default_user_avatar()),
            }
        }
        Role::Char => Some(CHAR_AVATAR.get_or_init(default_char_avatar).clone()),
    }
}

/// 顶栏头像：网页版顶栏画的是 Decem（char）；用户换过头像后跟随用户头像，
/// 否则默认仍是 char_01（规格 §2）。
fn header_avatar_bytes() -> Option<Arc<[u8]>> {
    let custom = {
        let state = USER_AVATAR.read().unwrap();
        state
            .as_ref()
            .filter(|state| state.custom)
            .and_then(|state| state.bytes.clone())
    };
    custom.or_else(|| avatar_bytes(Role::Char))
}

// ---------------------------------------------------------------- 头像持久化

/// 持久化头像支持的扩展名：写入时用原文件扩展名，启动时按这个顺序找。
const AVATAR_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/// `%APPDATA%\DecemChat`。读不到 `APPDATA` 时返回 `None`（保存会给出提示）。
fn avatar_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|base| Path::new(&base).join("DecemChat"))
}

/// 已保存的头像文件（`avatar.<ext>`）。按扩展名顺序取第一个存在的。
fn stored_avatar_path() -> Option<PathBuf> {
    let dir = avatar_dir()?;
    AVATAR_EXTENSIONS
        .iter()
        .map(|extension| dir.join(format!("avatar.{extension}")))
        .find(|path| path.is_file())
}

/// 把头像字节写成 `%APPDATA%\DecemChat\avatar.<ext>`，返回落盘路径。
/// 换过格式时删掉旧的 `avatar.*`，否则启动时可能按扩展名顺序读回旧图。
fn save_avatar(bytes: &[u8], source: &Path) -> Result<PathBuf, String> {
    let dir = avatar_dir().ok_or_else(|| "读不到 APPDATA 环境变量，无法保存头像".to_owned())?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("创建头像目录 {} 失败：{err}", dir.display()))?;

    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .filter(|extension| AVATAR_EXTENSIONS.contains(&extension.as_str()))
        .unwrap_or_else(|| "jpg".to_owned());
    let target = dir.join(format!("avatar.{extension}"));

    // 先写新的、再删旧的：写失败时旧头像还留着，不至于把用户已有头像弄丢
    std::fs::write(&target, bytes)
        .map_err(|err| format!("写入 {} 失败：{err}", target.display()))?;
    for old_extension in AVATAR_EXTENSIONS {
        let old = dir.join(format!("avatar.{old_extension}"));
        if old != target && old.is_file() {
            let _ = std::fs::remove_file(&old);
        }
    }
    Ok(target)
}

/// 启动时决定用户头像：已保存的文件优先，否则用**内嵌**的默认图。
/// 读失败返回一句给用户看的原因（调用方追成一条 `⚠️` 消息），绝不静默吞掉。
fn load_initial_user_avatar() -> (UserAvatar, Option<String>) {
    let fallback = || UserAvatar {
        bytes: Some(default_user_avatar()),
        custom: false,
    };
    let Some(path) = stored_avatar_path() else {
        return (fallback(), None);
    };
    match std::fs::read(&path) {
        Ok(bytes) if bytes.is_empty() => (
            fallback(),
            Some(format!("已保存的头像文件是空的：{}", path.display())),
        ),
        Ok(bytes) if bytes.len() > MAX_IMAGE_BYTES => (
            fallback(),
            Some(format!(
                "已保存的头像太大（{:.1} MB），上限 {} MB：{}",
                bytes.len() as f64 / 1_048_576.0,
                MAX_IMAGE_BYTES / 1_048_576,
                path.display()
            )),
        ),
        // 字节留着给界面显示；解码失败由各个 Image 自己吞掉（只画环）
        Ok(bytes) => (
            UserAvatar {
                bytes: Some(Arc::from(bytes)),
                custom: true,
            },
            None,
        ),
        Err(err) => (
            fallback(),
            Some(format!("读不了保存的头像（{}）：{err}", path.display())),
        ),
    }
}

/// 把头像图贴进某一条消息的头像框。**只在部件新建时调**：
/// `load_image_from_data` 每次都会真解码，而列表每帧都会重绘。
fn load_item_avatar(cx: &mut Cx, item: &WidgetRef, role: Role) {
    let photo = item.image(cx, ids!(avatar_photo));
    match avatar_bytes(role) {
        Some(bytes) => {
            photo.set_visible(cx, true);
            // 解码失败（文件损坏）就只剩环，不打断绘制
            let _ = photo.load_image_from_data(cx, &bytes);
        }
        None => photo.set_visible(cx, false),
    }
}

// ---------------------------------------------------------------- 深色/浅色

/// 主题模式。网页版对应 `settings.isDarkMode`（localStorage）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }
}

/// 当前模式。`script_mod` 在启动与每次样式重载时都会读它来决定装哪套
/// `StyleSheet`，所以必须放在静态变量里，而不是 `App` 字段。
static THEME: RwLock<ThemeMode> = RwLock::new(ThemeMode::Dark);
static THEME_INIT: OnceLock<()> = OnceLock::new();

/// `%APPDATA%\DecemChat\theme`（与头像同目录）。
fn theme_file() -> Option<PathBuf> {
    avatar_dir().map(|dir| dir.join("theme"))
}

/// 启动时的优先级：`DECEM_THEME` 环境变量 > 持久化文件 > 深色。
/// 环境变量只覆盖本次运行、不落盘——否则自动化一跑就会改掉用户的选择。
fn load_theme_mode() -> ThemeMode {
    if let Ok(value) = std::env::var("DECEM_THEME") {
        if let Some(mode) = ThemeMode::parse(&value) {
            return mode;
        }
    }
    theme_file()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| ThemeMode::parse(&text))
        .unwrap_or_default()
}

fn theme_mode() -> ThemeMode {
    THEME_INIT.get_or_init(|| {
        *THEME.write().unwrap() = load_theme_mode();
    });
    *THEME.read().unwrap()
}

fn set_theme_mode(mode: ThemeMode) {
    let _ = theme_mode();
    *THEME.write().unwrap() = mode;
}

fn save_theme_mode(mode: ThemeMode) -> Result<(), String> {
    let path = theme_file().ok_or_else(|| "读不到 APPDATA 环境变量，无法保存主题".to_owned())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|err| format!("建不了目录 {}：{err}", dir.display()))?;
    }
    std::fs::write(&path, format!("{}\n", mode.as_str()))
        .map_err(|err| format!("写不了 {}：{err}", path.display()))
}

/// 深色沿用原样（BlackOrange）；浅色借 Makepad 的 Windows 浅色 sheet。
/// `DesktopStyle` 没有独立的 "Light" 变体，Windows 是其中最中性的一套浅色令牌；
/// 界面上的主要颜色仍由下面的 `Palette` 逐项覆盖（`theme` 只负责 Markdown、
/// GlassButton、TextInput 光标/选区这些主题驱动的部件）。
fn theme_stylesheet(mode: ThemeMode) -> StyleSheet {
    match mode {
        ThemeMode::Dark => StyleSheet::load(DesktopStyle::BlackOrange),
        ThemeMode::Light => StyleSheet::load(DesktopStyle::Windows),
    }
}

/// `#xRRGGBBAA` → `Vec4f`，与脚本里的颜色字面量同一字节序。
fn rgba(hex: u32) -> Vec4f {
    vec4(
        ((hex >> 24) & 0xff) as f32 / 255.0,
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    )
}

/// App 自己画的所有颜色。写死的深色必须在这里成对给出浅色值，
/// 否则切到浅色会只剩「浅底 + 深色块」。
#[derive(Clone, Copy)]
struct Palette {
    /// 根背景与窗口 clear 色
    bg: Vec4f,
    /// 主文字/标题
    fg: Vec4f,
    /// 次级文字（时间戳、描述、提示）
    muted_fg: Vec4f,
    /// 1px 分隔线/边框
    border: Vec4f,
    /// 顶栏/抽屉的玻璃底
    glass: Vec4f,
    /// 玻璃底的 1px 边框
    glass_border: Vec4f,
    /// char 气泡底
    bubble: Vec4f,
    /// char 气泡 1px 边框
    bubble_border: Vec4f,
    /// 输入区外层底
    input_bg: Vec4f,
    /// 胶囊/模型触发器/主题行底（muted/50）
    row_bg: Vec4f,
    /// 图标按钮 hover / 关闭按钮 hover
    hover: Vec4f,
    /// 图标按钮按下
    hover_down: Vec4f,
    /// 透明按钮盖层的 hover/down 叠加色
    tint_hover: Vec4f,
    tint_down: Vec4f,
    /// 确认弹层卡片
    card: Vec4f,
    /// 模型下拉列表容器
    model_list_bg: Vec4f,
    /// Modal 遮罩
    mask: Vec4f,
    /// 开关轨道：深色模式（开）珊瑚，浅色模式（关）深灰
    switch_track: Vec4f,
    /// 开关滑块
    switch_knob: Vec4f,
    /// 发送键禁用底色/文字
    disabled_bg: Vec4f,
    disabled_fg: Vec4f,
}

fn palette() -> Palette {
    match theme_mode() {
        ThemeMode::Dark => Palette {
            bg: rgba(0x000000FF),
            fg: rgba(0xFFFFFFFF),
            muted_fg: rgba(0xB3B3B3FF),
            border: rgba(0x333333FF),
            glass: rgba(0x1A1A1ACC),
            glass_border: rgba(0xFFFFFF1A),
            bubble: rgba(0x1A1A1AFF),
            bubble_border: rgba(0xFFFFFF1A),
            input_bg: rgba(0x141414CC),
            row_bg: rgba(0x26262680),
            hover: rgba(0x262626FF),
            hover_down: rgba(0x333333FF),
            tint_hover: rgba(0xFFFFFF0D),
            tint_down: rgba(0xFFFFFF1A),
            card: rgba(0x2B2B35FF),
            model_list_bg: rgba(0x1A1A1AFF),
            mask: rgba(0x00000080),
            switch_track: rgba(0xF18484FF),
            switch_knob: rgba(0x000000FF),
            disabled_bg: rgba(0x26262680),
            disabled_fg: rgba(0xB3B3B380),
        },
        // 浅色值取自网页版 `src/index.css` 的 `.light` 段：
        // background #F2F2F2、foreground #1A1A1A、muted #E6E6E6、
        // muted-foreground #666666、border #D9D9D9、message-bot #FFFFFF，
        // 玻璃底 `rgba(255,255,255,0.8)`，遮罩 `rgba(0,0,0,0.3)`。
        ThemeMode::Light => Palette {
            bg: rgba(0xF2F2F2FF),
            fg: rgba(0x1A1A1AFF),
            muted_fg: rgba(0x666666FF),
            border: rgba(0xD9D9D9FF),
            glass: rgba(0xFFFFFFCC),
            glass_border: rgba(0x0000001A),
            bubble: rgba(0xFFFFFFFF),
            bubble_border: rgba(0x0000001A),
            input_bg: rgba(0xF2F2F2CC),
            row_bg: rgba(0xE6E6E680),
            hover: rgba(0xE6E6E6FF),
            hover_down: rgba(0xD9D9D9FF),
            tint_hover: rgba(0x0000000D),
            tint_down: rgba(0x0000001A),
            card: rgba(0xFFFFFFFF),
            model_list_bg: rgba(0xFFFFFFFF),
            mask: rgba(0x0000004D),
            switch_track: rgba(0x262626FF),
            switch_knob: rgba(0xFFFFFFFF),
            disabled_bg: rgba(0xE6E6E680),
            disabled_fg: rgba(0x66666680),
        },
    }
}

/// 页脚「Ave atque vale」的颜色。网页版是 90° 珊瑚↔薄荷↔黄渐变 +
/// background-size 200% 的位移/呼吸动画；Makepad 没有「渐变裁剪到文字」，
/// 这里沿同一条渐变取一个随时间移动的颜色点，4s 一循环（呼吸由 10 Hz
/// 重绘驱动，与背景光晕同一套）。
fn footer_glow() -> Vec4f {
    const STOPS: [u32; 4] = [0xF38181FF, 0x95E1D3FF, 0xFCE38AFF, 0xF38181FF];
    let phase = (glow_seconds() / 4.0).fract() * 3.0;
    let index = (phase as usize).min(2);
    let local = phase - index as f32;
    let from = STOPS[index];
    let to = STOPS[index + 1];
    let channel = |shift: u32| {
        let a = ((from >> shift) & 0xff) as f32;
        let b = ((to >> shift) & 0xff) as f32;
        (a + (b - a) * local) / 255.0
    };
    vec4(channel(24), channel(16), channel(8), 1.0)
}

/// 开关滑块位置：轨道 32 宽、滑块 16、上下留 1.2。
fn switch_knob_left() -> f64 {
    if theme_mode() == ThemeMode::Dark {
        32.0 - 16.0 - 1.2
    } else {
        1.2
    }
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

/// 当前选中的模型 ID。模型表、展示名与视觉能力判断都取 `decem_core::models`，
/// 这里只存一个 ID，切换后 `build_request_body` 立刻读到新值。
static MODEL_ID: RwLock<&'static str> = RwLock::new(DEFAULT_MODEL);

fn current_model_id() -> &'static str {
    // poisoned 说明另一个线程在持锁时 panic；这里的值仍是完整可读的
    *MODEL_ID.read().unwrap()
}

/// 把消息历史拼成上游请求体。`send` 与 `regenerate` 共用，
/// 保证「重发同一条用户消息」与「发新消息」走完全相同的编排。
///
/// system 提示词 + 最近 10 条由 `ChatRequest::new` 负责编排；
/// 图片在 `format_messages` 里按模型是否支持视觉决定拼成多模态数组还是降级成文字。
fn build_request_body(messages: &[Msg]) -> Result<Vec<u8>, String> {
    let history: Vec<Message> = messages
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
    let request = ChatRequest::new(current_model_id(), &history);
    serde_json::to_vec(&request.to_upstream()).map_err(|err| format!("请求体序列化失败：{err}"))
}

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

// ---------------------------------------------------------------- 背景光晕

/// 光晕的呼吸时钟。`script_mod` 每次重载（切主题）都会重建部件树，
/// 但动画相位不应该跟着归零，所以起点放在进程级静态里。
static GLOW_EPOCH: OnceLock<std::time::Instant> = OnceLock::new();

fn glow_seconds() -> f32 {
    GLOW_EPOCH
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f32()
}

/// 根背景 + 三层径向光晕（规格 §1），一次全屏 quad 画完。
///
/// 网页版是 `radial-gradient(circle, rgba(…,0.4) 0%, transparent 70%)`
/// 再叠 `filter: blur(40..60px)`。这里用解析式近似：径向衰减取
/// `exp(-2.7·t²)`（t = 到圆心距离 / 半径），它是上述「0% 处 0.4 →
/// 70% 处 0」的圆锥剖面再经高斯模糊后的拟合（逐点数值卷积得到，
/// t=0.5/0.7/1.0 时的相对值 0.51/0.26/0.06）。真正的逐像素模糊需要
/// 离屏 pass，为了三层极淡的光晕不值当。
#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawBackdrop {
    #[deref]
    draw_super: DrawQuad,
    /// 主题底色（深色 #000000 / 浅色 #F2F2F2）。
    #[live]
    bg: Vec4f,
    /// 自启动起的秒数，驱动 `pulse-glow` 的 3s 循环。
    #[live(0.0)]
    glow_time: f32,
}

/// 整个 body 的容器：先在最底层画底色与光晕，再把 walk 交给内部 View。
///
/// 光晕画在子层之前，所以永远在消息/顶栏/输入区之下；这个部件也不转发
/// 自己的事件（只转发内部 view 的），不会抢 TextInput 的命中区。
#[derive(Script, ScriptHook, Widget)]
pub struct Backdrop {
    #[deref]
    view: View,
    #[redraw]
    #[live]
    draw_glow: DrawBackdrop,
}

impl Widget for Backdrop {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // 只读当前 turtle 的内框，不消费 walk —— 下面还要把同一个 walk
        // 交给内部 view 排版。根部件是 Fill/Fill，内框就是整个 body。
        let rect = cx.turtle().inner_rect();
        if rect.size.x > 0.0 && rect.size.y > 0.0 {
            self.draw_glow.glow_time = glow_seconds();
            self.draw_glow.draw_abs(cx, rect);
        }
        // 子层可能 step（里面有 PortalList），走完再交回 done（同 Bubble）
        let mut step = self.view.draw_walk(cx, scope, walk);
        while step.is_step() {
            step = self.view.draw_walk(cx, scope, walk);
        }
        DrawStep::done()
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
            // 余量：这只测量实例与 Markdown 的渲染实例不是同一个；换成 Lato 之后
            // emoji 与字距的出入比 IBMPlex 时更大（8px 会让欢迎语提前折行），给到 16
            (used + self.pad_x + 16.0).clamp(60.0, cap)
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
    /// 必须自己把事件转发给内部的 `#[deref] view`。
    ///
    /// `Widget::handle_event` 的**默认实现是空的**（`widget.rs:302`），而 `#[derive(Widget)]`
    /// 只生成 `WidgetNode`、不生成转发（`derive_widget.rs:108`）。所以一个自定义容器
    /// 如果不写这个方法，里面的 `PortalList` 就**收不到任何鼠标事件**——
    /// 表现为：列表里的按钮点不动、拖拽滚动失效、文字选不了。
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

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
                        let (item_widget, existed) = list.item_with_existed(cx, item_id, id!(Char));
                        if !existed {
                            // 流式那条没有 Msg，但头像仍然是 char 的
                            load_item_avatar(cx, &item_widget, Role::Char);
                        }
                        // 流式中不显示时间戳与「重新生成」
                        item_widget
                            .glass_button(cx, ids!(regen_button))
                            .set_visible(cx, false);
                        item_widget.label(cx, ids!(time)).set_text(cx, "");
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
                        // 「重新生成」只挂在最后一条 char 消息上，流式时全部隐藏
                        // （与网页版同一个条件：最后一条 assistant 且非流式）。
                        // User 模板里没有这个按钮，对它调用只会拿到空引用，
                        // set_visible 是空操作；set_visible 只在值变化时 redraw。
                        let show_regen = !chat.is_streaming
                            && item_id + 1 == chat.messages.len()
                            && msg.role == Role::Char;
                        item_widget
                            .glass_button(cx, ids!(regen_button))
                            .set_visible(cx, show_regen);
                        // 时间戳在气泡下方、内容列里；用户侧只有时间戳
                        item_widget.label(cx, ids!(time)).set_text(cx, &msg.time);
                        let mut markdown = item_widget.markdown(cx, ids!(selectable));
                        markdown.set_text(cx, &msg.text);
                        if was_animating {
                            markdown.stop_streaming_animation();
                        }
                        // 图片只在部件**新建**时解一次。`load_image_from_data` 每次都真解码，
                        // 而这条路径每帧都会跑到，无条件调就是每帧重解码。`existed` 帮我们
                        // 分清「新建」和「复用」；部件被回收后再滚回来会重建，于是自然重解一次。
                        // 一条消息的图不会变，所以显隐也在这里一次性设好。
                        if !existed {
                            // 头像与消息图片都只在新建时解码一次
                            load_item_avatar(cx, &item_widget, msg.role);
                        }
                        if !existed && msg.role == Role::User {
                            let photo = item_widget.image(cx, ids!(photo));
                            match &msg.image {
                                Some(attachment) => {
                                    photo.set_visible(cx, true);
                                    // 选图那一刻已经解成功过一次，这里再失败就只是不显示
                                    let _ = photo.load_image_from_data(cx, &attachment.bytes);
                                }
                                None => {
                                    // 空的 Image 在 Fit/Smallest 下会占住请求高度，
                                    // 纯文字消息的气泡会被撑成一大块空白 —— 必须显式隐藏
                                    photo.set_visible(cx, false);
                                }
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

// ---------------------------------------------------------------- 设置抽屉

/// 设置抽屉里的一行模型（规格 §6）。
///
/// 没有用 Makepad 的 `DropDown`：它一行只有一段文字，做不出「名字 + 薄荷徽章 +
/// 描述」的两行结构，也管不了对勾固定灰色。这里由 `ModelList` 逐项喂文字，
/// 行自己负责悬停/选中的底色与文字反色。
#[derive(Script, ScriptHook, Widget)]
pub struct ModelRow {
    #[deref]
    view: View,
    /// 在 `AVAILABLE_MODELS` 里的下标，由 `ModelList` 画之前写入。
    #[rust]
    index: usize,
    #[rust]
    hovered: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ModelRowAction {
    #[default]
    None,
    Clicked,
}

/// 模型列表容器：包一层 PortalList，按 `AVAILABLE_MODELS` 逐项写入文字与徽章。
/// 与 `ChatList` 同一个路子（列表项内容只能在 Rust 侧按数据写）。
#[derive(Script, ScriptHook, Widget)]
pub struct ModelList {
    #[deref]
    view: View,
}

impl Widget for ModelRow {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let selected = AVAILABLE_MODELS
            .get(self.index)
            .is_some_and(|model| model.id == current_model_id());
        let active = selected || self.hovered;
        // 选中/悬停：accent 底 + 黑字；其余用当前模式的正文/次级灰（规格 §6）
        let p = palette();
        let ink = if active {
            vec4(0.0, 0.0, 0.0, 1.0)
        } else {
            p.fg
        };
        let sub_ink = if active { ink } else { p.muted_fg };

        self.view.widget(cx, ids!(row_hl)).set_visible(cx, active);
        self.view
            .label(cx, ids!(model_name))
            .set_text_color(cx, ink);
        self.view
            .label(cx, ids!(model_desc))
            .set_text_color(cx, sub_ink);
        // 对勾颜色在脚本里固定 #B3B3B3，不随高亮变黑（规格 §6）
        self.view.widget(cx, ids!(check)).set_visible(cx, selected);

        self.view.draw_walk(cx, scope, walk)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // 直接读原始鼠标事件，不依赖 View 的交互开关：
        // 自定义部件的根 View 即使写了 cursor，也不保证拿到 hover/click 动作。
        if matches!(event, Event::MouseMove(_)) {
            match event.hits(cx, self.view.area()) {
                Hit::FingerHoverIn(_) | Hit::FingerHoverOver(_) if !self.hovered => {
                    self.hovered = true;
                    cx.redraw_all();
                }
                Hit::FingerHoverOut(_) if self.hovered => {
                    self.hovered = false;
                    cx.redraw_all();
                }
                _ => {}
            }
        }
        if matches!(event, Event::MouseUp(_)) {
            if let Hit::FingerUp(up) = event.hits(cx, self.view.area()) {
                if up.was_tap() {
                    cx.widget_action(self.widget_uid(), ModelRowAction::Clicked);
                }
            }
        }
        self.view.handle_event(cx, event, scope);
    }
}

impl Widget for ModelList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // 必须手动转发：`#[derive(Widget)]` 只生成 WidgetNode，不生成
        // `Widget::handle_event`（它的默认实现是空的）。不转的话内层
        // PortalList 收不到任何鼠标事件，行就永远点不中。
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                list.set_item_range(cx, 0, AVAILABLE_MODELS.len());
                while let Some(item_id) = list.next_visible_item(cx) {
                    let Some(model) = AVAILABLE_MODELS.get(item_id) else {
                        continue;
                    };
                    let (item_widget, _existed) = list.item_with_existed(cx, item_id, id!(Row));
                    item_widget
                        .label(cx, ids!(model_name))
                        .set_text(cx, model.name);
                    item_widget
                        .label(cx, ids!(model_desc))
                        .set_text(cx, model.description);
                    item_widget
                        .widget(cx, ids!(vision_badge))
                        .set_visible(cx, model.supports_vision);
                    if let Some(mut row) = item_widget.as_model_row().borrow_mut() {
                        row.index = item_id;
                    }
                    item_widget.draw_all_unscoped(cx);
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
    /// `DECEM_AVATAR` 启动钩子只查一次环境变量。
    #[rust]
    startup_avatar_checked: bool,
    /// 顶栏头像只贴一次图。
    #[rust]
    avatars_loaded: bool,
    /// 发送键的禁用态只在首帧同步一次（之后跟着动作批同步）。
    #[rust]
    send_state_synced: bool,
    /// 设置抽屉里的模型列表是否展开。抽屉每次打开时先收起。
    #[rust]
    models_open: bool,
    /// 抽屉里的文字首帧同步一次（部件树建好之前拿不到 Label）。
    #[rust]
    drawer_synced: bool,
    /// 背景光晕的呼吸节拍（10 Hz）。网页版是 3s 的 CSS 动画；全速 60 Hz
    /// 重绘对这层 3–6% 的光晕没必要，10 Hz 已经看不出跳变。
    #[rust]
    glow_timer: Option<Timer>,
}

/// 文件对话框的 id，用来认出哪个对话框回了话。
const PICK_IMAGE: LiveId = live_id!(pick_image);
/// 头像上传用另一个 id：同一个 `FileDialogAction` 要区分「发消息的图」与「头像」。
const PICK_AVATAR: LiveId = live_id!(pick_avatar);

/// 图片对话框的扩展名过滤（图片消息与头像共用，保持一致）。
fn image_extensions() -> Vec<String> {
    ["png", "jpg", "jpeg", "gif", "webp", "bmp"]
        .iter()
        .map(|ext| (*ext).to_owned())
        .collect()
}

impl App {
    /// 弹系统文件对话框挑一张图。结果走平台动作异步回来（见 `handle_file_dialog`）。
    /// `want_bytes(true)` 让平台在 worker 线程把字节读好，省得自己碰文件系统。
    fn pick_image(&mut self, cx: &mut Cx) {
        let dialog = FileDialog::new()
            .set_id(PICK_IMAGE)
            .set_title("选一张图".to_owned())
            .add_filter("图片".to_owned(), image_extensions());
        cx.open_select_file_dialog(dialog);
    }

    /// 弹系统文件对话框挑头像。过滤条件与图片消息完全一致。
    fn pick_avatar(&mut self, cx: &mut Cx) {
        let dialog = FileDialog::new()
            .set_id(PICK_AVATAR)
            .set_title("选一张头像".to_owned())
            .add_filter("图片".to_owned(), image_extensions());
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
            let Some(path) = paths.first() else {
                break;
            };
            match *id {
                PICK_IMAGE => self.accept_image(cx, path),
                PICK_AVATAR => self.accept_avatar(cx, path),
                _ => continue,
            }
            break;
        }
    }

    /// 换头像：读字节 → 解码验证 → 落盘 → 更新全局头像（预览、顶栏、后续消息）。
    /// 文件对话框与 `DECEM_AVATAR` 启动钩子都走这里。
    fn accept_avatar(&mut self, cx: &mut Cx, path: &Path) {
        let bytes: Arc<[u8]> = match std::fs::read(path) {
            Ok(bytes) if bytes.is_empty() => {
                push_message(format!("⚠️ 这张图是空文件：{}", path.display()));
                return;
            }
            Ok(bytes) if bytes.len() > MAX_IMAGE_BYTES => {
                push_message(format!(
                    "⚠️ 头像图片太大（{:.1} MB），上限 {} MB",
                    bytes.len() as f64 / 1_048_576.0,
                    MAX_IMAGE_BYTES / 1_048_576
                ));
                return;
            }
            Ok(bytes) => Arc::from(bytes),
            Err(err) => {
                push_message(format!("⚠️ 读不了头像文件（{}）：{err}", path.display()));
                return;
            }
        };

        // 先解码验证：读不出来的文件不当成头像，也不落盘
        let preview = self.ui.image(cx, ids!(avatar_preview_photo));
        if let Err(err) = preview.load_image_from_data(cx, &bytes) {
            push_message(format!("⚠️ 这张图读不出来：{err}"));
            return;
        }

        *USER_AVATAR.write().unwrap() = Some(UserAvatar {
            bytes: Some(bytes.clone()),
            custom: true,
        });
        preview.set_visible(cx, true);
        self.reload_header_avatar(cx);

        // 落盘失败时头像在本次会话里仍然生效，但必须让用户知道重启后会丢
        if let Err(reason) = save_avatar(&bytes, path) {
            push_message(format!("⚠️ 头像已更新，但保存失败：{reason}"));
        }
        cx.redraw_all();
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
        photo.set_visible(cx, true);
        self.pending_image = Some(attachment);
        // 只有图片、没有文字也能发：待发图一进来就要点亮发送键。
        self.sync_send_enabled(cx);
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

    /// 顶栏头像：部件只建一次，所以第一次事件时贴图即可。
    fn load_header_avatar(&mut self, cx: &mut Cx) {
        if self.avatars_loaded {
            return;
        }
        self.avatars_loaded = true;
        self.reload_header_avatar(cx);
    }

    /// 顶栏头像随时可以重贴（换头像后立即生效）。
    fn reload_header_avatar(&self, cx: &mut Cx) {
        let photo = self.ui.image(cx, ids!(header_photo));
        match header_avatar_bytes() {
            Some(bytes) => {
                photo.set_visible(cx, true);
                // 解码失败（文件损坏）就只剩环，不打断绘制
                let _ = photo.load_image_from_data(cx, &bytes);
            }
            None => photo.set_visible(cx, false),
        }
    }

    /// 抽屉里的 64×64 头像预览 + 顶栏头像。首帧、打开抽屉、换头像时各同步一次。
    fn sync_avatar_ui(&self, cx: &mut Cx) {
        let photo = self.ui.image(cx, ids!(avatar_preview_photo));
        match avatar_bytes(Role::User) {
            Some(bytes) => {
                let _ = photo.load_image_from_data(cx, &bytes);
                photo.set_visible(cx, true);
            }
            None => photo.set_visible(cx, false),
        }
        self.reload_header_avatar(cx);
    }

    /// 抽屉里的深浅色行：文字与图标随模式变。颜色由 `script_mod` 里的
    /// `palette()` 插值在样式重载时更新；这里只负责字符串（`ScriptReapply`
    /// 会保留运行期 setter 写进去的文字，不会用脚本里的空串盖回来）。
    fn sync_theme_ui(&self, cx: &mut Cx) {
        let dark = theme_mode() == ThemeMode::Dark;
        self.ui
            .label(cx, ids!(theme_mode_label))
            .set_text(cx, if dark { "深色模式" } else { "浅色模式" });
        self.ui
            .label(cx, ids!(theme_mode_icon))
            // 主题自带 FontAwesome 的单色字形：U+F186 = fa-moon（深色模式）、
            // U+F185 = fa-sun（浅色模式）。彩色 emoji 的黄色由字体自带，`color` 染不上。
            .set_text(cx, if dark { "\u{f186}" } else { "\u{f185}" });
    }

    /// 切换主题：先改全局状态（`script_mod` 重载时读它），落盘，再请求
    /// 样式重载。真正重绘发生在下一个 `Event::LiveEdit`，`Apply::ScriptReapply`
    /// 会保留输入框文字与正在运行的动画。
    fn set_theme(&mut self, cx: &mut Cx, mode: ThemeMode) {
        if theme_mode() == mode {
            return;
        }
        set_theme_mode(mode);
        if let Err(reason) = save_theme_mode(mode) {
            push_message(format!("⚠️ 主题已切换，但保存失败：{reason}"));
        }
        self.sync_theme_ui(cx);
        desktop_style::handle_event(cx, &Event::Custom(theme_stylesheet(mode).to_json()));
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

    /// 头像版的启动钩子：`DECEM_AVATAR=<路径>` 时走与文件对话框完全相同的
    /// 「读入 → 解码 → 替换预览/顶栏 → 持久化」链路（系统对话框没法自动化驱动）。
    fn load_startup_avatar(&mut self, cx: &mut Cx) {
        if self.startup_avatar_checked {
            return;
        }
        self.startup_avatar_checked = true;
        if let Ok(path) = std::env::var("DECEM_AVATAR") {
            self.accept_avatar(cx, Path::new(&path));
        }
    }

    /// 胶囊的聚焦描边：Makepad 没有 box-shadow，改用 `focus_ring` 那圈 2px 内描边。
    /// TextInput 在拿到/失去键盘焦点时会发 `KeyFocus` / `KeyFocusLost` 动作
    /// （widgets/core/src/text_input.rs:3063、2237），这里照着切显隐。
    fn sync_input_focus(&self, cx: &mut Cx, actions: &Actions) {
        let uid = self.ui.text_input(cx, ids!(input)).widget_uid();
        let mut focused = None;
        for action in actions.filter_widget_actions_cast::<TextInputAction>(uid) {
            match action {
                TextInputAction::KeyFocus => focused = Some(true),
                TextInputAction::KeyFocusLost => focused = Some(false),
                _ => {}
            }
        }
        if let Some(focused) = focused {
            self.ui
                .widget(cx, ids!(focus_ring))
                .set_visible(cx, focused);
        }
    }

    /// 发送键禁用态：没有文字也没有待发图片时置灰（规格 §4）。
    fn sync_send_enabled(&self, cx: &mut Cx) {
        let has_text = !self.ui.text_input(cx, ids!(input)).text().trim().is_empty();
        let enabled = has_text || self.pending_image.is_some();
        let mut send_button = self.ui.widget(cx, ids!(send_button));
        send_button.set_disabled(cx, !enabled);
        // SVG 图标是单色 uniform，没有 `color_disabled` 槽位：跟着禁用态换色，
        // 否则置灰的珊瑚键上会留一架纯白的纸飞机（网页版是 muted-foreground）。
        let icon_color = if enabled {
            rgba(0xFFFFFFFF)
        } else {
            palette().disabled_fg
        };
        script_apply_eval!(cx, send_button, {
            draw_icon +: { color: #(icon_color) }
        });
    }

    /// 抽屉里与当前模型有关的三处文字/徽章：触发器、列表选中行、底部「当前: X」。
    /// 列表选中行由 `ModelList` 每帧按 `MODEL_ID` 重画，这里只同步常驻部件。
    fn sync_model_ui(&self, cx: &mut Cx) {
        let model = current_model_id();
        let name = model_display_name(model);
        self.ui
            .label(cx, ids!(model_trigger_label))
            .set_text(cx, &name);
        self.ui
            .label(cx, ids!(model_current_label))
            .set_text(cx, &format!("当前: {name}"));
        self.ui
            .widget(cx, ids!(current_badge))
            .set_visible(cx, is_vision_model(model));
        cx.redraw_all();
    }

    /// 展开/收起模型列表。收起时同时把高度压成 0：
    /// Makepad 的 `visible: false` 仍会在布局里留坑，只隐藏会空出一大块。
    fn set_models_open(&self, cx: &mut Cx, open: bool) {
        let list_box = self.ui.widget(cx, ids!(model_list_box));
        list_box.set_visible(cx, open);
        list_box.as_view().set_walk(
            cx,
            Walk {
                width: Size::fill(),
                height: if open { Size::fit() } else { Size::Fixed(0.0) },
                ..Walk::default()
            },
        );
        cx.redraw_all();
    }

    /// 选中第 `index` 个模型：只记 ID，下一条消息（含重新生成）由
    /// `build_request_body` 读取。
    fn select_model(&mut self, cx: &mut Cx, index: usize) {
        let Some(model) = AVAILABLE_MODELS.get(index) else {
            return;
        };
        *MODEL_ID.write().unwrap() = model.id;
        self.models_open = false;
        self.set_models_open(cx, false);
        self.sync_model_ui(cx);
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
                time: now_hhmm(),
            });
            chat.streaming.clear();
            chat.is_streaming = true;

            let payload = build_request_body(&chat.messages);
            (payload, chat.messages.len() + 1)
        };

        // 预览图跟着清掉（空图要一起隐藏，否则会白占一块高度）
        let pending_photo = self.ui.image(cx, ids!(pending_photo));
        pending_photo.set_texture(cx, None);
        pending_photo.set_visible(cx, false);

        match payload {
            Ok(body) => self.worker = Some(Worker::spawn_chat(body)),
            Err(reason) => {
                push_message(format!("⚠️ {reason}"));
                self.worker = None;
            }
        }

        self.scroll_to_tail(cx, items_len);
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

    /// 重新生成最后一段问答：丢掉最后一条 char 回复，用同一条用户消息再请求一次。
    /// 流式中不生效（`worker` 还在）——网页版只靠隐藏按钮防并发，这里照 egui 版加守门。
    fn regenerate(&mut self, cx: &mut Cx) {
        if self.worker.is_some() {
            return;
        }
        // 截断与组请求体在同一把锁里做完，避免中途被 UI 线程插队
        let (payload, items_len) = {
            let mut chat = CHAT.write().unwrap();
            let Some(last_user) = chat.messages.iter().rposition(|msg| msg.role == Role::User)
            else {
                // 没有用户消息（空记录 / 只有欢迎语）：什么都不做
                return;
            };
            // 丢掉该用户消息之后的 char 回复。egui 版同法；网页版只删最后一条，
            // 但失败提示也是 char 消息，删到最后一条用户消息为止更稳
            chat.messages.truncate(last_user + 1);
            chat.streaming.clear();
            chat.is_streaming = true;
            let payload = build_request_body(&chat.messages);
            (payload, chat.messages.len() + 1)
        };

        match payload {
            Ok(body) => self.worker = Some(Worker::spawn_chat(body)),
            Err(reason) => {
                push_message(format!("⚠️ {reason}"));
                self.worker = None;
            }
        }

        self.scroll_to_tail(cx, items_len);
    }

    /// 清空聊天记录（已经过二次确认）：打断流式、清掉所有消息，只留一条欢迎语。
    /// 照 egui 版 `clear_chat()` 先打断 worker——网页版不打断，
    /// 进行中的请求完成时会把消息「复活」回刚清空的列表。
    /// （本版没有持久化与 IndexedDB 图片库，网页版那两步在这里没有对应物。）
    fn clear_chat(&mut self, cx: &mut Cx) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.worker = None;
        {
            let mut chat = CHAT.write().unwrap();
            chat.streaming.clear();
            chat.is_streaming = false;
            chat.messages.clear();
            chat.messages.push(Msg {
                role: Role::Char,
                text: WELCOME.to_owned(),
                image: None,
                time: now_hhmm(),
            });
        }
        self.scroll_to_tail(cx, 1);
    }

    /// 把列表滚到指定项（新消息或正在流式的那一项）。send / regenerate / clear 共用。
    fn scroll_to_tail(&self, cx: &mut Cx, items_len: usize) {
        let list = self
            .ui
            .widget(cx, ids!(chat_list))
            .portal_list(cx, ids!(list));
        list.set_tail_range(true);
        list.set_first_id_and_scroll(items_len.saturating_sub(1), 0.0);
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
                time: now_hhmm(),
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
        if self.ui.button(cx, ids!(attach_button)).clicked(actions) {
            self.pick_image(cx);
        }
        if self.ui.button(cx, ids!(send_button)).clicked(actions) {
            self.send(cx);
        }
        // 清空聊天记录：先弹二次确认。Modal 的 can_dismiss 默认 true，
        // 点遮罩或 Esc 即取消，Modal 自己关，不用接 dismissed。
        if self
            .ui
            .button(cx, ids!(drawer_clear_button))
            .clicked(actions)
        {
            self.ui.modal(cx, ids!(clear_confirm)).open(cx);
        }
        if self
            .ui
            .glass_button(cx, ids!(confirm_clear_button))
            .clicked(actions)
        {
            self.ui.modal(cx, ids!(clear_confirm)).close(cx);
            self.clear_chat(cx);
        }
        if self
            .ui
            .glass_button(cx, ids!(cancel_clear_button))
            .clicked(actions)
        {
            self.ui.modal(cx, ids!(clear_confirm)).close(cx);
        }
        // 设置抽屉：齿轮开、头部 ✕ 关（点遮罩 / Esc 由 Modal 自己关）。
        if self.ui.button(cx, ids!(settings_button)).clicked(actions) {
            self.models_open = false;
            self.set_models_open(cx, false);
            self.sync_model_ui(cx);
            // 抽屉里的头像预览可能与最新状态不同步（换头像只改了全局状态）
            self.sync_avatar_ui(cx);
            self.sync_theme_ui(cx);
            self.ui.modal(cx, ids!(settings_drawer)).open(cx);
        }
        // 深浅色开关：翻转模式并立即请求样式重载。
        if self
            .ui
            .button(cx, ids!(theme_toggle_button))
            .clicked(actions)
        {
            let next = theme_mode().toggled();
            self.set_theme(cx, next);
        }
        // 「上传头像」：走与图片消息同一个系统文件对话框，id 不同。
        if self
            .ui
            .button(cx, ids!(upload_avatar_button))
            .clicked(actions)
        {
            self.pick_avatar(cx);
        }
        if self
            .ui
            .button(cx, ids!(drawer_close_button))
            .clicked(actions)
        {
            self.ui.modal(cx, ids!(settings_drawer)).close(cx);
        }
        if self
            .ui
            .button(cx, ids!(model_trigger_button))
            .clicked(actions)
        {
            self.models_open = !self.models_open;
            self.set_models_open(cx, self.models_open);
        }
        // 模型行是 PortalList 里的自定义行，动作带 item uid，
        // 用 item_id 直接下标 `AVAILABLE_MODELS`（与列表填充同序）。
        let model_rows = self.ui.portal_list(cx, ids!(model_rows));
        let mut picked = None;
        for (item_id, item) in model_rows.items_with_actions(actions) {
            if actions.find_widget_action_cast::<ModelRowAction>(item.widget_uid())
                == ModelRowAction::Clicked
            {
                picked = Some(item_id);
            }
        }
        // 同一批 actions 里可能同时带 hover 与 click 两条，`items_with_actions`
        // 会把同一个 item 返回多次；只认最后一次，避免重复 select。
        if let Some(item_id) = picked {
            self.select_model(cx, item_id);
        }
        // 最后一条 char 气泡底部的「重新生成」。逐项动作照 aichat 示例的
        // 逐项删除：动作带着所在 item 的 uid，这里按 item 找回按钮。
        // regenerate 内部还有流式守门与最后一条用户消息的重查，
        // 就算按错了 item 也不会出事。
        let list = self
            .ui
            .widget(cx, ids!(chat_list))
            .portal_list(cx, ids!(list));
        for (_, item) in list.items_with_actions(actions) {
            if item.glass_button(cx, ids!(regen_button)).clicked(actions) {
                self.regenerate(cx);
            }
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

        // 输入区的两处状态：聚焦描边（TextInput 的 KeyFocus/KeyFocusLost 动作）
        // 与发送键禁用态（空输入且没有待发图时置灰）。
        self.sync_input_focus(cx, actions);
        self.sync_send_enabled(cx);
    }
}

/// 网页版用的拉丁字体（`src/index.css:1` 引的三族）：正文 Lato、标题 Poppins、
/// 代码 Fira Code。文件是 Google Fonts 的 `latin` 子集转回 TTF（Makepad 的
/// ttf-parser 不带 woff/woff2 解码），所以只覆盖 U+0000-00FF 与常用标点。
const LATO_REGULAR_TTF: &[u8] = include_bytes!("../resources/fonts/Lato-Regular.ttf");
const LATO_BOLD_TTF: &[u8] = include_bytes!("../resources/fonts/Lato-Bold.ttf");
const POPPINS_SEMIBOLD_TTF: &[u8] = include_bytes!("../resources/fonts/Poppins-SemiBold.ttf");
const FIRA_CODE_REGULAR_TTF: &[u8] = include_bytes!("../resources/fonts/FiraCode-Regular.ttf");

/// lucide 0.562.0 的六个图标原文件。**嵌进二进制**：`crate_resource("self:...")`
/// 在运行期是按编译期 manifest 路径拼绝对路径读磁盘的
/// （`platform/src/script/res.rs:842-905`），exe 拷到别处就丢图标（见 §25.2）。
/// `include_bytes!` 只要求编译期源码树里有这些文件，打包后的 exe 不再依赖它们。
const ICON_SPARKLES_SVG: &[u8] = include_bytes!("../resources/icons/sparkles.svg");
const ICON_SETTINGS_SVG: &[u8] = include_bytes!("../resources/icons/settings.svg");
const ICON_GITHUB_SVG: &[u8] = include_bytes!("../resources/icons/github.svg");
const ICON_IMAGE_SVG: &[u8] = include_bytes!("../resources/icons/image.svg");
const ICON_SEND_SVG: &[u8] = include_bytes!("../resources/icons/send.svg");
const ICON_BOT_SVG: &[u8] = include_bytes!("../resources/icons/bot.svg");

/// 把六个 SVG 注册成内存资源，挂到 `mod.decem_icons`。
///
/// 与 `install_web_fonts` 同一套路：`res.binary_resource`（`res.rs:1186-1218`）
/// 建一个 `binary://` 资源，数据直接是内存字节；`DrawSvg::load_svg`
/// （`draw/src/shader/draw_svg.rs:466-501`）走 `cx.get_resource`，拿到的是已
/// Loaded 的字节，不碰磁盘。
///
/// 必须在 `self::script_mod` 之前调用——DSL 里的 `svg: mod.decem_icons.sparkles`
/// 要在求值时就能解析到这个全局。
fn install_icons(vm: &mut ScriptVm) {
    // 与字体同理：样式重载会重跑 script_mod，但 heap 不重置，资源只建一次。
    static EMBEDDED: OnceLock<()> = OnceLock::new();
    if EMBEDDED.get().is_none() {
        for (name, bytes) in [
            (id!(sparkles_svg), ICON_SPARKLES_SVG),
            (id!(settings_svg), ICON_SETTINGS_SVG),
            (id!(github_svg), ICON_GITHUB_SVG),
            (id!(image_svg), ICON_IMAGE_SVG),
            (id!(send_svg), ICON_SEND_SVG),
            (id!(bot_svg), ICON_BOT_SVG),
        ] {
            let bytes: ScriptValue = vm.bx.heap.new_array_from_vec_u8(bytes.to_vec()).into();
            vm.set_injected_global(name, bytes);
        }
        script_eval!(vm, {
            mod.decem_icons = {
                sparkles: mod.res.binary_resource(sparkles_svg)
                settings: mod.res.binary_resource(settings_svg)
                github: mod.res.binary_resource(github_svg)
                image: mod.res.binary_resource(image_svg)
                send: mod.res.binary_resource(send_svg)
                bot: mod.res.binary_resource(bot_svg)
            }
        });
        let _ = EMBEDDED.set(());
    }
}

/// 把嵌进二进制的字体注册成 Makepad 资源，并把主题里几个 `TextStyle` 的
/// `font_family` 换成「自定义拉丁字体 + 原来的 IBM Plex + LXGW + Noto Color Emoji」。
///
/// 注册走脚本侧的 `res.binary_resource`（`platform/src/script/res.rs:1186`），
/// 资源路径是 `binary://<id>`；`Cx::get_resource_font_bytes_by_path`
/// （`platform/src/cx_api.rs:1043`）先试文件、失败再回落到这份内存字节，所以
/// 运行时不需要源码树里的字体文件（Android 打包也一样）。
///
/// 每个族都保留原字体链：拉丁子集没有箭头等符号，仍要 IBM Plex 兜底；中文与
/// emoji 由 LXGW / Noto Color Emoji 渲染——这就是 `FontSet::International`
/// 原来的链条（`platform/src/font_policy.rs:338`），顺序不能丢。
fn install_web_fonts(vm: &mut ScriptVm) {
    // 样式重载会重跑 script_mod，但 heap 不重置：资源只建一次，
    // 否则每切一次深浅色就多留一份字体字节。
    // 用 Rust 侧的 flag 而不是脚本里读 `mod.decem_fonts`——读一个还不存在的
    // 属性会打一条 `[E] property ... not found`，冒烟要求日志里没有 `[E]`。
    static EMBEDDED: OnceLock<()> = OnceLock::new();
    if EMBEDDED.get().is_none() {
        for (name, bytes) in [
            (id!(lato_regular_ttf), LATO_REGULAR_TTF),
            (id!(lato_bold_ttf), LATO_BOLD_TTF),
            (id!(poppins_semibold_ttf), POPPINS_SEMIBOLD_TTF),
            (id!(fira_code_ttf), FIRA_CODE_REGULAR_TTF),
        ] {
            let bytes: ScriptValue = vm.bx.heap.new_array_from_vec_u8(bytes.to_vec()).into();
            vm.set_injected_global(name, bytes);
        }
        script_eval!(vm, {
            mod.decem_fonts = {
                lato_regular: mod.res.binary_resource(lato_regular_ttf)
                lato_bold: mod.res.binary_resource(lato_bold_ttf)
                poppins_semibold: mod.res.binary_resource(poppins_semibold_ttf)
                fira_code: mod.res.binary_resource(fira_code_ttf)
            }
        });
        let _ = EMBEDDED.set(());
    }

    script_eval!(vm, {
        use mod.text.*
        // 用 `theme.x{...}` 合并而不是新建 TextStyle：字体大小、行距仍归主题管。
        mod.theme.font_regular = mod.theme.font_regular{font_family: FontFamily{
            lato := FontMember{res: mod.decem_fonts.lato_regular asc: 0.0 desc: 0.0}
            plex := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/IBMPlexSans-Text.ttf") asc: -0.1 desc: 0.0}
            cjk := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/LXGWWenKaiRegular.ttf") asc: 0.0 desc: 0.0}
            emoji := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/NotoColorEmoji.ttf") asc: 0.0 desc: 0.0}
        }}
        mod.theme.font_bold = mod.theme.font_bold{font_family: FontFamily{
            lato := FontMember{res: mod.decem_fonts.lato_bold asc: 0.0 desc: 0.0}
            plex := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
            cjk := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/LXGWWenKaiBold.ttf") asc: 0.0 desc: 0.0}
            emoji := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/NotoColorEmoji.ttf") asc: 0.0 desc: 0.0}
        }}
        mod.theme.font_code = mod.theme.font_code{font_family: FontFamily{
            fira := FontMember{res: mod.decem_fonts.fira_code asc: 0.0 desc: 0.0}
            mono := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/LiberationMono-Regular.ttf") asc: 0.0 desc: 0.0}
            cjk := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/LXGWWenKaiRegular.ttf") asc: 0.0 desc: 0.0}
            emoji := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/NotoColorEmoji.ttf") asc: 0.0 desc: 0.0}
        }}
        // 网页版只在标题上用 Poppins（ChatInterface.tsx:117 的 h1、
        // SettingsPanel.tsx:89 的 h2），正文与强调仍是 Lato。
        mod.theme.font_display = mod.theme.font_bold{font_family: FontFamily{
            poppins := FontMember{res: mod.decem_fonts.poppins_semibold asc: 0.0 desc: 0.0}
            plex := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
            cjk := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/LXGWWenKaiBold.ttf") asc: 0.0 desc: 0.0}
            emoji := FontMember{res: mod.res.crate_resource("makepad_widgets:resources/NotoColorEmoji.ttf") asc: 0.0 desc: 0.0}
        }}
    });
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        // 深色沿用近黑 + 橙的 BlackOrange；浅色借 Windows 的浅色 sheet。
        // **必须装在任何 script_mod 之前**：`makepad_widgets::script_mod` 一开始就
        // 用当前 StyleSheet 求值 theme.splash（desktop_style.rs:281），注册完部件
        // 再补 widgets.splash（desktop_style.rs:297）。装晚了整棵树拿到的还是默认
        // 暗色主题，窗口 clear 色也就停在默认暗色的 #4C4C4C，而不是
        // black-orange 的 #14171c（widgets/themes/black-orange/widgets.splash:6）。
        //
        // 每次样式重载都会重跑这个函数，所以这里读 `theme_mode()` 而不是写死：
        // `desktop_style::handle_event` 装好的 sheet 不会被这里又盖回深色。
        desktop_style::install(vm, theme_stylesheet(theme_mode()));
        crate::makepad_widgets::script_mod(vm);
        // 网页版字体的接入点：必须在 `makepad_code_editor::script_mod` 之前，
        // CodeView 在那一刻就把 `theme.font_code` 抄进了自己的 TextStyle。
        install_web_fonts(vm);
        // 图标同理：嵌进二进制的 SVG 必须先于 `self::script_mod` 注册成资源
        install_icons(vm);
        // CodeView（带语法高亮的代码块）属于独立的 makepad-code-editor crate，
        // 必须先注册它的脚本部件，下面 script_mod! 里的 `use mod.widgets.CodeView` 才能解析
        makepad_code_editor::script_mod(vm);
        self::script_mod(vm)
    }

    fn after_new_from_script(_vm: &mut ScriptVm, app: &mut Self) {
        // 用户头像在部件树建立之前定下来：持久化文件优先于仓库默认图。
        // 读失败要可见（这里是唯一能安全拿 CHAT 写锁的时机，push_message 会重入死锁）。
        let (user_avatar, avatar_warning) = load_initial_user_avatar();
        *USER_AVATAR.write().unwrap() = Some(user_avatar);

        let mut chat = CHAT.write().unwrap();
        chat.messages.push(Msg {
            role: Role::Char,
            text: WELCOME.to_owned(),
            image: None,
            time: now_hhmm(),
        });
        if let Some(reason) = avatar_warning {
            chat.messages.push(Msg {
                role: Role::Char,
                text: format!("⚠️ {reason}"),
                image: None,
                time: now_hhmm(),
            });
        }
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
                    time: now_hhmm(),
                });
            }
        }
        drop(chat);
        let _ = app;
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.load_header_avatar(cx);
        self.load_startup_image(cx);
        self.load_startup_avatar(cx);
        // 光晕的 3s 呼吸：10 Hz 定时重绘。`Event::Startup` 有时在首帧前
        // 到不了（部件树还没建），所以 Draw 也当作启动信号，只启动一次。
        if self.glow_timer.is_none() && matches!(event, Event::Startup | Event::Draw(_)) {
            self.glow_timer = Some(cx.start_interval(0.1));
        }
        if self
            .glow_timer
            .is_some_and(|timer| timer.is_event(event).is_some())
        {
            cx.redraw_all();
        }
        self.match_event(cx, event);
        if let Event::Actions(actions) = event {
            self.handle_file_dialog(cx, actions);
        }
        self.ui.handle_event(cx, event, &mut Scope::empty());
        // 首帧的发送键状态：部件树真正建立之前拿不到按钮，所以一直试到建好为止。
        if !self.send_state_synced && !self.ui.widget(cx, ids!(send_button)).is_empty() {
            self.send_state_synced = true;
            // 空 Image 会占住 max_height，必须显式隐藏（迁移文档 §6 的坑）。
            // 但 DECEM_IMAGE 可能已经先一步把图放好了，别把它盖掉。
            if self.pending_image.is_none() {
                self.ui
                    .image(cx, ids!(pending_photo))
                    .set_visible(cx, false);
            }
            self.sync_send_enabled(cx);
        }
        // 抽屉里的当前模型文字：部件树建好后的第一帧同步一次。
        if !self.drawer_synced && !self.ui.widget(cx, ids!(model_trigger_label)).is_empty() {
            self.drawer_synced = true;
            self.sync_model_ui(cx);
            // 头像预览也在同一批部件里，建好就贴一次图
            self.sync_avatar_ui(cx);
            // 深浅色行的文字：脚本里的 `text` 只是空串，首帧由 Rust 写入
            self.sync_theme_ui(cx);
        }
        self.poll_worker(cx);
    }
}

script_mod! {
    use mod.prelude.widgets.*
    // 代码块用 makepad-code-editor 的 CodeView：自带语法高亮（默认 TokenColors 主题，
    // code_editor/src/code_editor.rs:26-48）。它不在 makepad-widgets 里，
    // AppMain::script_mod 已先调 makepad_code_editor::script_mod 注册。
    use mod.widgets.CodeView

    // ── 背景 + 光晕（规格 §1）
    // 三层光晕的尺寸、位置、透明度都照网页版 ChatInterface.tsx:75-99：
    // 珊瑚 384 在左 1/4 上 1/4、薄荷 320 在右 1/4 下 1/4、黄 256 在右 1/3 中。
    // 网页版 `animate-pulse-glow`（index.css:185-190）是 3s 的
    // opacity 0.3↔0.6 + scale 1↔1.1；这里把相位映射到 0..1 后做同样的事。
    // 三个 orb 的 alpha 峰值取「很淡」的 6% / 4.5% / 3%（orb opacity 与
    // 动画 opacity 相乘、再经 blur 之后的量级，规格 §1 与 §8-4）。
    let DrawBackdrop = set_type_default() do #(DrawBackdrop::script_shader(vm)) {
        ..mod.draw.DrawQuad
        bg: #x000000
        glow_time: 0.0

        // CSS ease-in-out 的三角波（0 → 1 → 0）
        pulse_at: fn(phase: float) -> float {
            let tri = 1.0 - abs(fract(phase) * 2.0 - 1.0)
            return tri * tri * (3.0 - 2.0 * tri)
        }

        // 一个 orb 的 alpha：峰值 × 呼吸 × 径向衰减。
        // t 用「到圆心距离 / 半径」归一化，scale 动画直接改半径。
        orb_alpha: fn(p: vec2, center: vec2, radius: float, peak: float, pulse: float) -> float {
            let t = length(p - center) / max(radius, 0.001)
            return peak * mix(0.5, 1.0, pulse) * exp(-2.7 * t * t)
        }

        pixel: fn() {
            let w = self.rect_size.x
            let h = self.rect_size.y
            let p = self.pos * self.rect_size
            let phase = self.glow_time / 3.0
            let coral = #xF38181
            let mint = #x95E1D3
            let yellow = #xFCE38A
            let mut col = self.bg.rgb

            // 珊瑚：左 1/4、上 1/4，直径 384
            let p1 = self.pulse_at(phase)
            let a1 = self.orb_alpha(
                p,
                vec2(w * 0.25 + 192.0, h * 0.25 + 192.0),
                192.0 * (1.0 + 0.1 * p1),
                0.06,
                p1
            )
            col = mix(col, coral.rgb, a1)

            // 薄荷：右 1/4、下 1/4，直径 320（网页版动画延迟 1.5s = 半周）
            let p2 = self.pulse_at(phase + 0.5)
            let a2 = self.orb_alpha(
                p,
                vec2(w * 0.75 - 160.0, h * 0.75 - 160.0),
                160.0 * (1.0 + 0.1 * p2),
                0.045,
                p2
            )
            col = mix(col, mint.rgb, a2)

            // 黄：右 1/3、垂直居中，直径 256（延迟 3s = 同相）
            let p3 = self.pulse_at(phase)
            let a3 = self.orb_alpha(
                p,
                vec2(w - w / 3.0 - 128.0, h * 0.5 + 128.0),
                128.0 * (1.0 + 0.1 * p3),
                0.03,
                p3
            )
            col = mix(col, yellow.rgb, a3)

            return vec4(col, 1.0)
        }
    }

    let Backdrop = #(Backdrop::register_widget(vm)) {
        width: Fill
        height: Fill
        flow: Down
        spacing: 0.
        padding: 0
        draw_glow: DrawBackdrop {
            bg: #(palette().bg)
        }
    }

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

    // 消息两侧的圆头像（规格 §3）：40×40、2px 环、图片裁成圆。
    // 环用 RoundedView 的描边；图片用 Image 自己的 `draw_bg.border_radius`
    // 裁圆（image.rs:289-296 与 207-224）。图片读不到时 Rust 侧把 Image 隐藏，
    // 只剩环 —— 不占位、不报错。
    let MessageAvatar = RoundedView {
        width: 40
        height: 40
        flow: Down
        align: Center
        padding: Inset{left: 2 top: 2 right: 2 bottom: 2}
        show_bg: true
        draw_bg.color: #x00000000
        draw_bg.border_size: 2.0
        draw_bg.border_radius: uniform(20.0)

        avatar_photo := Image {
            width: Fill
            height: Fill
            fit: ImageFit.CropToFill
            draw_bg.border_radius: 18.0
        }
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

            // Decem 一侧：头像在左、气泡在右，两者顶对齐（规格 §3）。
            // 第一行放头像与气泡，第二行放时间戳与「重新生成」。
            Char := View {
                width: Fill
                height: Fit
                flow: Down
                align: Align{x: 0.0}
                padding: Inset{top: 6 bottom: 6 left: 4 right: 64}

                View {
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{x: 0.0, y: 0.0}
                    spacing: 12.

                    MessageAvatar {
                        draw_bg.border_color: #x95E1D3
                    }

                    bubble := Bubble {
                        RoundedView {
                            width: Fill
                            height: Fit
                            show_bg: true
                            draw_bg.color: #(palette().bubble)
                            draw_bg.border_size: 1.0
                            draw_bg.border_color: #(palette().bubble_border)
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
                                // 代码块外框：网页版 pre 是 rounded-lg（12px）+ p-3（12px）
                                // （MessageBubble.tsx:105）。底色画在这一层，因为 CodeEditor 的
                                // 底色是 DrawColor 直角色块（code_editor.rs:250），自己给不出圆角。
                                code_block := RoundedView {
                                    width: Fill
                                    height: Fit
                                    show_bg: true
                                    draw_bg.color: #x1a1b26
                                    draw_bg.border_radius: uniform(12.)
                                    // 12 的内距照网页版；CodeView 的 `pad_left_top` 在
                                    // CodeViewBase 里已经是 0（code_view.rs:11-12），不会叠加。
                                    padding: Inset{left: 12 top: 12 right: 12 bottom: 12}

                                    // CodeView 的 walk 取自 editor 字段（code_view.rs:60-62），
                                    // 宽高与底色都要在 editor +: 里给，实例上写 width/height 会运行期报错。
                                    code_view := CodeView {
                                        // 聊天里的代码块只读：光标不粘末尾、不响应输入
                                        keep_cursor_at_end: false
                                        editor +: {
                                            height: Fit
                                            // 只读（code_editor.rs:293）；行号 gutter 关掉（code_editor.rs:295），与网页版一致
                                            read_only: true
                                            show_gutter: false
                                            // 全透明：底色交给上面的 RoundedView，圆角才露得出来
                                            draw_bg +: { color: #x00000000 }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // 时间戳在气泡**下方**（规格 §3）。左缩进 52 = 头像 40 + 间距 12，
                // 让时间戳与气泡左缘对齐；char 侧与「重新生成」同行。
                View {
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 8.
                    align: Align{x: 0.0, y: 0.5}
                    padding: Inset{left: 52 top: 4}

                    time := Label {
                        width: Fit
                        height: Fit
                        // 去掉 Label 默认内距，时间戳才与气泡边缘对齐
                        padding: 0
                        text: ""
                        draw_text +: {
                            color: #(palette().muted_fg)
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }

                    // 「重新生成」：最后一条 char 气泡底部的文字按钮。
                    // 只在最后一条 char 消息上可见（流式时隐藏），
                    // 显隐由 ChatList::draw_walk 按项控制。
                    regen_button := glass.GlassButton {
                        // 规格 §3：12px 珊瑚文字链接，无边框、无底色。
                        // ↻ 代替网页版的 RotateCcw 图标（12px）；`GlassButton`
                        // 是 `ChatList` 里 `glass_button(...)` 查找的类型，不能换。
                        text: "↻ 重新生成"
                        width: Fit
                        height: 18
                        padding: 0
                        // 玻璃层整体关掉：`layer_opacity` 乘在所有输出上
                        // （gauss_view.rs:808），底、边、阴影一并消失。
                        // 只留下文字层。
                        draw_glass +: {
                            layer_opacity: 0.0
                            surface_alpha: 0.0
                            shadow_alpha: 0.0
                            border_width: 0.0
                        }
                        // GlassButton 的默认文字是白的（glass_panel.rs:252），
                        // 浅色模式下白字压在白底上会看不见；珊瑚在两种模式都清楚。
                        draw_text +: {
                            color: #xF38181
                            text_style: theme.font_regular{font_size: 12}
                        }
                        // 悬停只轻微收一点透明度（网页版是换 #e86a6a；
                        // 这里拿不到 hover 颜色通道，退一步做可感知的反馈）
                        hover_ink: 0.92
                        down_ink: 0.72
                    }
                }
            }

            // 用户一侧：右对齐、珊瑚气泡。网页版是 135° 渐变 #F38181 → #e86a6a，
            // 而 Makepad 的 RoundedView 只有轴对齐双色渐变（`color_2` +
            // `gradient_fill_horizontal`，widgets/core/src/view_ui.rs:351-372）：
            // 默认按 pos.y 竖直铺，取它作 135° 的近似，方向仍是左上 → 右下。
            User := View {
                width: Fill
                height: Fit
                flow: Down
                align: Align{x: 1.0}
                padding: Inset{top: 6 bottom: 6 left: 64 right: 4}

                View {
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{x: 1.0, y: 0.0}
                    spacing: 12.

                    bubble := Bubble {
                        RoundedView {
                            width: Fill
                            height: Fit
                            flow: Down
                            show_bg: true
                            draw_bg.color: #xF38181
                            draw_bg.color_2: #xe86a6a
                            draw_bg.border_radius: uniform(16.)
                            padding: Inset{left: 14 top: 10 right: 14 bottom: 10}

                            // 用户发的图。没有纹理时高度收成 0、不占位。
                            // 宽度必须是 Fill：`Image` 在 Fit 宽度下会占住位置却不画纹理。
                            // 另外 `Image` **没有 align 属性**，写了会在运行时脚本报错（编译期不报），
                            // 所以图是左对齐的，特别竖长的图右侧会留白。
                            photo := Image {
                                width: Fill
                                height: Fit
                                max_height: FitBound.Abs(320)
                                fit: ImageFit.Smallest
                            }

                            selectable := Markdown {
                                width: Fill
                                height: Fit
                                body: ""
                                // 浅色主题的正文色是深色；用户气泡是珊瑚底，必须钉白字。
                                font_color: #xffffffff
                                draw_text +: { color: #xffffffff }
                            }
                        }
                    }

                    MessageAvatar {
                        draw_bg.border_color: #xF38181
                    }
                }

                // 用户侧只有时间戳（没有「重新生成」）。右缩进 52 与头像列对齐，
                // 让时间戳贴住气泡右缘。
                View {
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{x: 1.0, y: 0.5}
                    padding: Inset{right: 52 top: 4}

                    time := Label {
                        width: Fit
                        height: Fit
                        // 去掉 Label 默认内距，时间戳才与气泡边缘对齐
                        padding: 0
                        text: ""
                        draw_text +: {
                            color: #(palette().muted_fg)
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                }
            }
        }
    }

    // 设置抽屉里的模型行（规格 §6）。列表由 ModelList 逐项驱动，
    // 名字/描述/徽章都来自 decem_core::AVAILABLE_MODELS，UI 不另存一份。
    let ModelRow = #(ModelRow::register_widget(vm)) {
        width: Fill
        height: 64
        flow: Overlay
        align: Align{x: 0.0, y: 0.0}
        // 让行根 View 参与命中并上报 hover/click；Makepad 的 View 只有
        // 设置了 cursor 或 animator 才会走自己的命中分支（view.rs:1350）。
        cursor: MouseCursor.Hand

        // 选中/悬停底色单独一层：裸 View 的 draw_bg 没有 color，
        // RoundedView 的实例色又没有 Rust 侧 setter，显隐切换最省事。
        row_hl := RoundedView {
            width: Fill
            height: Fill
            visible: false
            show_bg: true
            draw_bg.color: #x8FD4E5
            draw_bg.border_radius: uniform(8.)
        }

        View {
            width: Fill
            height: Fill
            flow: Down
            spacing: 2.
            padding: Inset{left: 8 top: 8 right: 8 bottom: 8}

            // 上行：模型名 +（支持视觉时）薄荷徽章
            View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 6.
                align: Align{y: 0.5}

                model_name := Label {
                    width: Fit
                    height: Fit
                    padding: 0
                    text: ""
                    draw_text +: {
                        color: #(palette().fg)
                        text_style: theme.font_regular{font_size: 14}
                    }
                }

                vision_badge := RoundedView {
                    width: Fit
                    height: Fit
                    flow: Right
                    spacing: 3.
                    align: Align{y: 0.5}
                    padding: Inset{left: 6 top: 2 right: 6 bottom: 2}
                    show_bg: true
                    // rgba(149,225,211,0.2)
                    draw_bg.color: #x95E1D333
                    draw_bg.border_radius: uniform(99.)

                    Label {
                        width: Fit
                        height: Fit
                        padding: 0
                        // U+F06E = FontAwesome 的 fa-eye，单色、跟随徽章的薄荷色
                        text: "\u{f06e}"
                        draw_text +: {
                            color: #x4DB6AC
                            text_style: theme.font_icons{font_size: 11}
                        }
                    }

                    Label {
                        width: Fit
                        height: Fit
                        padding: 0
                        text: "视觉理解"
                        draw_text +: {
                            color: #x4DB6AC
                            text_style: theme.font_regular{font_size: 10}
                        }
                    }
                }
            }

            // 下行：12px 描述，超长省略
            model_desc := Label {
                width: Fill
                height: Fit
                padding: 0
                max_lines: 1
                text_overflow: Ellipsis
                text: ""
                draw_text +: {
                    color: #(palette().muted_fg)
                    text_style: theme.font_regular{font_size: 12}
                }
            }
        }

        // 右侧对勾（仅选中项显示；颜色固定 #B3B3B3）
        View {
            width: Fill
            height: Fill
            flow: Right
            align: Align{x: 1.0, y: 0.5}
            padding: Inset{right: 10}

            check := Label {
                width: 14
                height: 14
                padding: 0
                visible: false
                text: "✓"
                draw_text +: {
                    color: #xB3B3B3
                    text_style: theme.font_regular{font_size: 14}
                }
            }
        }
    }

    let ModelList = #(ModelList::register_widget(vm)) {
        width: Fill
        height: Fit

        model_rows := PortalList {
            width: Fill
            height: 320
            flow: Down
            reuse_items: false

            Row := ModelRow {}
        }
    }

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Decem Chat"
                window.inner_size: vec2(980, 760)
                // 窗口 render pass 的 clear 色默认取 theme.color_bg_app
                // （widgets/core/src/window.rs:154）；这里按当前模式钉成根背景色，
                // 保证窗口没有任何一条边缝漏出主题灰。
                pass +: { clear_color: #(palette().bg) }
                body +: {
                    // 背景与光晕都在 Backdrop 自己的着色器里（见 main.rs 的
                    // DrawBackdrop）：它先画底色 + 三层 orb，再把 walk 交给
                    // 内部 View。因此光晕天然位于全部内容之下，不参与命中。
                    Backdrop{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 0.
                        padding: 0

                        // ── 顶栏（规格 §2，高 65 = 64 内容 + 1 底边）
                        // 全宽；玻璃底只用半透明色模拟（Makepad 没有 backdrop blur）。
                        View {
                            width: Fill
                            height: Fit
                            flow: Down

                            RoundedView {
                                width: Fill
                                height: 64
                                flow: Right
                                align: Align{y: 0.5}
                                padding: Inset{left: 16 top: 12 right: 16 bottom: 12}
                                show_bg: true
                                // 深色 rgba(26,26,26,0.8) / 浅色 rgba(255,255,255,0.8)
                                draw_bg.color: #(palette().glass)
                                draw_bg.border_radius: uniform(0.0)

                                View {
                                    width: Fit
                                    height: Fit
                                    flow: Right
                                    spacing: 12.
                                    align: Align{y: 0.5}

                                    // 40×40 头像 + 2px 薄荷环 + 右下角在线点
                                    View {
                                        width: 40
                                        height: 40
                                        flow: Overlay
                                        align: Align{x: 0.0, y: 0.0}
                                        // 在线点按网页版 `-bottom-0.5 -right-0.5` 溢出头像
                                        // 右/下各 2px，容器不能裁。
                                        clip_x: false
                                        clip_y: false

                                        RoundedView {
                                            width: 40
                                            height: 40
                                            flow: Down
                                            align: Center
                                            padding: Inset{left: 2 top: 2 right: 2 bottom: 2}
                                            show_bg: true
                                            draw_bg.color: #x00000000
                                            draw_bg.border_size: 2.0
                                            draw_bg.border_color: #x95E1D3
                                            draw_bg.border_radius: uniform(20.0)

                                            header_photo := Image {
                                                width: Fill
                                                height: Fill
                                                fit: ImageFit.CropToFill
                                                draw_bg.border_radius: 18.0
                                            }
                                        }

                                        // 12×12 在线绿 + 2px 背景色描边（规格 §2）。
                                        // 踩过两次：Overlay 里的 `margin` 不保证偏移；`abs_pos` 是
                                        // **窗口绝对坐标**（会把点甩到窗口左上角）。所以用一个
                                        // 与头像同尺寸的定位层，靠 `padding` 把点推到右下。
                                        View {
                                            width: 40
                                            height: 40
                                            flow: Down
                                            padding: Inset{left: 30 top: 30}
                                            clip_x: false
                                            clip_y: false

                                            RoundedView {
                                                width: 12
                                                height: 12
                                                show_bg: true
                                                draw_bg.color: #x22C55E
                                                draw_bg.border_size: 2.0
                                                draw_bg.border_color: #(palette().bg)
                                                draw_bg.border_radius: uniform(6.0)
                                            }
                                        }
                                    }

                                    // 标题行 + 副标题
                                    View {
                                        width: Fit
                                        height: Fit
                                        flow: Down
                                        spacing: 2.

                                        View {
                                            width: Fit
                                            height: Fit
                                            flow: Right
                                            spacing: 8.
                                            align: Align{y: 0.5}

                                            Label {
                                                width: Fit
                                                height: Fit
                                                // Label 默认 padding = theme.mspace_1；顶栏文字要贴边，
                                                // 否则两行文字会被撑高、把 40px 头像挤出 64px 的栏
                                                padding: 0
                                                text: "Decem"
                                                draw_text +: {
                                                    color: #(palette().fg)
                                                    text_style: theme.font_display{font_size: 18}
                                                }
                                            }

                                            // lucide `sparkles`（网页版 ChatInterface.tsx:119 的
                                            // `<Sparkles className="w-4 h-4">`）：16px、#FCE38A。
                                            // SVG 取自 lucide 0.562.0，与网页端依赖同版本。
                                            Icon {
                                                width: Fit
                                                height: Fit
                                                size: 16
                                                draw_icon +: {
                                                    svg: mod.decem_icons.sparkles
                                                    color: #xFCE38A
                                                }
                                            }
                                        }

                                        Label {
                                            width: Fit
                                            height: Fit
                                            padding: 0
                                            text: "在线 - 随时陪你聊天"
                                            draw_text +: {
                                                color: #(palette().muted_fg)
                                                text_style: theme.font_regular{font_size: 12}
                                            }
                                        }
                                    }
                                }

                                // 撑开，把右侧按钮推到最右
                                View { width: Fill height: 1 }

                                View {
                                    width: Fit
                                    height: Fit
                                    flow: Right
                                    spacing: 8.
                                    align: Align{y: 0.5}

                                    // 两个按钮的图标：lucide 0.562.0 的 `github` / `settings`
                                    // 原图（网页版 ChatInterface.tsx:135/144，20px）。
                                    // ButtonFlat 自带 `draw_icon` 槽位，图标与文字共用命中区，
                                    // 不需要额外叠一层（叠层会抢走点击）。
                                    github_button := glass.Button {
                                        text: ""
                                        width: 40
                                        height: 40
                                        padding: 0
                                        icon_walk: Walk{width: 20.}
                                        draw_icon +: {
                                            svg: mod.decem_icons.github
                                            color: #(palette().fg)
                                        }
                                        draw_bg +: {
                                            border_size: 0.0
                                            border_radius: 16.0
                                            color: #x00000000
                                            color_hover: #(palette().hover)
                                            color_down: #(palette().hover_down)
                                            border_color: #x00000000
                                            border_color_hover: #x00000000
                                            border_color_down: #x00000000
                                            top_glint: #x00000000
                                            inner_shadow: #x00000000
                                        }
                                    }

                                    settings_button := glass.Button {
                                        // lucide 的描边齿轮（不是 FontAwesome 的实心 fa-gear）。
                                        text: ""
                                        width: 40
                                        height: 40
                                        padding: 0
                                        icon_walk: Walk{width: 20.}
                                        draw_icon +: {
                                            svg: mod.decem_icons.settings
                                            color: #(palette().fg)
                                        }
                                        draw_bg +: {
                                            border_size: 0.0
                                            border_radius: 16.0
                                            color: #x00000000
                                            color_hover: #(palette().hover)
                                            color_down: #(palette().hover_down)
                                            border_color: #x00000000
                                            border_color_hover: #x00000000
                                            border_color_down: #x00000000
                                            top_glint: #x00000000
                                            inner_shadow: #x00000000
                                        }
                                    }
                                }
                            }

                            // 底部 1px 边框（整框描边会四边都画，所以单独一条）
                            RectView {
                                width: Fill
                                height: 1
                                show_bg: true
                                draw_bg.color: #(palette().border)
                            }
                        }

                        // 消息列与输入区的外层。输入区要全宽（规格 §1），
                        // 所以内边距只给消息列，输入区自己带 12 的内距。
                        View {
                            width: Fill
                            height: Fill
                            flow: Down
                            spacing: 0
                            padding: 0

                        // 消息列容器
                        View {
                            width: Fill
                            height: Fill
                            flow: Down
                            padding: Inset{left: 14 top: 10 right: 14 bottom: 10}

                        // 消息列：网页版是「内容列 768 居中，顶栏与输入区全宽」。
                        // 居中只能靠父层的 `align`（`Walk.align` 在布局里不生效，
                        // 起作用的是 turtle 的 `Layout.align`，见 draw/src/turtle.rs:3097），
                        // 所以在中间套一层全宽容器。ChatList 仍是 Fill——窄窗口不溢出，
                        // 只是用 `max_width` 把宽窗口下的消息列收到 768（与 `Image` 的
                        // `max_height: FitBound.Abs(…)` 同一个写法）。
                        View {
                            width: Fill
                            height: Fill
                            align: Align{x: 0.5}

                            chat_list := ChatList {
                                max_width: FitBound.Abs(768)
                            }
                        }

                        } // 消息列容器

                        // ── 输入区（规格 §4）：全宽、顶部 1px 边框、玻璃底、内距 12
                        View {
                            width: Fill
                            height: Fit
                            flow: Down

                            // 顶部 1px 边框（整框描边会四边都画，所以单独一条）
                            RectView {
                                width: Fill
                                height: 1
                                show_bg: true
                                draw_bg.color: #(palette().border)
                            }

                            RoundedView {
                                width: Fill
                                height: Fit
                                flow: Down
                                padding: Inset{left: 12 top: 12 right: 12 bottom: 12}
                                show_bg: true
                                // 深色 rgba(20,20,20,0.8) / 浅色背景色 80%
                                draw_bg.color: #(palette().input_bg)
                                draw_bg.border_radius: uniform(0.)

                                // 待发图片的预览（规格 §4：80 高、圆角 12）。
                                // **宽度不能是 Fit**：Fit 宽度下 Image 会占住位置却不画纹理
                                // （用户气泡里的图也踩过，见 §12）。没有图片时显隐由 Rust 侧管。
                                pending_photo := Image {
                                    width: 120
                                    height: 80
                                    fit: ImageFit.Smallest
                                    draw_bg.border_radius: 12.0
                                }

                                View{
                                    width: Fill
                                    height: Fit
                                    flow: Right
                                    spacing: 8.
                                    align: Align{y: 0.5}

                                    // 胶囊：56 高 = 40 内容 + 上下各 8 内距。
                                    // 内部一条 Right 行：[fa-image] [input Fill] [↑]（规格 §4）。
                                    // 胶囊外的 Overlay 只用来叠聚焦描边。
                                    pill := View {
                                        width: Fill
                                        height: 56
                                        flow: Overlay
                                        align: Align{x: 0.0, y: 0.0}

                                        RoundedView {
                                            width: Fill
                                            height: Fill
                                            flow: Right
                                            spacing: 8.
                                            align: Align{y: 0.5}
                                            padding: Inset{left: 8 top: 8 right: 8 bottom: 8}
                                            show_bg: true
                                            // 深色 rgba(38,38,38,0.5) / 浅色 muted/50
                                            draw_bg.color: #(palette().row_bg)
                                            draw_bg.border_radius: uniform(16.)

                                            // 图片按钮在胶囊内部、输入框左侧（规格 §4）。
                                            // 按钮不是 Image，进输入行不会经 NaN 弄坏
                                            // TextInput 的命中区（迁移文档 §12 只针对 Image）。
                                            // 图标：lucide 0.562.0 的 `image` 原图（网页版
                                            // ChatInput.tsx 的 `<Image className="w-5 h-5">`，20px）。
                                            attach_button := glass.Button {
                                                text: ""
                                                width: 40
                                                height: 40
                                                padding: 0
                                                icon_walk: Walk{width: 20.}
                                                draw_icon +: {
                                                    svg: mod.decem_icons.image
                                                    color: #(palette().fg)
                                                }
                                                draw_bg +: {
                                                    border_size: 0.0
                                                    border_radius: uniform(16.0)
                                                    color: #x00000000
                                                    color_hover: #xF381811A
                                                    color_down: #xF381812E
                                                    border_color: #x00000000
                                                    border_color_hover: #x00000000
                                                    border_color_down: #x00000000
                                                    top_glint: #x00000000
                                                    inner_shadow: #x00000000
                                                }
                                            }

                                            // 14px、透明底、无边框；单行输入由 TextInput
                                            // 自己把字形垂直居中（text_input.rs:1717-1740）
                                            input := glass.TextInput {
                                                width: Fill
                                                height: Fill
                                                padding: Inset{left: 8 right: 8 top: 10 bottom: 10}
                                                empty_text: "发个消息聊聊呗～（Enter 发送，Esc 中断）"
                                                draw_bg +: {
                                                    border_size: 0.0
                                                    border_radius: uniform(8.)
                                                    color: #x00000000
                                                    color_hover: #x00000000
                                                    color_focus: #x00000000
                                                    color_down: #x00000000
                                                    color_empty: #x00000000
                                                    border_color: #x00000000
                                                    border_color_hover: #x00000000
                                                    border_color_focus: #x00000000
                                                    border_color_down: #x00000000
                                                    border_color_empty: #x00000000
                                                }
                                                draw_text +: {
                                                    color: #(palette().fg)
                                                    color_hover: #(palette().fg)
                                                    color_focus: #(palette().fg)
                                                    color_empty: #(palette().muted_fg)
                                                    color_empty_hover: #(palette().muted_fg)
                                                    color_empty_focus: #(palette().muted_fg)
                                                    text_style: theme.font_regular{font_size: 14}
                                                }
                                            }

                                            // 40×40、圆角 16 的珊瑚发送键。
                                            // 图标：lucide 0.562.0 的 `send` 原图（网页版
                                            // ChatInput.tsx 的 `<Send className="w-4 h-4">`，16px）。
                                            // 必须是 ButtonFlat 而不是 glass.Button：后者的
                                            // 裁剪版 draw_bg（glass_panel.rs:841-878）不混合
                                            // color_disabled，禁用态只有文字会变灰。
                                            // 图标是单色 uniform，没有 color_disabled，由
                                            // `sync_send_enabled` 直接改 `draw_icon.color`。
                                            send_button := ButtonFlat {
                                                text: ""
                                                width: 40
                                                height: 40
                                                padding: 0
                                                icon_walk: Walk{width: 16.}
                                                draw_icon +: {
                                                    svg: mod.decem_icons.send
                                                    color: #xffffffff
                                                }
                                                draw_bg +: {
                                                    border_size: 0.0
                                                    border_radius: uniform(16.0)
                                                    color: #xF38181
                                                    color_hover: #xe86a6a
                                                    color_down: #xe86a6a
                                                    color_focus: #xF38181
                                                    // 禁用：muted 底 + 半透明，近似网页版的 opacity .5
                                                    color_disabled: #(palette().disabled_bg)
                                                    border_color: #x00000000
                                                    border_color_hover: #x00000000
                                                    border_color_down: #x00000000
                                                    border_color_disabled: #x00000000
                                                }
                                            }
                                        }

                                        // 聚焦外描边的近似：Makepad 没有 box-shadow，
                                        // 用一圈 2px 内描边代替（不占布局）。
                                        // 显隐由 Rust 侧的 TextInputAction::KeyFocus /
                                        // KeyFocusLost 驱动（main.rs::sync_input_focus）。
                                        focus_ring := RoundedView {
                                            width: Fill
                                            height: Fill
                                            visible: false
                                            draw_bg.color: #x00000000
                                            draw_bg.border_size: 2.0
                                            draw_bg.border_color: #xF381814D
                                            draw_bg.border_radius: uniform(16.)
                                        }
                                    }
                                }

                                // 底部提示（规格 §4）
                                Label {
                                    width: Fill
                                    height: Fit
                                    padding: Inset{top: 8}
                                    align: Align{x: 0.5}
                                    text: "按 Enter 发送，Shift + Enter 换行"
                                    draw_text +: {
                                        color: #(palette().muted_fg)
                                        text_style: theme.font_regular{font_size: 12}
                                    }
                                }
                            }
                        }
                        } // 消息列 + 输入区的外层

                        // ── 设置抽屉（规格 §5；本轮只做「切换模型」）
                        // 用 Modal 做外壳：它是 overlay（on_after_apply 把 Walk 钉成 empty），
                        // 不占 body 布局；遮罩、点遮罩关闭、Esc 关闭都直接复用。
                        // 贴右：Modal 根部 flow 是 Overlay，align x:1.0 即右对齐；
                        // content 固定 320 宽、全高，点在它左侧的遮罩上仍会关闭。
                        settings_drawer := Modal {
                            align: Align{x: 1.0, y: 0.0}
                            bg_view +: {
                                draw_bg +: {
                                    // 深色 rgba(0,0,0,0.5) / 浅色 rgba(0,0,0,0.3)
                                    color: #(palette().mask)
                                }
                            }
                            content +: {
                                width: 320
                                height: Fill
                                flow: Down

                                drawer_panel := RoundedView {
                                    width: Fill
                                    height: Fill
                                    flow: Down
                                    show_bg: true
                                    // 深色 rgba(26,26,26,0.8) / 浅色 rgba(255,255,255,0.8)
                                    draw_bg.color: #(palette().glass)
                                    draw_bg.border_size: 1.0
                                    draw_bg.border_color: #(palette().glass_border)
                                    draw_bg.border_radius: uniform(0.)

                                    // 头部：标题 18px + 关闭按钮（§5）
                                    RoundedView {
                                        width: Fill
                                        height: 56
                                        flow: Right
                                        align: Align{y: 0.5}
                                        padding: Inset{left: 16 right: 16 top: 12 bottom: 12}
                                        show_bg: true
                                        draw_bg.color: #x00000000
                                        draw_bg.border_radius: uniform(0.)

                                        Label {
                                            width: Fit
                                            height: Fit
                                            padding: 0
                                            text: "设置"
                                            draw_text +: {
                                                color: #(palette().fg)
                                                text_style: theme.font_display{font_size: 18}
                                            }
                                        }

                                        View { width: Fill height: 1 }

                                        drawer_close_button := glass.Button {
                                            text: "×"
                                            width: 28
                                            height: 28
                                            padding: 0
                                            draw_text +: {
                                                color: #(palette().muted_fg)
                                                color_hover: #(palette().fg)
                                                text_style: theme.font_regular{font_size: 18}
                                            }
                                            draw_bg +: {
                                                border_size: 0.0
                                                border_radius: uniform(12.0)
                                                color: #x00000000
                                                color_hover: #(palette().hover)
                                                color_down: #(palette().hover_down)
                                                border_color: #x00000000
                                                border_color_hover: #x00000000
                                                border_color_down: #x00000000
                                                top_glint: #x00000000
                                                inner_shadow: #x00000000
                                            }
                                        }
                                    }

                                    // 头部底边 1px（§5）
                                    RectView {
                                        width: Fill
                                        height: 1
                                        show_bg: true
                                        draw_bg.color: #(palette().border)
                                    }

                                    // 内容区：内距 16、段间 24
                                    View {
                                        width: Fill
                                        height: Fill
                                        flow: Down
                                        spacing: 24.
                                        padding: Inset{left: 16 top: 16 right: 16 bottom: 16}

                                        // ── 第 1 段：切换模型（§5-1 / §6）
                                        View {
                                            width: Fill
                                            height: Fit
                                            flow: Down
                                            spacing: 8.

                                            View {
                                                width: Fill
                                                height: Fit
                                                flow: Right
                                                spacing: 8.
                                                align: Align{y: 0.5}

                                                // 图标：lucide 0.562.0 的 `bot` 原图（网页版切换模型
                                                // 用的是 lucide `Bot`，不是 FontAwesome 的 shuffle）。
                                                Icon {
                                                    width: Fit
                                                    height: Fit
                                                    size: 16
                                                    draw_icon +: {
                                                        svg: mod.decem_icons.bot
                                                        color: #xF38181
                                                    }
                                                }

                                                Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: "切换模型"
                                                    draw_text +: {
                                                        color: #(palette().fg)
                                                        text_style: theme.font_regular{font_size: 14}
                                                    }
                                                }
                                            }

                                            // 触发器：36 高、圆角 10、1px 边框、底 muted/50
                                            RoundedView {
                                                width: Fill
                                                height: 36
                                                flow: Overlay
                                                show_bg: true
                                                draw_bg.color: #(palette().row_bg)
                                                draw_bg.border_size: 1.0
                                                draw_bg.border_color: #(palette().glass_border)
                                                draw_bg.border_radius: uniform(10.)

                                                View {
                                                    width: Fill
                                                    height: Fill
                                                    flow: Right
                                                    spacing: 8.
                                                    align: Align{y: 0.5}
                                                    padding: Inset{left: 10 right: 10}

                                                    model_trigger_label := Label {
                                                        width: Fill
                                                        height: Fit
                                                        padding: 0
                                                        max_lines: 1
                                                        text_overflow: Ellipsis
                                                        text: "Qwen3.8-Flash"
                                                        draw_text +: {
                                                            color: #(palette().fg)
                                                            text_style: theme.font_regular{font_size: 14}
                                                        }
                                                    }

                                                    Label {
                                                        width: Fit
                                                        height: Fit
                                                        padding: 0
                                                        text: "▼"
                                                        draw_text +: {
                                                            color: #(palette().muted_fg)
                                                            text_style: theme.font_regular{font_size: 14}
                                                        }
                                                    }
                                                }

                                                // 透明按钮盖在触发器上：单击展开/收起。
                                                // 文字留给上面的 Label，按钮只负责命中与 hover 亮底。
                                                model_trigger_button := ButtonFlat {
                                                    width: Fill
                                                    height: Fill
                                                    margin: 0
                                                    padding: 0
                                                    text: ""
                                                    draw_text +: {
                                                        color: #x00000000
                                                        color_hover: #x00000000
                                                        color_down: #x00000000
                                                        color_focus: #x00000000
                                                        color_disabled: #x00000000
                                                    }
                                                    draw_bg +: {
                                                        border_size: 0.0
                                                        border_radius: uniform(10.)
                                                        color: #x00000000
                                                        color_hover: #(palette().tint_hover)
                                                        color_down: #(palette().tint_down)
                                                        color_focus: #x00000000
                                                        color_disabled: #x00000000
                                                        color_2: #x00000000
                                                        color_2_hover: #x00000000
                                                        color_2_down: #x00000000
                                                        color_2_focus: #x00000000
                                                        color_2_disabled: #x00000000
                                                        border_color: #x00000000
                                                        border_color_hover: #x00000000
                                                        border_color_down: #x00000000
                                                        border_color_focus: #x00000000
                                                        border_color_disabled: #x00000000
                                                    }
                                                }
                                            }

                                            // 模型列表（§6）：容器底 card、圆角 10、1px border。
                                            // 显隐与高度由 Rust 侧 `set_models_open` 管：
                                            // visible:false 在 Makepad 里仍占布局，必须同时把高度压成 0。
                                            model_list_box := RoundedView {
                                                width: Fill
                                                height: Fit
                                                visible: false
                                                flow: Down
                                                padding: Inset{left: 0 top: 4 right: 0 bottom: 4}
                                                show_bg: true
                                                draw_bg.color: #(palette().model_list_bg)
                                                draw_bg.border_size: 1.0
                                                draw_bg.border_color: #(palette().border)
                                                draw_bg.border_radius: uniform(10.)

                                                model_list := ModelList {
                                                    width: Fill
                                                }
                                            }

                                            // 列表下方：「当前: X」+ 视觉理解徽章
                                            View {
                                                width: Fill
                                                height: Fit
                                                flow: Right
                                                spacing: 6.
                                                align: Align{y: 0.5}

                                                model_current_label := Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: "当前: Qwen3.8-Flash"
                                                    draw_text +: {
                                                        color: #(palette().muted_fg)
                                                        text_style: theme.font_regular{font_size: 12}
                                                    }
                                                }

                                                current_badge := RoundedView {
                                                    width: Fit
                                                    height: Fit
                                                    flow: Right
                                                    spacing: 3.
                                                    align: Align{y: 0.5}
                                                    padding: Inset{left: 6 top: 2 right: 6 bottom: 2}
                                                    show_bg: true
                                                    draw_bg.color: #x95E1D333
                                                    draw_bg.border_radius: uniform(99.)

                                                    Label {
                                                        width: Fit
                                                        height: Fit
                                                        padding: 0
                                                        // U+F06E = FontAwesome 的 fa-eye（同上，单色）
                                                        text: "\u{f06e}"
                                                        draw_text +: {
                                                            color: #x4DB6AC
                                                            text_style: theme.font_icons{font_size: 11}
                                                        }
                                                    }

                                                    Label {
                                                        width: Fit
                                                        height: Fit
                                                        padding: 0
                                                        text: "视觉理解"
                                                        draw_text +: {
                                                            color: #x4DB6AC
                                                            text_style: theme.font_regular{font_size: 10}
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // 段间分隔线（§5）
                                        RectView {
                                            width: Fill
                                            height: 1
                                            show_bg: true
                                            draw_bg.color: #(palette().border)
                                        }

                                        // ── 第 2 段：更改头像（§5-2）
                                        View {
                                            width: Fill
                                            height: Fit
                                            flow: Down
                                            spacing: 8.

                                            // 分区标题：16px 薄荷图标 + 14px 文字
                                            View {
                                                width: Fill
                                                height: Fit
                                                flow: Right
                                                spacing: 8.
                                                align: Align{y: 0.5}

                                                // U+F007 = FontAwesome 的 fa-user，单色、跟随薄荷色
                                                Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: "\u{f007}"
                                                    draw_text +: {
                                                        color: #x95E1D3
                                                        text_style: theme.font_icons{font_size: 16}
                                                    }
                                                }

                                                Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: "更改头像"
                                                    draw_text +: {
                                                        color: #(palette().fg)
                                                        text_style: theme.font_regular{font_size: 14}
                                                    }
                                                }
                                            }

                                            View {
                                                width: Fill
                                                height: Fit
                                                flow: Right
                                                spacing: 16.
                                                align: Align{y: 0.5}

                                                // 预览：64×64 圆形、2px 珊瑚边框。
                                                // 环用 RoundedView 描边，图用 Image 自己的
                                                // border_radius 裁圆（同消息头像）。
                                                RoundedView {
                                                    width: 64
                                                    height: 64
                                                    flow: Down
                                                    align: Center
                                                    padding: Inset{left: 2 top: 2 right: 2 bottom: 2}
                                                    show_bg: true
                                                    draw_bg.color: #x00000000
                                                    draw_bg.border_size: 2.0
                                                    draw_bg.border_color: #xF38181
                                                    draw_bg.border_radius: uniform(32.0)

                                                    // 没有图时由 Rust 侧隐藏，只剩环
                                                    avatar_preview_photo := Image {
                                                        width: Fill
                                                        height: Fill
                                                        fit: ImageFit.CropToFill
                                                        draw_bg.border_radius: 30.0
                                                    }
                                                }

                                                // 上传按钮：32 高、圆角 10、1px 边框、
                                                // 16px 图标 + 8px 间距。文字与图标用 Label 拼，
                                                // 透明 ButtonFlat 盖在上面只管命中（同模型触发器）。
                                                RoundedView {
                                                    width: Fill
                                                    height: 32
                                                    flow: Overlay
                                                    show_bg: true
                                                    draw_bg.color: #x00000000
                                                    draw_bg.border_size: 1.0
                                                    draw_bg.border_color: #(palette().glass_border)
                                                    draw_bg.border_radius: uniform(10.)

                                                    View {
                                                        width: Fill
                                                        height: Fill
                                                        flow: Right
                                                        spacing: 8.
                                                        align: Align{x: 0.5, y: 0.5}

                                                        Label {
                                                            width: Fit
                                                            height: Fit
                                                            padding: 0
                                                            text: "↑"
                                                            draw_text +: {
                                                                color: #(palette().fg)
                                                                text_style: theme.font_regular{font_size: 16}
                                                            }
                                                        }

                                                        Label {
                                                            width: Fit
                                                            height: Fit
                                                            padding: 0
                                                            text: "上传头像"
                                                            draw_text +: {
                                                                color: #(palette().fg)
                                                                text_style: theme.font_regular{font_size: 14}
                                                            }
                                                        }
                                                    }

                                                    upload_avatar_button := ButtonFlat {
                                                        width: Fill
                                                        height: Fill
                                                        margin: 0
                                                        padding: 0
                                                        text: ""
                                                        draw_text +: {
                                                            color: #x00000000
                                                            color_hover: #x00000000
                                                            color_down: #x00000000
                                                            color_focus: #x00000000
                                                            color_disabled: #x00000000
                                                        }
                                                        draw_bg +: {
                                                            border_size: 0.0
                                                            border_radius: uniform(10.)
                                                            color: #x00000000
                                                            color_hover: #(palette().tint_hover)
                                                            color_down: #(palette().tint_down)
                                                            color_focus: #x00000000
                                                            color_disabled: #x00000000
                                                            color_2: #x00000000
                                                            color_2_hover: #x00000000
                                                            color_2_down: #x00000000
                                                            color_2_focus: #x00000000
                                                            color_2_disabled: #x00000000
                                                            border_color: #x00000000
                                                            border_color_hover: #x00000000
                                                            border_color_down: #x00000000
                                                            border_color_focus: #x00000000
                                                            border_color_disabled: #x00000000
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // 段间分隔线（§5）
                                        RectView {
                                            width: Fill
                                            height: 1
                                            show_bg: true
                                            draw_bg.color: #(palette().border)
                                        }

                                        // ── 第 3 段：深色/浅色模式（§5-3）
                                        View {
                                            width: Fill
                                            height: Fit
                                            flow: Down
                                            spacing: 8.

                                            // 分区标题：16px 黄色月亮/太阳（FontAwesome 单色）+ 14px 文字
                                            View {
                                                width: Fill
                                                height: Fit
                                                flow: Right
                                                spacing: 8.
                                                align: Align{y: 0.5}

                                                // 文字由 Rust 侧写入：U+F186 = fa-moon / U+F185 = fa-sun
                                                theme_mode_icon := Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: ""
                                                    draw_text +: {
                                                        color: #xFCE38A
                                                        text_style: theme.font_icons{font_size: 16}
                                                    }
                                                }

                                                Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: "深色/浅色模式"
                                                    draw_text +: {
                                                        color: #(palette().fg)
                                                        text_style: theme.font_regular{font_size: 14}
                                                    }
                                                }
                                            }

                                            // 一行底：muted/50、圆角 12、内距 12（规格 §5-3）
                                            RoundedView {
                                                width: Fill
                                                height: Fit
                                                flow: Right
                                                align: Align{y: 0.5}
                                                padding: Inset{left: 12 top: 12 right: 12 bottom: 12}
                                                show_bg: true
                                                draw_bg.color: #(palette().row_bg)
                                                draw_bg.border_radius: uniform(12.)

                                                theme_mode_label := Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: ""
                                                    draw_text +: {
                                                        color: #(palette().fg)
                                                        text_style: theme.font_regular{font_size: 14}
                                                    }
                                                }

                                                View { width: Fill height: 1 }

                                                // 开关：轨道 32×18.4、圆角 full、滑块 16 圆。
                                                // 深色=开（珊瑚轨道、黑滑块），浅色=关（深灰轨道、白滑块）。
                                                theme_track := RoundedView {
                                                    width: 32
                                                    height: 18.4
                                                    flow: Overlay
                                                    align: Align{x: 0.0, y: 0.0}
                                                    show_bg: true
                                                    draw_bg.color: #(palette().switch_track)
                                                    draw_bg.border_radius: uniform(9.2)

                                                    theme_knob := RoundedView {
                                                        width: 16
                                                        height: 16
                                                        margin: Inset{left: #(switch_knob_left()), top: 1.2}
                                                        show_bg: true
                                                        draw_bg.color: #(palette().switch_knob)
                                                        draw_bg.border_radius: uniform(8.)
                                                    }

                                                    // 透明按钮盖在整条轨道上，只负责命中与手型
                                                    theme_toggle_button := ButtonFlat {
                                                        width: Fill
                                                        height: Fill
                                                        margin: 0
                                                        padding: 0
                                                        text: ""
                                                        draw_text +: {
                                                            color: #x00000000
                                                            color_hover: #x00000000
                                                            color_down: #x00000000
                                                            color_focus: #x00000000
                                                            color_disabled: #x00000000
                                                        }
                                                        draw_bg +: {
                                                            border_size: 0.0
                                                            border_radius: uniform(9.2)
                                                            color: #x00000000
                                                            color_hover: #x00000000
                                                            color_down: #x00000000
                                                            color_focus: #x00000000
                                                            color_disabled: #x00000000
                                                            color_2: #x00000000
                                                            color_2_hover: #x00000000
                                                            color_2_down: #x00000000
                                                            color_2_focus: #x00000000
                                                            color_2_disabled: #x00000000
                                                            border_color: #x00000000
                                                            border_color_hover: #x00000000
                                                            border_color_down: #x00000000
                                                            border_color_focus: #x00000000
                                                            border_color_disabled: #x00000000
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // 段间分隔线（§5）
                                        RectView {
                                            width: Fill
                                            height: 1
                                            show_bg: true
                                            draw_bg.color: #(palette().border)
                                        }

                                        // ── 第 4 段：清空聊天记录（§5-4）
                                        // 危险操作从输入区移到这里：输入胶囊内部只剩
                                        // [fa-image] [input] [↑]（规格 §4），清空属于设置。
                                        View {
                                            width: Fill
                                            height: Fit
                                            flow: Down
                                            spacing: 8.

                                            // 分区标题：16px 红色垃圾桶 + 14px 文字
                                            View {
                                                width: Fill
                                                height: Fit
                                                flow: Right
                                                spacing: 8.
                                                align: Align{y: 0.5}

                                                // U+F2ED = FontAwesome 的 fa-trash-can，单色、跟随警示红
                                                Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: "\u{f2ed}"
                                                    draw_text +: {
                                                        color: #xEF4444
                                                        text_style: theme.font_icons{font_size: 16}
                                                    }
                                                }

                                                Label {
                                                    width: Fit
                                                    height: Fit
                                                    padding: 0
                                                    text: "清空聊天记录"
                                                    draw_text +: {
                                                        color: #(palette().fg)
                                                        text_style: theme.font_regular{font_size: 14}
                                                    }
                                                }
                                            }

                                            // 红底白字、32 高、圆角 10。二次确认复用
                                            // 下面的 clear_confirm（与网页版一致）。
                                            drawer_clear_button := glass.Button {
                                                text: "清空记录"
                                                width: Fill
                                                height: 32
                                                padding: 0
                                                draw_text +: {
                                                    color: #xffffffff
                                                    color_hover: #xffffffff
                                                    color_down: #xffffffff
                                                    text_style: theme.font_regular{font_size: 13}
                                                }
                                                draw_bg +: {
                                                    border_size: 0.0
                                                    border_radius: uniform(10.0)
                                                    color: #xEF4444
                                                    color_hover: #xDC2626
                                                    color_down: #xB91C1C
                                                    border_color: #x00000000
                                                    border_color_hover: #x00000000
                                                    border_color_down: #x00000000
                                                    top_glint: #x00000000
                                                    inner_shadow: #x00000000
                                                }
                                            }

                                            Label {
                                                width: Fill
                                                height: Fit
                                                padding: 0
                                                text: "此操作不可撤销，将删除所有聊天记录"
                                                draw_text +: {
                                                    color: #xB3B3B3
                                                    text_style: theme.font_regular{font_size: 12}
                                                }
                                            }
                                        }

                                        // 段间分隔线（§5）
                                        RectView {
                                            width: Fill
                                            height: 1
                                            show_bg: true
                                            draw_bg.color: #(palette().border)
                                        }

                                        // 页脚：18px「Ave atque vale」+ 16 上内距（§5）。
                                        // 渐变文字用随时间移动的单色近似，见 footer_glow()。
                                        Label {
                                            width: Fill
                                            height: Fit
                                            padding: Inset{top: 16}
                                            align: Align{x: 0.5}
                                            text: "Ave atque vale"
                                            draw_text +: {
                                                color: #(footer_glow())
                                                text_style: theme.font_regular{font_size: 18}
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 「清空聊天记录」二次确认。放在抽屉之后声明：Modal 的
                        // overlay 按绘制顺序叠放（modal.rs 的双 modal 测试里
                        // 「后声明的在上面」），抽屉开着时它必须压在最上层。
                        // can_dismiss 默认 true：点遮罩或 Esc 即取消，Modal 自己关。
                        clear_confirm := Modal {
                            content +: {
                                // 固定宽卡片 + Fill 子项：与 makepad 自带的
                                // Dialog（dialog.rs）和 modal_footprint 示例
                                // 同一个模式。宽度照 Dialog 的 420 收一档。
                                // RoundedView 而不是 View：裸 View 的 draw_bg 没有
                                // color/border_radius，卡片会整个透明（同根因，见上）。
                                card := RoundedView {
                                    width: 360
                                    height: Fit
                                    flow: Down
                                    spacing: 10.
                                    padding: Inset{left: 20 top: 16 right: 20 bottom: 16}
                                    show_bg: true
                                    draw_bg.color: #(palette().card)
                                    draw_bg.border_radius: uniform(16.)

                                    title := Label {
                                        width: Fill
                                        height: Fit
                                        text: "确认清空聊天记录？"
                                        draw_text +: {
                                            color: #(palette().fg)
                                            text_style: theme.font_bold{font_size: 15}
                                        }
                                    }

                                    desc := Label {
                                        width: Fill
                                        height: Fit
                                        text: "此操作将永久删除所有聊天记录，此操作不可撤销。"
                                        draw_text +: {
                                            color: #(palette().muted_fg)
                                            text_style: theme.font_regular{font_size: 12}
                                        }
                                    }

                                    row := View {
                                        width: Fill
                                        height: Fit
                                        flow: Right
                                        spacing: 8.
                                        align: Align{x: 1.0}

                                        cancel_clear_button := glass.GlassButton {
                                            text: "取消"
                                            width: 72
                                            height: 36
                                            // 默认白字在浅色玻璃上不可读；跟随当前模式正文色
                                            draw_text +: { color: #(palette().fg) }
                                        }

                                        confirm_clear_button := glass.GlassButtonProminent {
                                            text: "确认清空"
                                            width: 96
                                            height: 36
                                            // 警示红：GlassButton 没有 destructive 变体，靠 tint 自己调
                                            draw_glass +: { tint: vec4(0.85, 0.24, 0.20, 0.40) }
                                            // 深色模式白字 / 浅色模式深字（红 tint 叠在白玻璃上）
                                            draw_text +: { color: #(palette().fg) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
