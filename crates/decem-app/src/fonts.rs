//! 字体：中文与 emoji 的回退字体。
//!
//! 关于 emoji 有个硬限制：**egui 只能渲染轮廓字形**。
//! `epaint-0.36.2/src/text/font.rs:200` 只取了 skrifa 的 `OutlineGlyphCollection`，
//! 没有任何 COLR/CBDT 位图通路，所以 Noto Color Emoji 这类彩色字体画不出来。
//! 因此这里内置**单色**的 Noto Emoji —— 与网页版那头用的 Noto 字形同源，
//! 形状一致、只是没有颜色，而且三个平台表现完全一样。
//!
//! 中文字体仍按平台探测（体积太大，不适合内置），可用环境变量覆盖。

use std::path::PathBuf;
use std::sync::Arc;

/// 覆盖中文字体路径的环境变量。
pub const FONT_ENV: &str = "DECEMCHAT_FONT";

/// 覆盖 emoji 字体路径的环境变量。
pub const EMOJI_FONT_ENV: &str = "DECEMCHAT_EMOJI_FONT";

/// 内置 emoji 字体（Noto Emoji，SIL OFL 1.1，见 `assets/OFL.txt`）。
const BUNDLED_EMOJI: &[u8] = include_bytes!("../assets/NotoEmoji.ttf");

const CJK_KEY: &str = "cjk";
const EMOJI_KEY: &str = "emoji";

#[cfg(target_os = "windows")]
const CJK_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyh.ttf",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\Deng.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
];

#[cfg(target_os = "macos")]
const CJK_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
];

#[cfg(all(unix, not(target_os = "macos")))]
const CJK_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "/usr/share/fonts/truetype/arphic/uming.ttc",
    "/usr/share/fonts/opentype/source-han-sans/SourceHanSansSC-Regular.otf",
];

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
const CJK_CANDIDATES: &[&str] = &[];

/// 实际装上的字体。
#[derive(Debug, Clone, Default)]
pub struct LoadedFonts {
    /// 中文字体来源。
    pub cjk: Option<String>,
    /// emoji 字体来源。
    pub emoji: Option<String>,
}

/// 已经读进内存的字体。
struct ResolvedFont {
    label: String,
    bytes: Vec<u8>,
}

/// 查找中文字体：环境变量优先，其次平台候选路径。
pub fn find_font() -> Option<PathBuf> {
    let override_path = std::env::var(FONT_ENV)
        .ok()
        .filter(|p| !p.trim().is_empty());
    override_path
        .into_iter()
        .map(PathBuf::from)
        .chain(CJK_CANDIDATES.iter().map(PathBuf::from))
        .find(|path| path.is_file())
}

/// 把中文与 emoji 字体作为回退字体装进 egui。
///
/// 回退顺序：egui 自带字体（拉丁 + 少量 emoji）→ 中文 → 内置 Noto Emoji。
pub fn install(ctx: &egui::Context) -> LoadedFonts {
    let mut fonts = egui::FontDefinitions::default();
    let mut loaded = LoadedFonts::default();

    let candidates = [
        (CJK_KEY, resolve_cjk(), Fallback::AfterLatin),
        (EMOJI_KEY, resolve_emoji(), Fallback::Last),
    ];
    for (key, font, fallback) in candidates {
        let Some(font) = font else {
            continue;
        };
        add_font(&mut fonts, key, font.bytes, fallback);
        match key {
            CJK_KEY => loaded.cjk = Some(font.label),
            _ => loaded.emoji = Some(font.label),
        }
    }

    ctx.set_fonts(fonts);
    if let Some(cjk) = &loaded.cjk {
        log::info!("已加载中文字体：{cjk}");
    }
    if let Some(emoji) = &loaded.emoji {
        log::info!("已加载 emoji 字体：{emoji}");
    }
    loaded
}

/// 回退链里的插入位置。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fallback {
    /// 紧跟拉丁字体之后。
    AfterLatin,
    /// 排在整条链的最后。
    Last,
}

/// 把字体数据挂进两个字体族的回退链。
///
/// 位置很关键：**中文字体必须紧跟拉丁字体**。
/// egui 自带的 `NotoEmoji-Regular`、`emoji-icon-font` 排在拉丁字体后面，
/// 而这两个字体里也带着 `？`、`！`、`…` 这类全角标点 —— 中文排在它们后面，
/// 标点就被它们抢走，画出来又粗又小（网页版是交给系统中文字体的）。
fn add_font(fonts: &mut egui::FontDefinitions, key: &str, bytes: Vec<u8>, fallback: Fallback) {
    // `.ttc` 是字体集合，取第一个字面
    let mut data = egui::FontData::from_owned(bytes);
    data.index = 0;
    fonts.font_data.insert(key.to_owned(), Arc::new(data));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let chain = fonts.families.entry(family).or_default();
        match fallback {
            // 0 号是拉丁字体，中文插到它后面
            Fallback::AfterLatin => {
                let at = chain.len().min(1);
                chain.insert(at, key.to_owned());
            }
            Fallback::Last => chain.push(key.to_owned()),
        }
    }
}

