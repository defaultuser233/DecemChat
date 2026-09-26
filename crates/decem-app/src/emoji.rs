//! 彩色 emoji：把 emoji 当图片画，绕开 egui 只能画轮廓字形的限制。
//!
//! `epaint-0.36.2/src/text/font.rs:200` 只取了 skrifa 的 `OutlineGlyphCollection`，
//! COLR / CBDT 这些彩色字形表它根本不读，所以彩色 emoji 走不了字体管线，只能走图片：
//!
//! 1. 渲染 Markdown 之前，把正文里成对的 emoji 换成一个内联 span —— `$emoji:1f98a$`
//!    （见 [`to_render_markdown`]）；
//! 2. `egui_commonmark` 把这个 span 当行内公式，回调 [`render_math`]
//!    （`egui_commonmark-0.25.0/src/parsers/pulldown.rs:576` 的 `math_fn` 分支）；
//! 3. 回调里用 skrifa 从内置的 Noto Color Emoji（CBDT/PNG 位图）取出该字的位图，
//!    按当前 `pixels_per_point` 缩放后画成纹理。
//!
//! 于是清晰度跟着屏幕缩放级别走，也不依赖任何平台字体。
//!
//! 覆盖范围：所有走 `ui.label` 的地方（正文、标题、列表、引用、表格）。
//! 代码块用的是 `egui::TextEdit`（`egui_commonmark_backend-0.25.0/src/elements.rs:105`），
//! `Galley` 里塞不进图片，那里仍是单色 Noto Emoji。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use egui::{Color32, FontId, Rect, RichText, Sense, TextureHandle, TextureOptions, Vec2};
use skrifa::MetadataProvider as _;

/// 内置彩色 emoji 字体（Noto Color Emoji，SIL OFL 1.1，见 `assets/OFL.txt`）。
const COLOR_EMOJI: &[u8] = include_bytes!("../assets/NotoColorEmoji.ttf");

/// 内联 span 的载荷前缀：`$emoji:1f98a$`。
const PAYLOAD_PREFIX: &str = "emoji:";

/// `U+FE0F`：要求彩色呈现，跟在 emoji 后面时一起换成图片。
const VARIATION_SELECTOR_EMOJI: char = '\u{fe0f}';
/// `U+FE0E`：要求文字呈现，这时不该换成图片。
const VARIATION_SELECTOR_TEXT: char = '\u{fe0e}';
/// `U+200D`：零宽连接符，组合 emoji（如 👨‍👩‍👧）用它把多个字连成一个字形。
const ZERO_WIDTH_JOINER: char = '\u{200d}';

/// 肤色修饰符（👍🏻 的后半个字）。
const SKIN_TONES: std::ops::RangeInclusive<u32> = 0x1f3fb..=0x1f3ff;

/// 可能出现彩色字形的码点范围。
const EMOJI_RANGES: &[std::ops::RangeInclusive<u32>] = &[
    0x2600..=0x27bf,   // 杂项符号、装饰符号（❤ ✨ ✅ ⚠ 等）
    0x2b00..=0x2bff,   // 杂项符号与箭头（⭐ ⬛ ⭕ 等）
    0x1f000..=0x1f0ff, // 麻将、多米诺、扑克（🀄 🃏）
    0x1f200..=0x1f2ff, // 带框表意文字补充（🈁 🉐 等）
    0x1f300..=0x1faff, // 图形符号、补充符号、扩展 A（🦊 🌸 🥹 等）
];

/// 量尺寸用的探针：单色 emoji 字体里一定有的字。
const PROBE: &str = "🦊";

/// 纹理边长（像素）上限，别为超大字体尺寸白白光栅化。
const MAX_TEXTURE_PIXELS: u32 = 512;

/// 一枚 emoji 的高度（单位：em）。
///
/// Noto Color Emoji 的 CBDT 位图框是 136×128（ppem 128），但里面的画只占框高的
/// 六成左右；网页版用的 Segoe UI Emoji 是「字宽 1.37 em、墨迹 0.9 em」。
/// 把位图框按 1.4 em 画，正好把墨迹放大到 0.88 em 上下，跟网页版一致。
const EMOJI_HEIGHT_EM: f32 = 1.4;

