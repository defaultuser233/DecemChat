//! 图片纹理缓存：同一张图只解码一次。

use std::collections::HashMap;

use egui::TextureHandle;

use crate::{assets, media};

/// 纹理缓存。
#[derive(Default)]
pub struct TextureCache {
    map: HashMap<String, TextureHandle>,
}

impl TextureCache {
    /// 新建空缓存。
    pub fn new() -> Self {
        Self::default()
    }

    /// 头像：既支持内嵌的内置头像路径，也支持用户上传的 data URL。
    pub fn avatar(&mut self, ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
        if path.starts_with("data:") {
            let key = format!("avatar-inline:{}", short_hash(path));
            return self.data_url(ctx, &key, path);
        }
        let key = format!("avatar:{path}");
        if let Some(handle) = self.map.get(&key) {
            return Some(handle.clone());
        }
        let bytes = assets::avatar_bytes(path)?;
        self.load(ctx, &key, bytes)
    }

    /// 由 data URL 加载（图片消息、上传的头像、GIF 拼接图）。
    pub fn data_url(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        data_url: &str,
    ) -> Option<TextureHandle> {
        if let Some(handle) = self.map.get(key) {
            return Some(handle.clone());
        }
        let bytes = media::decode_data_url(data_url)?;
        self.load(ctx, key, &bytes)
    }

    fn load(&mut self, ctx: &egui::Context, key: &str, bytes: &[u8]) -> Option<TextureHandle> {
        let image = media::color_image_from_bytes(bytes)?;
        let handle = ctx.load_texture(key, image, egui::TextureOptions::LINEAR);
        self.map.insert(key.to_owned(), handle.clone());
        Some(handle)
    }
}

/// 把超长的 data URL 折成短键（FNV-1a，够用且无依赖）。
fn short_hash(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_differ_for_different_inputs() {
        assert_ne!(short_hash("abc"), short_hash("abd"));
        assert_eq!(short_hash("abc"), short_hash("abc"));
    }
}
