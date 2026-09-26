//! 本地持久化：替换网页版的 `localStorage` + IndexedDB。
//!
//! 数据放在平台标准配置目录下（由 `directories::ProjectDirs` 决定）：
//!
//! - Windows：`%APPDATA%\valedecem\DecemChat\config`
//! - macOS：`~/Library/Application Support/top.valedecem.DecemChat`
//! - Linux：`~/.config/decemchat`
//!
//! 写入一律「先写临时文件再改名」，避免断电 / 崩溃时留下半截 JSON。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use decem_core::{ChatSettings, Message};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// 与网页版一致：最多保留最近 50 条聊天记录。
pub const MESSAGE_STORAGE_LIMIT: usize = 50;

/// 图片子目录名。
const IMAGES_DIR: &str = "images";

/// 落盘的配置：界面设置 + 可选的自定义接口信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConfig {
    /// 界面设置，字段与网页版 `decem-chat-settings` 同构。
    #[serde(flatten)]
    pub settings: ChatSettings,
    /// 用户填写的 API Key（也可只靠环境变量）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// 自定义接口地址，留空则用官方地址。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
}

impl StoredConfig {
    /// 用给定设置构造一份配置。
    pub fn new(settings: ChatSettings) -> Self {
        Self {
            settings,
            api_key: None,
            api_url: None,
        }
    }
}

/// 磁盘读写入口。
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// 打开平台默认配置目录并确保存在。
    pub fn open() -> Result<Self> {
        let dirs = directories::ProjectDirs::from("top", "valedecem", "DecemChat")
            .context("无法定位用户配置目录")?;
        Self::at(dirs.config_dir())
    }

    /// 使用指定目录（测试与 `--selftest` 使用）。
    pub fn at(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join(IMAGES_DIR))
            .with_context(|| format!("无法创建配置目录 {}", root.display()))?;
        Ok(Self { root })
    }

    /// 数据根目录。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 读取配置；文件不存在或损坏时返回 `None`。
    pub fn load_config(&self) -> Option<StoredConfig> {
        read_json(&self.config_path())
    }

    /// 保存配置。
    pub fn save_config(&self, config: &StoredConfig) -> Result<()> {
        write_json(&self.config_path(), config)
    }

    /// 读取聊天记录；文件不存在或损坏时返回空列表。
    pub fn load_messages(&self) -> Vec<Message> {
        read_json(&self.messages_path()).unwrap_or_default()
    }

    /// 保存聊天记录：只留最近 [`MESSAGE_STORAGE_LIMIT`] 条。
    ///
    /// 图片不进 JSON（[`Message::image_url`] 标了 `serde(skip)`），单独存在 images 目录里，
    /// 因此这里直接写借用到的子切片，不需要逐条克隆。
    pub fn save_messages(&self, messages: &[Message]) -> Result<()> {
        let recent = &messages[messages.len().saturating_sub(MESSAGE_STORAGE_LIMIT)..];
        write_json(&self.messages_path(), &recent)
    }

    /// 按消息 ID 保存图片（data URL 原样落盘，自描述、免额外元数据）。
    pub fn save_image(&self, id: &str, data_url: &str) -> Result<()> {
        write_atomic(&self.image_path(id), data_url.as_bytes())
    }

    /// 按消息 ID 读回图片。
    pub fn load_image(&self, id: &str) -> Option<String> {
        fs::read_to_string(self.image_path(id))
            .ok()
            .filter(|text| !text.is_empty())
    }

    /// 清空图片目录。
    pub fn clear_images(&self) -> Result<()> {
        let dir = self.root.join(IMAGES_DIR);
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("删除 {} 失败", dir.display()))?;
        }
        fs::create_dir_all(&dir).with_context(|| format!("创建 {} 失败", dir.display()))?;
        Ok(())
    }

    fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    fn messages_path(&self) -> PathBuf {
        self.root.join("messages.json")
    }

    fn image_path(&self, id: &str) -> PathBuf {
        self.root
            .join(IMAGES_DIR)
            .join(format!("{}.b64", sanitize_id(id)))
    }
}