/// 把正文换成为渲染用的 Markdown：段落里的 emoji 变成 `$emoji:xxxxx$`。
///
/// 三处保持原样，因为在那里插入 `$` 只会显示成乱码或者被误当成公式：
///
/// - 围栏代码块与缩进代码块（其中的 `$` 不会被解析成行内公式）；
/// - 行内代码（同上）；
/// - 整段里出现 `$` 的段落 —— 否则我们插入的 `$` 会和原有的 `$` 配对成公式，
///   把公式外面的文字一起吞掉。
pub fn to_render_markdown(content: &str) -> String {
    let mut out = String::with_capacity(content.len() + 32);
    let mut paragraph: Vec<&str> = Vec::new();
    let mut paragraph_has_dollar = false;
    let mut fence: Option<(char, usize)> = None;
    let mut indented_code = false;
    let mut previous_blank = true;

    for chunk in content.split_inclusive('\n') {
        let (line, _newline) = split_line(chunk);

        // 围栏代码块内部：原样，直到遇到同字符、不少于起始长度的收尾围栏
        if let Some((marker, length)) = fence {
            out.push_str(chunk);
            if closes_fence(line, marker, length) {
                fence = None;
            }
            continue;
        }

        let blank = line.trim().is_empty();
        let fence_start = (!blank).then(|| starts_fence(line)).flatten();

        if indented_code && !blank && !is_indented(line) {
            indented_code = false;
        }

        if let Some((marker, length)) = fence_start {
            flush(&mut out, &mut paragraph, &mut paragraph_has_dollar);
            out.push_str(chunk);
            fence = Some((marker, length));
            previous_blank = false;
            continue;
        }

        if indented_code && (blank || is_indented(line)) {
            flush(&mut out, &mut paragraph, &mut paragraph_has_dollar);
            out.push_str(chunk);
            previous_blank = blank;
            continue;
        }

        if blank {
            flush(&mut out, &mut paragraph, &mut paragraph_has_dollar);
            out.push_str(chunk);
            previous_blank = true;
            continue;
        }

        // 紧跟在空行后面的四空格缩进是缩进代码块的开头
        if previous_blank && is_indented(line) {
            flush(&mut out, &mut paragraph, &mut paragraph_has_dollar);
            out.push_str(chunk);
            indented_code = true;
            previous_blank = false;
            continue;
        }

        paragraph.push(chunk);
        paragraph_has_dollar |= line.contains('$');
        previous_blank = false;
    }

    flush(&mut out, &mut paragraph, &mut paragraph_has_dollar);
    out
}

/// 落笔一段：段里有 `$` 就整段原样输出，否则逐行替换 emoji。
fn flush(out: &mut String, paragraph: &mut Vec<&str>, has_dollar: &mut bool) {
    for chunk in paragraph.drain(..) {
        if *has_dollar {
            out.push_str(chunk);
        } else {
            let (line, newline) = split_line(chunk);
            push_rendered(out, line);
            out.push_str(newline);
        }
    }
    *has_dollar = false;
}

/// 拆出一行的内容与行尾换行符。
fn split_line(chunk: &str) -> (&str, &str) {
    chunk
        .strip_suffix('\n')
        .map_or((chunk, ""), |line| (line, "\n"))
}

/// 一行的 emoji 替换结果。
fn push_rendered(out: &mut String, line: &str) {
    use std::fmt::Write as _;

    for segment in scan_line(line, true) {
        match segment {
            Segment::Text(text) | Segment::Code(text) => out.push_str(&text),
            Segment::Emoji(ch) => {
                out.push('$');
                out.push_str(PAYLOAD_PREFIX);
                let _ = write!(out, "{:x}", u32::from(ch));
                out.push('$');
            }
        }
    }
}

