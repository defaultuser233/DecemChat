//! 界面层：每个模块只负责画一块区域，并把「想做的事」写进 [`Action`]，
//! 由主循环统一执行，避免绘制途中修改状态。

pub mod composer;
pub mod header;
pub mod icons;
pub mod lucide_data;
pub mod messages;
pub mod settings;
pub mod svg_path;
pub mod textures;
pub mod toast;
pub mod widgets;

/// 界面产生的动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 发送当前输入。
    Send,
    /// 重新生成最后一条回复。
    Regenerate,
    /// 清空聊天记录（已经过二次确认）。
    ClearChat,
    /// 丢弃待发送图片。
    DropPendingImage,
    /// 选择待发送图片。
    PickImage,
    /// 选择用户头像。
    PickAvatar,
    /// 切换模型。
    SetModel(String),
    /// 切换深浅色。
    SetDarkMode(bool),
    /// 开关设置面板。
    ToggleSettings,
    /// 开关「确认清空」弹窗。
    ShowClearConfirm(bool),
    /// 保存首次填写的 API Key 并关闭弹窗。
    SaveApiKey,
    /// 关闭首次配置弹窗。
    CloseKeyPrompt,
}

/// 头像边长。
pub const AVATAR_SIZE: f32 = 40.0;
/// 内容列最大宽度（网页版 `max-w-3xl` = 768）。
pub const CONTENT_WIDTH: f32 = 768.0;
/// 气泡最大宽度（网页版 `max-w-[75%]`，按 768 的 75% 取）。
pub const BUBBLE_WIDTH: f32 = 576.0;
/// 消息之间的垂直间距（网页版 `space-y-6` = 24）。
pub const MESSAGE_GAP: f32 = 24.0;
/// 头像与气泡的间距（网页版 `gap-3` = 12）。
pub const AVATAR_GAP: f32 = 12.0;