fn resolve_cjk() -> Option<ResolvedFont> {
    let path = find_font()?;
    let bytes = std::fs::read(&path)
        .inspect_err(|err| log::warn!("读取中文字体 {} 失败：{err}", path.display()))
        .ok()?;
    Some(ResolvedFont {
        label: path.display().to_string(),
        bytes,
    })
}

fn resolve_emoji() -> Option<ResolvedFont> {
    // 显式指定优先（用户想换字体时用）
    if let Ok(custom) = std::env::var(EMOJI_FONT_ENV)
        && !custom.trim().is_empty()
    {
        let path = PathBuf::from(custom);
        match std::fs::read(&path) {
            Ok(bytes) => {
                return Some(ResolvedFont {
                    label: path.display().to_string(),
                    bytes,
                });
            }
            Err(err) => log::warn!(
                "读取 emoji 字体 {} 失败，改用内置字体：{err}",
                path.display()
            ),
        }
    }

    Some(ResolvedFont {
        label: format!("内置 Noto Emoji（{} 字节）", BUNDLED_EMOJI.len()),
        bytes: BUNDLED_EMOJI.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用 egui 排一遍版，取每个字所用字体「脸」的度量 —— 用它区分到底用了哪个字体。
    fn face_metrics(ctx: &egui::Context, probes: &[&str]) -> Vec<(u32, u32)> {
        let mut faces: Vec<(u32, u32)> = Vec::new();
        // 字体要下一趟才生效，跑两趟
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                faces.clear();
                for probe in probes {
                    let galley = ui.painter().layout_no_wrap(
                        (*probe).to_owned(),
                        egui::FontId::proportional(14.0),
                        egui::Color32::WHITE,
                    );
                    let face = galley
                        .rows
                        .first()
                        .and_then(|row| row.row.glyphs.first())
                        .map_or((0, 0), |glyph| {
                            (
                                (glyph.font_face_ascent * 100.0).round() as u32,
                                (glyph.font_face_height * 100.0).round() as u32,
                            )
                        });
                    faces.push(face);
                }
            });
            output.textures_delta.clear();
        }
        faces
    }

    #[test]
    fn fullwidth_punctuation_uses_the_cjk_face() {
        // `？`、`！`、`～` 必须和中文字用同一个字体：egui 自带的 emoji / 图标字体里
        // 也带着这些全角标点，中文排在它们后面就会被抢走，画出来又粗又小
        // （网页版是由系统中文字体画的）。
        let ctx = egui::Context::default();
        let loaded = install(&ctx);
        if loaded.cjk.is_none() {
            return; // 本机没有中文字体，这条不适用
        }
        let measured = face_metrics(&ctx, &["咪", "？", "！", "，", "～", "。"]);
        let expected = measured[0];
        assert!(
            measured.iter().all(|face| *face == expected),
            "全角标点应与中文同一个字体：{measured:?}"
        );
    }

    #[test]
    fn latin_keeps_its_own_face() {
        let ctx = egui::Context::default();
        let loaded = install(&ctx);
        if loaded.cjk.is_none() {
            return;
        }
        let measured = face_metrics(&ctx, &["A", "咪"]);
        assert_ne!(
            measured[0], measured[1],
            "拉丁字母仍应走拉丁字体：{measured:?}"
        );
    }

    #[test]
    fn font_discovery_does_not_panic() {
        // 找不到系统字体是允许的（会退回自带字体），但绝不能 panic
        let _ = find_font();
    }

    #[test]
    fn bundled_emoji_font_is_a_truetype_file() {
        assert!(BUNDLED_EMOJI.len() > 100_000, "内置 emoji 字体不应是空文件");
        // TrueType 的 sfnt 版本号 0x00010000 或 "OTTO"
        let magic = &BUNDLED_EMOJI[..4];
        assert!(
            magic == [0x00, 0x01, 0x00, 0x00] || magic == *b"OTTO",
            "内置 emoji 字体不是合法的 sfnt：{magic:?}"
        );
    }
}