/// 把纯文本里的 emoji 直接画成图片，其余交给 egui 排版。
///
/// 用于没有 Markdown 的场合（打字机动画那一段），免得打完字 emoji 才变色。
pub fn show_text(ui: &mut egui::Ui, text: &str, font: FontId, color: Color32) {
    for segment in scan_line(text, false) {
        match segment {
            Segment::Text(text) | Segment::Code(text) => {
                if !text.is_empty() {
                    ui.label(RichText::new(text).font(font.clone()).color(color));
                }
            }
            Segment::Emoji(ch) => {
                let _ = draw(ui, ch, &font);
            }
        }
    }
}

/// 画一段可以换行的文本，emoji 画成图片。
///
/// 抽屉里的模型说明这类「普通一行字」用它，emoji 才是彩色。
/// 行内间距归零，靠位图自带的留白分隔，与正文里的排法一致。
pub fn wrapped_text(ui: &mut egui::Ui, text: &str, font: FontId, color: Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        show_text(ui, text, font, color);
    });
}

/// `egui_commonmark` 的行内公式回调：是 emoji 就画图，否则按字面文本还原成公式。
///
/// 还原很重要：段落里出现 `$` 时它可能被解析成公式（我们没法禁止），
/// 那样至少要和打开 math 之前显示的文字一模一样。
pub fn render_math(ui: &mut egui::Ui, payload: &str, _inline: bool) {
    if let Some(ch) = parse_payload(payload) {
        if draw(ui, ch, &body_font(ui)).is_some() {
            return;
        }
        // 字体里没有这个字的位图：退回单色字形
        ui.label(RichText::new(ch.to_string()));
        return;
    }
    ui.label(format!("${payload}$"));
}

/// 一行里的片段。
enum Segment {
    /// 普通文本。
    Text(String),
    /// 行内代码（含反引号），原样保留。
    Code(String),
    /// 可以换成彩色图片的 emoji。
    Emoji(char),
}

/// 扫描一行，切成「文本 / 行内代码 / emoji」。
///
/// `code_spans` 为真时识别行内代码（Markdown 渲染用），为假时把反引号当普通字符。
fn scan_line(line: &str, code_spans: bool) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut text = String::new();
    let mut previous: Option<char> = None;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if code_spans && ch == '`' {
            let mut run = 1_usize;
            let mut code = String::from("`");
            while chars.next_if_eq(&'`').is_some() {
                run += 1;
                code.push('`');
            }
            let mut closed = false;
            while let Some(next) = chars.next() {
                code.push(next);
                if next == '`' {
                    let mut closing = 1_usize;
                    while chars.next_if_eq(&'`').is_some() {
                        closing += 1;
                        code.push('`');
                    }
                    if closing == run {
                        closed = true;
                        break;
                    }
                }
            }
            // 没有收尾反引号就不是代码，当普通文本
            if closed {
                push_text(&mut segments, &mut text);
                segments.push(Segment::Code(code));
            } else {
                text.push_str(&code);
            }
            previous = Some('`');
            continue;
        }

        if is_emoji_base(ch) && can_substitute(previous, chars.peek().copied()) {
            push_text(&mut segments, &mut text);
            segments.push(Segment::Emoji(ch));
            if chars.next_if_eq(&VARIATION_SELECTOR_EMOJI).is_some() {
                // 变体选择符由图片代替，丢掉
            }
            previous = Some(ch);
            continue;
        }

        text.push(ch);
        previous = Some(ch);
    }

    push_text(&mut segments, &mut text);
    segments
}

fn push_text(segments: &mut Vec<Segment>, text: &mut String) {
    if !text.is_empty() {
        segments.push(Segment::Text(std::mem::take(text)));
    }
}

/// 这个字本身可能带彩色字形吗（不含肤色修饰符、不含区域指示符）。
fn is_emoji_base(ch: char) -> bool {
    let code = u32::from(ch);
    !SKIN_TONES.contains(&code) && EMOJI_RANGES.iter().any(|range| range.contains(&code))
}