/// 只保留字母、数字、`-`、`_`，避免 ID 变成路径穿越。
fn sanitize_id(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if cleaned.is_empty() {
        "image".to_owned()
    } else {
        cleaned
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            log::warn!("解析 {} 失败，已忽略：{err}", path.display());
            None
        }
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value).context("序列化失败")?;
    write_atomic(path, text.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).with_context(|| format!("写入 {} 失败", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("替换 {} 失败", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use decem_core::{MessageRole, default_settings};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_store() -> Store {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("decemchat-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Store::at(&dir).expect("创建测试目录失败")
    }

    #[test]
    fn sanitize_id_strips_path_characters() {
        assert_eq!(sanitize_id("1758000000000"), "1758000000000");
        assert_eq!(sanitize_id("../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_id("///"), "image");
    }

    #[test]
    fn config_round_trips() {
        let store = temp_store();
        let mut config = StoredConfig::new(default_settings());
        config.api_key = Some("sk-test".to_owned());
        config.settings.model = "kimi-k3".to_owned();
        store.save_config(&config).expect("保存配置失败");

        let loaded = store.load_config().expect("配置应能读回");
        assert_eq!(loaded.settings.model, "kimi-k3");
        assert_eq!(loaded.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn messages_are_trimmed_and_images_stay_out_of_json() {
        let store = temp_store();
        let messages: Vec<Message> = (0..60)
            .map(|i| {
                let mut message =
                    Message::text(i.to_string(), MessageRole::User, format!("m{i}"), i);
                message.image_url = Some("data:image/png;base64,AAAA".to_owned());
                message
            })
            .collect();
        store.save_messages(&messages).expect("保存消息失败");

        let loaded = store.load_messages();
        assert_eq!(loaded.len(), MESSAGE_STORAGE_LIMIT);
        assert_eq!(loaded[0].content, "m10");
        assert!(loaded.iter().all(|m| m.image_url.is_none()));

        // 文件里也不该出现图片数据（image_url 是内存态字段）
        let raw = fs::read_to_string(store.messages_path()).expect("读取聊天记录文件失败");
        assert!(
            !raw.contains("data:image"),
            "聊天记录文件不应包含 base64 图片"
        );
    }

    #[test]
    fn images_round_trip_and_clear() {
        let store = temp_store();
        store
            .save_image("1758000000000", "data:image/png;base64,AAAA")
            .expect("保存图片失败");
        assert_eq!(
            store.load_image("1758000000000").as_deref(),
            Some("data:image/png;base64,AAAA")
        );
        assert_eq!(store.load_image("not-there"), None);

        store.clear_images().expect("清空图片失败");
        assert_eq!(store.load_image("1758000000000"), None);
    }

    #[test]
    fn legacy_messages_file_with_image_url_still_loads() {
        let store = temp_store();
        // 旧版可能把 imageUrl 写进过文件；skip 掉的字段应被忽略，而不是解析失败
        fs::write(
            store.messages_path(),
            r#"[{"id":"1","role":"user","content":"嗷呜","timestamp":1,"imageUrl":"data:image/png;base64,AAAA","imagePath":"stored"}]"#,
        )
        .expect("写入旧格式文件失败");

        let loaded = store.load_messages();
        assert_eq!(loaded.len(), 1, "旧格式文件应能读回");
        assert_eq!(loaded[0].content, "嗷呜");
        assert_eq!(loaded[0].image_path.as_deref(), Some("stored"));
        assert!(loaded[0].image_url.is_none(), "图片数据应被忽略");
    }

    #[test]
    fn corrupted_json_is_ignored() {
        let store = temp_store();
        fs::write(store.config_path(), b"{ not json").expect("写入失败");
        assert!(store.load_config().is_none());
        assert!(store.load_messages().is_empty());
    }
}
