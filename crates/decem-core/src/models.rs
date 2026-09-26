//! 可用模型表与用户设置。
//!
//! 模型清单来自前端的 `src/types/index.ts`（2026-09 更新），是唯一事实来源。

use serde::{Deserialize, Serialize};

/// 一个可选模型及其能力标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AIModel {
    /// 下发给服务商的模型 ID。
    pub id: &'static str,
    /// 界面展示名。
    pub name: &'static str,
    /// 一句话介绍（带 emoji，前端原样保留）。
    pub description: &'static str,
    /// 是否支持图片输入。
    pub supports_vision: bool,
    /// 是否接受 `temperature` 参数（部分三方模型传了会报错）。
    pub supports_temperature: bool,
    /// 上下文窗口（token 数）。
    pub context_window: Option<u64>,
}

/// 百炼平台模型清单。
pub const AVAILABLE_MODELS: &[AIModel] = &[
    AIModel {
        id: "qwen3.8-flash",
        name: "Qwen3.8-Flash",
        description: "⚡像闪电一样快的小狐狸！适合需要快速响应的日常对话，虽然体积小但能力不弱哦～",
        supports_vision: true,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
    AIModel {
        id: "qwen3.7-plus",
        name: "Qwen3.7-Plus",
        description: "🧠 更聪明的大狐狸！推理和复杂任务更强，像能同时记住浆果藏在森林的哪里、又帮松鼠算松子库存～",
        supports_vision: true,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
    AIModel {
        id: "qwen3.8-max",
        name: "Qwen3.8-Max",
        description: "🏆森林里的智者！综合能力最强，能写诗、解谜、画地图，还会用尾巴尖编复杂的故事～",
        supports_vision: true,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
    AIModel {
        id: "qwen3.5-omni-plus",
        name: "Qwen3.5-Omni-Plus",
        description: "🌐全能的旅行狐狸！多语言和跨领域知识都擅长，像会翻译鸟语、看懂蘑菇生长的密码～",
        supports_vision: true,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
    AIModel {
        id: "deepseek-v4-pro",
        name: "DeepSeek-V4-Pro",
        description: "🐳人坏，小鲸鱼好",
        supports_vision: false,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
    AIModel {
        id: "deepseek-v4-flash",
        name: "DeepSeek-V4-Flash",
        description: "🐳小鲸鱼最好了",
        supports_vision: false,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
    AIModel {
        id: "kimi-k3",
        name: "Kimi-K3",
        description: "🌕寻求将能源转化为智能的最优解",
        supports_vision: true,
        supports_temperature: false,
        context_window: Some(262_144),
    },
    AIModel {
        id: "glm-5.2",
        name: "GLM-5.2",
        description: "🐼整理竹子的熊猫",
        supports_vision: false,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
    AIModel {
        id: "MiniMax-M3",
        name: "MiniMax-M3",
        description: "🌊来自深海的吟游诗人，文笔流畅、情感细腻",
        supports_vision: false,
        supports_temperature: true,
        context_window: Some(196_608),
    },
    AIModel {
        id: "mimo-v2.5-pro",
        name: "MiMo-v2.5-Pro",
        description: "🫧小水母的灵感泡泡，创意与效率兼得",
        supports_vision: false,
        supports_temperature: true,
        context_window: Some(1_048_576),
    },
];

/// 默认模型 ID。
pub const DEFAULT_MODEL: &str = "qwen3.8-flash";

/// 可选的 Decem 头像。
pub const CHAR_AVATARS: &[&str] = &[
    "./images/char_01.jpg",
    "./images/char_02.jpg",
    "./images/char_03.jpg",
];

/// 可选的用户头像。
pub const USER_AVATARS: &[&str] = &["./images/user_01.jpg", "./images/user_02.jpg"];

/// 界面设置，可序列化后落盘 / 存 localStorage 同构结构。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettings {
    /// 当前模型 ID。
    pub model: String,
    /// 用户头像路径。
    pub user_avatar: String,
    /// Decem 头像路径。
    pub char_avatar: String,
    /// 是否深色模式。
    pub is_dark_mode: bool,
    /// 采样温度；`None` 表示不发送该参数。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// 按当前时间取一个随机头像：与前端「每次加载都随机」的行为一致。
///
/// 不引入随机数依赖，用纳秒时间低位打散即可 —— 头像随机性没有统计意义要求。
fn pick_avatar(candidates: &[&str]) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    candidates
        .get(nanos % candidates.len())
        .copied()
        .unwrap_or_default()
        .to_owned()
}

/// 随机挑一个 Decem 头像。
pub fn random_char_avatar() -> String {
    pick_avatar(CHAR_AVATARS)
}

/// 随机挑一个用户头像。
pub fn random_user_avatar() -> String {
    pick_avatar(USER_AVATARS)
}

/// 默认设置（头像随机）。
pub fn default_settings() -> ChatSettings {
    ChatSettings {
        model: DEFAULT_MODEL.to_owned(),
        char_avatar: random_char_avatar(),
        user_avatar: random_user_avatar(),
        is_dark_mode: true,
        temperature: Some(0.8),
    }
}

/// 按 ID 查模型；未知 ID 返回 `None`。
pub fn find_model(id: &str) -> Option<&'static AIModel> {
    AVAILABLE_MODELS.iter().find(|m| m.id == id)
}

/// 该模型是否支持图片输入。
pub fn is_vision_model(model: &str) -> bool {
    find_model(model).is_some_and(|m| m.supports_vision)
}

/// 该模型是否接受 `temperature`；未知模型按「支持」处理，与前端默认值一致。
pub fn supports_temperature(model: &str) -> bool {
    find_model(model).is_none_or(|m| m.supports_temperature)
}

/// 模型展示名；未知 ID 原样返回。
pub fn model_display_name(model: &str) -> String {
    find_model(model).map_or_else(|| model.to_owned(), |m| m.name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_models_have_expected_capabilities() {
        assert!(is_vision_model("qwen3.8-flash"));
        assert!(!is_vision_model("deepseek-v4-pro"));
        assert!(!supports_temperature("kimi-k3"));
        // 未知模型默认允许 temperature，避免误伤新模型
        assert!(supports_temperature("brand-new-model"));
        assert!(!is_vision_model("brand-new-model"));
    }

    #[test]
    fn defaults_are_complete() {
        let settings = default_settings();
        assert_eq!(settings.model, DEFAULT_MODEL);
        assert!(CHAR_AVATARS.contains(&settings.char_avatar.as_str()));
        assert!(USER_AVATARS.contains(&settings.user_avatar.as_str()));
        assert!(settings.is_dark_mode);
    }

    #[test]
    fn display_name_falls_back_to_id() {
        assert_eq!(model_display_name("kimi-k3"), "Kimi-K3");
        assert_eq!(model_display_name("who-knows"), "who-knows");
    }
}