/// 这个字能不能单独换成图片。
///
/// 前面是零宽连接符、后面跟着零宽连接符 / 肤色修饰符时不行 —— 那是组合序列的一部分，
/// 不整形就拆开画会散架；后面跟着 `U+FE0E` 时也不行，那是明确要求文字呈现。
fn can_substitute(previous: Option<char>, next: Option<char>) -> bool {
    if previous == Some(ZERO_WIDTH_JOINER) {
        return false;
    }
    match next {
        Some(ZERO_WIDTH_JOINER) | Some(VARIATION_SELECTOR_TEXT) => false,
        Some(ch) => !SKIN_TONES.contains(&u32::from(ch)),
        None => true,
    }
}

/// `emoji:1f98a` → `🦊`。
fn parse_payload(payload: &str) -> Option<char> {
    let hex = payload.strip_prefix(PAYLOAD_PREFIX)?;
    let code = u32::from_str_radix(hex, 16).ok()?;
    char::from_u32(code).filter(|ch| is_emoji_base(*ch))
}

/// 当前正文的字号。
fn body_font(ui: &egui::Ui) -> FontId {
    ui.style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(|| FontId::proportional(14.0))
}

/// 画一枚 emoji。返回 `None` 表示这个字没有可用位图。
fn draw(ui: &mut egui::Ui, ch: char, font: &FontId) -> Option<()> {
    let (line_height, ink_offset) = glyph_metrics(ui, font);
    let height = font.size * EMOJI_HEIGHT_EM;
    if height <= 0.0 {
        return None;
    }
    let pixels = (height * ui.ctx().pixels_per_point())
        .round()
        .clamp(8.0, MAX_TEXTURE_PIXELS as f32) as u32;
    let texture = texture(ui.ctx(), ch, pixels)?;

    let texture_size = texture.size_vec2();
    let aspect = if texture_size.y > 0.0 {
        texture_size.x / texture_size.y
    } else {
        1.0
    };
    let width = height * aspect;
    // 占位高度只按单色字形的行高算：图片比行高时也不会把整行撑高
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(width, line_height), Sense::hover());
    // 行是底对齐排的，把图片抬到单色字形原来的下沿位置上
    let target = Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() - ink_offset - height),
        Vec2::new(width, height),
    );
    ui.painter().image(
        texture.id(),
        target,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    Some(())
}

/// 单色 emoji 字形的行高，以及它的下沿离行底有多远。
///
/// 拿单色字体里同一个字量出来的，换成图片后下沿正好落在原来那个字形的下沿上。
fn glyph_metrics(ui: &egui::Ui, font: &FontId) -> (f32, f32) {
    let galley = ui
        .painter()
        .layout_no_wrap(PROBE.to_owned(), font.clone(), Color32::WHITE);
    let ink_bottom = galley.rows.first().map_or(galley.rect.bottom(), |placed| {
        placed.pos.y + placed.row.visuals.mesh_bounds.bottom()
    });
    // 量不出来时（没有字形、mesh 为空）退化成贴着行底
    let offset = (galley.rect.bottom() - ink_bottom).clamp(0.0, font.size * 0.5);
    (galley.rect.height(), offset)
}

/// 取纹理，同一（字符，像素尺寸）只光栅化一次。
fn texture(ctx: &egui::Context, ch: char, pixels: u32) -> Option<TextureHandle> {
    let key = (ch, pixels);
    if let Ok(cache) = cache().lock()
        && let Some(handle) = cache.get(&key)
    {
        return Some(handle.clone());
    }

    let image = rasterize(ch, pixels)?;
    let handle = ctx.load_texture(
        format!("emoji/{:x}/{pixels}", u32::from(ch)),
        image,
        TextureOptions::LINEAR,
    );
    if let Ok(mut cache) = cache().lock() {
        cache.insert(key, handle.clone());
    }
    Some(handle)
}

fn cache() -> &'static Mutex<HashMap<(char, u32), TextureHandle>> {
    static CACHE: OnceLock<Mutex<HashMap<(char, u32), TextureHandle>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 从 CBDT 位图字体里取出这个字的 PNG 位图，缩放到目标像素尺寸。
///
/// `pixels` 是目标**高度**（像素），宽度按位图自身的宽高比算出来。
fn rasterize(ch: char, pixels: u32) -> Option<egui::ColorImage> {
    let font = skrifa::FontRef::from_index(COLOR_EMOJI, 0).ok()?;
    let glyph_id = font.charmap().map(ch)?;
    let glyph = font
        .bitmap_strikes()
        .glyph_for_size(skrifa::instance::Size::new(pixels as f32), glyph_id)?;
    let skrifa::bitmap::BitmapData::Png(png) = glyph.data else {
        return None;
    };

    let decoded = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let (source_width, source_height) = (decoded.width().max(1), decoded.height().max(1));
    let target_height = pixels.max(1);
    let target_width = (target_height * source_width / source_height).max(1);
    let scaled = decoded
        .resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        )
        .into_rgba8();
    let (width, height) = scaled.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        scaled.as_raw(),
    ))
}

/// 围栏代码块的起始：三个以上同字符（`` ` `` 或 `~`），且缩进不超过三格。
fn starts_fence(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let length = trimmed.chars().take_while(|ch| *ch == marker).count();
    // 反引号围栏的信息串里不能再出现反引号
    if length < 3 || (marker == '`' && trimmed.chars().skip(length).any(|ch| ch == '`')) {
        return None;
    }
    Some((marker, length))
}

/// 是不是围栏代码块的收尾行。
fn closes_fence(line: &str, marker: char, length: usize) -> bool {
    let trimmed = line.trim_start_matches(' ').trim_end();
    let run = trimmed.chars().take_while(|ch| *ch == marker).count();
    run >= length && trimmed.chars().all(|ch| ch == marker || ch == ' ')
}

fn is_indented(line: &str) -> bool {
    line.starts_with("    ") || line.starts_with('\t')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_emoji_becomes_inline_span() {
        assert_eq!(
            to_render_markdown("你好 🦊 世界"),
            "你好 $emoji:1f98a$ 世界"
        );
    }

    #[test]
    fn variation_selector_is_consumed_by_the_image() {
        assert_eq!(to_render_markdown("❤️"), "$emoji:2764$");
    }

    #[test]
    fn text_presentation_is_left_alone() {
        // U+FE0E 要求以文字呈现，不该换成彩色图片
        assert_eq!(to_render_markdown("❤︎"), "❤︎");
    }

    #[test]
    fn joined_sequences_are_left_alone() {
        let family = "👨‍👩‍👧";
        assert_eq!(to_render_markdown(family), family);
        assert_eq!(to_render_markdown("👍🏻"), "👍🏻");
    }

    #[test]
    fn fenced_code_block_is_untouched() {
        let markdown = "看代码：\n\n```python\nprint(\"hi 🦊\")\n```\n\n就这些 🦊";
        let rendered = to_render_markdown(markdown);
        assert!(rendered.contains("print(\"hi 🦊\")"), "{rendered}");
        assert!(rendered.ends_with("就这些 $emoji:1f98a$"), "{rendered}");
    }

    #[test]
    fn indented_code_block_is_untouched() {
        let markdown = "例子：\n\n    echo 🦊\n\n结束 🦊";
        let rendered = to_render_markdown(markdown);
        assert!(rendered.contains("    echo 🦊"), "{rendered}");
        assert!(rendered.ends_with("结束 $emoji:1f98a$"), "{rendered}");
    }

    #[test]
    fn inline_code_is_untouched() {
        assert_eq!(
            to_render_markdown("`print('🦊')` 和 🦊"),
            "`print('🦊')` 和 $emoji:1f98a$"
        );
    }

    #[test]
    fn paragraph_with_dollar_is_untouched() {
        // 段落里有 `$` 时整段不动，免得插入的 `$` 与它配对成公式
        let text = "成本 $100 🦊";
        assert_eq!(to_render_markdown(text), text);
        // 换一段就可以照常替换
        assert_eq!(
            to_render_markdown("成本 $100\n\n🦊"),
            "成本 $100\n\n$emoji:1f98a$"
        );
    }

    #[test]
    fn carriage_returns_and_trailing_newline_survive() {
        assert_eq!(to_render_markdown("🦊\r\n"), "$emoji:1f98a$\r\n");
        assert_eq!(to_render_markdown("🦊\n"), "$emoji:1f98a$\n");
    }

    #[test]
    fn non_emoji_symbols_are_left_alone() {
        assert_eq!(to_render_markdown("100% → ok © ™"), "100% → ok © ™");
    }

    #[test]
    fn payload_round_trips() {
        assert_eq!(parse_payload("emoji:1f98a"), Some('🦊'));
        assert_eq!(parse_payload("emoji:zzz"), None);
        assert_eq!(parse_payload("emoji:41"), None, "A 不是 emoji");
        assert_eq!(parse_payload("x^2"), None);
    }

    #[test]
    fn plain_text_emoji_is_painted_as_an_image() {
        // 打字机那段走的就是这条路径
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_text(
                ui,
                "🦊好的朋友～ 🌸",
                FontId::proportional(13.5),
                Color32::WHITE,
            );
        });
        assert_eq!(
            painted_image_count(&mut output),
            2,
            "两枚 emoji 都该画成图片（不是字体图集里的字形）"
        );
    }

    #[test]
    fn markdown_emoji_is_painted_as_an_image() {
        // 整条链路：替换 → pulldown 的 InlineMath → math_fn → 位图纹理 → 画出去
        let ctx = egui::Context::default();
        let mut cache = egui_commonmark::CommonMarkCache::default();
        let rendered = to_render_markdown("🦊好的～\n\n```python\nprint(\"🦊\")\n```\n\n就这样 ✨");
        let math: &egui_commonmark::RenderMathFn = &render_math;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui_commonmark::CommonMarkViewer::new()
                .render_math_fn(Some(math))
                .show(ui, &mut cache, &rendered);
        });
        assert_eq!(
            painted_image_count(&mut output),
            2,
            "正文两枚 emoji 画成图片，代码块里那枚保持单色文字"
        );
    }

    /// 数一数这一帧画了几张图片。
    ///
    /// 文字走的是 `Shape::Text`，只有图片（我们画的 emoji）会生成网格，
    /// 所以数网格就等于数 emoji。顺带清掉纹理增量 —— epaint 不允许带着
    /// 未处理的 delta 丢帧。
    fn painted_image_count(output: &mut egui::FullOutput) -> usize {
        output.textures_delta.clear();
        let mut count = 0;
        for clipped in &output.shapes {
            count_meshes(&clipped.shape, &mut count);
        }
        count
    }

    fn count_meshes(shape: &egui::Shape, count: &mut usize) {
        match shape {
            egui::Shape::Mesh(_) => *count += 1,
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    count_meshes(shape, count);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn color_bitmap_is_decodable() {
        // 全链路：字体表 → CBDT 里的 PNG → 缩放到目标像素
        let image = rasterize('🦊', 32).expect("🦊 应该有彩色位图");
        assert_eq!(image.size[1], 32);
        assert!(image.size[0] >= 32, "应保持宽高比（位图比高度略宽）");
        assert!(
            image.pixels.iter().any(|pixel| pixel.a() > 0),
            "位图不该是全透明"
        );
    }

    #[test]
    fn texture_is_cached_per_size() {
        let ctx = egui::Context::default();
        let first = texture(&ctx, '🦊', 32).expect("应能造出纹理");
        let second = texture(&ctx, '🦊', 32).expect("应能造出纹理");
        assert_eq!(first.id(), second.id(), "同一尺寸应命中缓存");
        let other = texture(&ctx, '🦊', 64).expect("应能造出纹理");
        assert_ne!(first.id(), other.id(), "不同尺寸应是不同纹理");
    }

    #[test]
    fn characters_without_a_bitmap_report_none() {
        // 字体里没有这个字：不该崩，只该返回 None（调用方会退回单色字形）
        assert!(rasterize('A', 32).is_none());
        assert!(rasterize('中', 32).is_none());
    }
}
