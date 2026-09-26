//! 消息列表：气泡、头像、逐字光标、时间戳与「重新生成」。

use decem_core::{ChatSettings, Message, MessageRole};
use egui::{Align, CornerRadius, Frame, Layout, Margin, Rect, RichText, Stroke, Vec2};

use crate::app::{BubbleWidthKey, DecemApp};
use crate::theme::{self, Palette};
use crate::ui::icons::{self, Icon};
use crate::ui::widgets;

use super::{
    AVATAR_GAP, AVATAR_SIZE, Action, BUBBLE_WIDTH, CONTENT_WIDTH, MESSAGE_GAP,
    textures::TextureCache,
};

/// 消息图片的最大显示高度。
const IMAGE_MAX_HEIGHT: f32 = 192.0;
/// 入场动画时长（网页版 `animate-slide-up` 为 0.5s）。
const ENTRANCE_SECONDS: f64 = 0.5;
/// 入场时上浮的像素数。
const ENTRANCE_RISE: f32 = 20.0;
/// 气泡内边距（网页版 `px-4 py-3` = 水平 16、垂直 12）。
const BUBBLE_PADDING: f32 = 16.0;
/// 气泡垂直内边距。
const BUBBLE_PADDING_Y: f32 = 12.0;
/// 气泡内容的最小宽度：网页版没有下限，这里只防极短的怪异内容把气泡压成一条。
const BUBBLE_MIN_CONTENT: f32 = 16.0;
/// 头像悬停放大倍数（网页版 `hover:scale-110`）。
const AVATAR_HOVER_SCALE: f32 = 1.1;

/// 一行的绘制参数。
struct Row<'a> {
    message: &'a Message,
    avatar: &'a str,
    is_user: bool,
    is_last_char: bool,
    waiting: bool,
    typing: bool,
    palette: Palette,
}

/// 绘制消息区域。
pub fn show(ui: &mut egui::Ui, app: &mut DecemApp, palette: Palette, actions: &mut Vec<Action>) {
    let stick_to_bottom = app.stick_to_bottom || app.is_busy();
    let waiting_id = app.streaming_message_id().map(str::to_owned);
    let typing_id = app.typing_message_id().map(str::to_owned);

    egui::CentralPanel::default()
        .frame(Frame::new().fill(palette.background))
        .show(ui, |ui| {
            background_glow(ui, palette);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(stick_to_bottom)
                .show(ui, |ui| {
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        // 设置面板展开时可用宽度会变小，不能按固定宽度居中，否则会溢出
                        ui.set_max_width(CONTENT_WIDTH.min(ui.available_width()));
                        let DecemApp {
                            messages,
                            settings,
                            textures,
                            markdown,
                            measure,
                            bubble_widths,
                            seen_at,
                            ..
                        } = app;
                        let now = ui.input(|input| input.time);

                        let last_char = messages
                            .iter()
                            .rposition(|message| message.role == MessageRole::Assistant);

                        for (index, message) in messages.iter().enumerate() {
                            let is_user = message.role == MessageRole::User;
                            let waiting = waiting_id.as_deref() == Some(message.id.as_str());

                            // 网页版的 `animate-slide-up`：上浮 20px + 淡入，0.5s 缓出
                            let born = *seen_at.entry(message.id.clone()).or_insert(now);
                            let (alpha, rise) = entrance(now, born);
                            if rise > 0.0 {
                                ui.add_space(rise);
                                ui.ctx().request_repaint();
                            }

                            let row = Row {
                                message,
                                avatar: avatar_for(settings, is_user),
                                is_user,
                                is_last_char: last_char == Some(index),
                                waiting: waiting && message.content.is_empty(),
                                typing: typing_id.as_deref() == Some(message.id.as_str()),
                                palette: palette.faded(alpha),
                            };
                            draw_row(
                                ui,
                                &row,
                                textures,
                                markdown,
                                measure,
                                bubble_widths,
                                actions,
                            );
                        }
                    });
                    ui.add_space(16.0);
                });
        });
}

fn avatar_for(settings: &ChatSettings, is_user: bool) -> &str {
    if is_user {
        &settings.user_avatar
    } else {
        &settings.char_avatar
    }
}

fn draw_row(
    ui: &mut egui::Ui,
    row: &Row<'_>,
    textures: &mut TextureCache,
    markdown: &mut egui_commonmark::CommonMarkCache,
    measure: &mut egui_commonmark::CommonMarkCache,
    bubble_widths: &mut std::collections::HashMap<BubbleWidthKey, f32>,
    actions: &mut Vec<Action>,
) {
    let layout = if row.is_user {
        Layout::right_to_left(Align::TOP)
    } else {
        Layout::left_to_right(Align::TOP)
    };

    ui.with_layout(layout, |ui| {
        draw_avatar(ui, row, textures);
        ui.add_space(AVATAR_GAP);
        ui.vertical(|ui| {
            let align = if row.is_user {
                Align::RIGHT
            } else {
                Align::LEFT
            };
            ui.with_layout(Layout::top_down(align), |ui| {
                draw_bubble(ui, row, textures, markdown, measure, bubble_widths);
                draw_footer(ui, row, actions);
            });
        });
    });
    ui.add_space(MESSAGE_GAP);
}

fn draw_avatar(ui: &mut egui::Ui, row: &Row<'_>, textures: &mut TextureCache) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(AVATAR_SIZE), egui::Sense::hover());
    let Some(texture) = textures.avatar(ui.ctx(), row.avatar) else {
        return;
    };

    let scale = if response.hovered() {
        AVATAR_HOVER_SCALE
    } else {
        1.0
    };
    let size = AVATAR_SIZE * scale;
    let target = Rect::from_center_size(rect.center(), Vec2::splat(size));
    ui.put(
        target,
        egui::Image::from_texture(egui::load::SizedTexture::from_handle(&texture))
            .fit_to_exact_size(Vec2::splat(size))
            .corner_radius(size / 2.0),
    );

    let ring = if row.is_user {
        row.palette.accent
    } else {
        row.palette.mint
    };
    ui.painter()
        .circle_stroke(rect.center(), size / 2.0 - 1.0, Stroke::new(2.0, ring));
}

fn draw_bubble(
    ui: &mut egui::Ui,
    row: &Row<'_>,
    textures: &mut TextureCache,
    markdown: &mut egui_commonmark::CommonMarkCache,
    measure: &mut egui_commonmark::CommonMarkCache,
    bubble_widths: &mut std::collections::HashMap<BubbleWidthKey, f32>,
) {
    let palette = row.palette;
    let (fill, text_color, corners, border) = if row.is_user {
        (
            palette.bubble_user,
            palette.bubble_user_text,
            theme::USER_BUBBLE_CORNERS,
            Stroke::NONE,
        )
    } else {
        (
            palette.bubble_char,
            palette.text,
            theme::CHAR_BUBBLE_CORNERS,
            Stroke::new(1.0, palette.bubble_border),
        )
    };

    Frame::new()
        .fill(fill)
        .corner_radius(corners)
        .inner_margin(Margin::symmetric(
            BUBBLE_PADDING as i8,
            BUBBLE_PADDING_Y as i8,
        ))
        .stroke(border)
        .show(ui, |ui| {
            // 上限放在这里：给外层 Ui 设宽度会在反向布局里把气泡锚到另一侧
            ui.set_max_width(BUBBLE_WIDTH - 2.0 * BUBBLE_PADDING);

            if let Some(url) = row.message.image_url.as_deref() {
                draw_message_image(ui, &format!("image:{}", row.message.id), url, textures);
            }

            if row.waiting {
                ui.horizontal(|ui| {
                    // 网页版是 <Loader2 className="animate-spin"/>，这里按时间旋转同一个图标
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), egui::Sense::hover());
                    let angle = ui.input(|input| input.time) as f32 * std::f32::consts::TAU;
                    icons::draw_rotated(ui.painter(), Icon::Loader, rect, text_color, 1.0, angle);
                    ui.label(
                        RichText::new("狐狐正在思考...")
                            .size(13.0)
                            .color(text_color),
                    );
                });
            } else if row.typing {
                // 打字机阶段：保持纯文本，末尾跟一个闪烁光标
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    crate::emoji::show_text(
                        ui,
                        &row.message.content,
                        egui::FontId::proportional(13.5),
                        text_color,
                    );
                    if cursor_visible(ui) {
                        ui.label(RichText::new("▌").size(14.0).color(palette.accent));
                    }
                });
            } else if row.message.content.is_empty() {
                // 只发了一张图片时不要写「(空消息)」：图就是全部内容
                if row.message.image_url.is_none() {
                    ui.label(
                        RichText::new("(空消息)")
                            .size(12.0)
                            .color(text_color.gamma_multiply(0.7)),
                    );
                }
            } else {
                // emoji 先换成内联 span：量宽度与实际渲染必须用同一份文本
                let rendered = crate::emoji::to_render_markdown(&row.message.content);
                let width = content_width(ui, measure, bubble_widths, &rendered, palette);
                draw_markdown(ui, markdown, &rendered, palette, width);
            }
        });
}

/// 渲染 Markdown。
///
/// 代码块的底色来自 `visuals.extreme_bg_color`、圆角来自 `noninteractive().corner_radius`，
/// 因此在一个作用域里临时改成网页版 `pre` 的样式（`rounded-lg` + 无描边）。
fn draw_markdown(
    ui: &mut egui::Ui,
    markdown: &mut egui_commonmark::CommonMarkCache,
    rendered: &str,
    palette: Palette,
    width: f32,
) {
    // 取整要向上：`default_width` 收的是整数像素，向下取整会让最右边一个字挤到下一行
    let width = width.ceil();
    ui.scope(|ui| {
        ui.set_max_width(width);
        show_markdown(ui, markdown, rendered, palette, Some(width));
    });
}

/// 正文那段 Markdown 的样式与渲染器设置。
///
/// 量宽度与实际渲染共用同一套设置，否则量出来的宽度和真正的排版对不上。
fn show_markdown(
    ui: &mut egui::Ui,
    markdown: &mut egui_commonmark::CommonMarkCache,
    rendered: &str,
    palette: Palette,
    width: Option<f32>,
) {
    // 网页版给段落设的是 `m-0 mb-1`（段间 4px）
    ui.spacing_mut().item_spacing.y = 4.0;
    // 代码块与行内代码都用 `text-xs`（12px），比正文小一号
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.0));

    let visuals = ui.visuals_mut();
    visuals.extreme_bg_color = palette.code_bg;
    // 行内代码底色（网页版 `bg-black/20`）
    visuals.code_bg_color = palette.code_inline_bg;
    // 全局的 `override_text_color` 会把正文颜色「烧死」在文字上，
    // 链接就再也拿不到 `hyperlink_color`（会显示成白色）。这里让颜色走常规解析：
    // 正文改用非交互前景色（同一个 #f2f2f2），链接恢复成主题里的珊瑚色。
    visuals.override_text_color = None;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
    // 表格外框与 `---` 分隔线都用这个描边，不能设成 NONE，
    // 否则表格没有边框、分隔线直接消失
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);
    // 表格外框与斑马纹底色（网页版表头是 `bg-muted`）
    visuals.faint_bg_color = palette.row_bg;

    // emoji 走图片：先把它们换成内联 span，再用 math_fn 回调画出来
    let math_fn: &egui_commonmark::RenderMathFn = &crate::emoji::render_math;
    egui_commonmark::CommonMarkViewer::new()
        .render_math_fn(Some(math_fn))
        .default_width(width.map(|width| width as usize))
        // 网页版明暗模式都用同一份 tokyo-night 配色
        .syntax_theme_dark(crate::app::CODE_THEME_NAME)
        .syntax_theme_light(crate::app::CODE_THEME_NAME)
        .show(ui, markdown, rendered);
}

/// 量出内容最宽一行的宽度，用来让气泡贴合文字而不是铺满整列。
///
/// 网页版的气泡是 `inline-block`：宽度 = `min(最宽一行, 上限)`。
/// 这里用**屏幕外的一次真实渲染**量出那个「最宽一行」，所以标题、列表、表格、
/// 代码块、emoji 都是准的 —— 早先用字号估算再补经验值，短消息会多出一截空白。
/// 量过的内容按（内容哈希, DPI）缓存，正常帧不会多渲染一遍。
fn content_width(
    ui: &mut egui::Ui,
    measure: &mut egui_commonmark::CommonMarkCache,
    cache: &mut std::collections::HashMap<BubbleWidthKey, f32>,
    rendered: &str,
    palette: Palette,
) -> f32 {
    let key = (
        content_hash(rendered),
        (ui.ctx().pixels_per_point() * 100.0).round() as u32,
        layout_fingerprint(ui),
    );
    let natural = match cache.get(&key) {
        Some(width) => *width,
        None => {
            let width = measure_natural_width(ui, measure, rendered, palette);
            cache.insert(key, width);
            width
        }
    };
    natural.clamp(BUBBLE_MIN_CONTENT, BUBBLE_WIDTH - 2.0 * BUBBLE_PADDING)
}

/// 屏幕外渲染一遍，量「不换行时最宽一行」的宽度。
fn measure_natural_width(
    ui: &mut egui::Ui,
    markdown: &mut egui_commonmark::CommonMarkCache,
    rendered: &str,
    palette: Palette,
) -> f32 {
    // 就地量：起点必须和真正渲染时一致。字形坐标会按像素取整，换个起点
    // 累计误差能差出一两个像素，最右边那个字就会莫名其妙折行。
    // 不画出来（透明度 0），只走布局。
    // 高度必须给足：可用高度为 0 时 egui 会按高度限制行数，把文本截断，
    // 量出来的宽度就偏小，真正渲染时最右边那个字又会折行。
    let rect = Rect::from_min_size(
        egui::pos2(ui.max_rect().min.x, ui.max_rect().top()),
        Vec2::new(20_000.0, 20_000.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::LEFT)),
    );
    child.set_opacity(0.0);
    show_markdown(&mut child, markdown, rendered, palette, None);
    (child.min_rect().right() - rect.min.x).max(0.0)
}

/// 排版环境指纹：字体（含回退字体）的度量一变，这个值就变。
///
/// 第一趟渲染时字体还没生效（`set_fonts` 要下一趟才用上），同一段文字量出来的
/// 宽度比之后小一大截 —— 指纹进缓存键，才不会把那个偏小的值一直用下去。
fn layout_fingerprint(ui: &egui::Ui) -> u32 {
    let font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(|| egui::FontId::proportional(14.0));
    let probe =
        ui.painter()
            .layout_no_wrap(FINGERPRINT_PROBE.to_owned(), font, egui::Color32::WHITE);
    (probe.size().x * 64.0).round() as u32
}

/// 指纹探针：一个汉字 + 一个 emoji + 一个拉丁字母，覆盖三条回退链。
const FINGERPRINT_PROBE: &str = "字🦊A";

fn content_hash(content: &str) -> u64 {
    use std::hash::{Hash as _, Hasher as _};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// 背景里三个柔和光斑（网页版那三个 `animate-pulse-glow` 渐变圆）。
///
/// egui 没有模糊，用多层递减透明度的大圆近似。
fn background_glow(ui: &egui::Ui, palette: Palette) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    for (rel, radius, color) in [
        (Vec2::new(0.28, 0.24), 0.44, palette.accent),
        (Vec2::new(0.78, 0.72), 0.40, palette.mint),
        (Vec2::new(0.56, 0.46), 0.30, palette.yellow),
    ] {
        let center = rect.lerp_inside(rel);
        let base = rect.width().min(rect.height()) * radius;
        // 层数多、每层更淡，接近网页版 blur(60px) 的柔和过渡
        for step in 0..24 {
            let t = step as f32 / 24.0;
            let alpha = (1.0 - t).powi(3) * 0.010;
            painter.circle_filled(center, base * (1.0 - t * 0.92), color.gamma_multiply(alpha));
        }
    }
}

fn draw_message_image(ui: &mut egui::Ui, key: &str, data_url: &str, textures: &mut TextureCache) {
    let Some(texture) = textures.data_url(ui.ctx(), key, data_url) else {
        return;
    };
    let size = texture.size_vec2();
    let scale = (IMAGE_MAX_HEIGHT / size.y).min(1.0);
    ui.add(
        egui::Image::from_texture(egui::load::SizedTexture::from_handle(&texture))
            .fit_to_exact_size(size * scale)
            .corner_radius(8.0),
    );
    ui.add_space(6.0);
}

fn draw_footer(ui: &mut egui::Ui, row: &Row<'_>, actions: &mut Vec<Action>) {
    if row.waiting {
        return;
    }
    let palette = row.palette;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        ui.label(
            RichText::new(format_time(row.message.timestamp))
                .size(12.0)
                .color(palette.muted),
        );
        if row.is_last_char && !row.is_user && !row.typing {
            let icon_clicked =
                widgets::icon_button(ui, Icon::Refresh, 18.0, palette.accent, palette.row_bg)
                    .on_hover_text("重新生成")
                    .clicked();
            let text_clicked = ui
                .add(
                    egui::Label::new(RichText::new("重新生成").size(12.0).color(palette.accent))
                        .selectable(false)
                        .sense(egui::Sense::click()),
                )
                .clicked();
            if icon_clicked || text_clicked {
                actions.push(Action::Regenerate);
            }
        }
    });
}

/// 入场动画进度：返回（透明度倍数，需要下移的像素）。
///
/// 对应网页版 `@keyframes slide-up`（`translateY(20px)` + `opacity: 0` → 1），
/// 缓动取 `cubic-bezier(0.16, 1, 0.3, 1)` 的近似。
fn entrance(now: f64, born: f64) -> (f32, f32) {
    let progress = (((now - born) / ENTRANCE_SECONDS) as f32).clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - progress).powi(3);
    (eased, (1.0 - eased) * ENTRANCE_RISE)
}

/// 光标闪烁：0.8 秒一个周期。
fn cursor_visible(ui: &egui::Ui) -> bool {
    let time = ui.input(|input| input.time);
    (time / 0.8).fract() < 0.5
}

/// 毫秒时间戳格式化成本地 `HH:MM`。
fn format_time(timestamp_ms: i64) -> String {
    let Ok(timestamp) = jiff::Timestamp::from_millisecond(timestamp_ms) else {
        return String::new();
    };
    timestamp
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime("%H:%M")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_is_formatted_as_hh_mm() {
        let text = format_time(1_700_000_000_000);
        assert_eq!(text.len(), 5, "应为 HH:MM，实际为 {text}");
        assert_eq!(text.as_bytes()[2], b':');
    }

    /// 量一组内容的宽度（走与正文一样的替换与渲染路径）。
    fn widths(contents: &[&str]) -> Vec<f32> {
        let ctx = egui::Context::default();
        let mut measure = egui_commonmark::CommonMarkCache::default();
        let mut cache = std::collections::HashMap::new();
        let palette = crate::theme::palette(true);
        let mut widths = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for content in contents {
                let rendered = crate::emoji::to_render_markdown(content);
                widths.push(content_width(
                    ui,
                    &mut measure,
                    &mut cache,
                    &rendered,
                    palette,
                ));
            }
        });
        output.textures_delta.clear();
        widths
    }

    #[test]
    fn bubble_hugs_short_text() {
        // 网页版气泡是 inline-block：一个字的宽度就是一个字。
        // 早先按字号估算再 +16 经验值，短消息右侧会多出一截空白。
        let [one, two] = widths(&["咪", "咪咪"])[..] else {
            panic!("应量出两个宽度");
        };
        assert!(one < 20.0, "单字气泡不该被撑宽：{one}");
        assert!(two > one, "两字应比一字宽：{one} -> {two}");
    }

    #[test]
    fn bubble_width_accounts_for_emoji() {
        // emoji 是图片（约 1.5 em 宽），比同字号的中文方块更宽
        let [text, emoji] = widths(&["咪", "🦊"])[..] else {
            panic!("应量出两个宽度");
        };
        assert!(emoji > text, "emoji 占位应比中文字更宽：{text} -> {emoji}");
    }

    #[test]
    fn long_content_is_clamped_to_the_bubble_limit() {
        let long = "很长的消息".repeat(80);
        let sentence = "很长的消息".to_owned();
        let [short, long] = widths(&[&sentence, &long])[..] else {
            panic!("应量出两个宽度");
        };
        let limit = BUBBLE_WIDTH - 2.0 * BUBBLE_PADDING;
        assert!(long > short, "长内容应更宽");
        assert!(long <= limit, "宽度要被气泡上限截住：{long} > {limit}");
    }

    #[test]
    fn identical_content_reuses_the_measurement() {
        let first = widths(&["咪咪"])[0];
        let second = widths(&["咪咪"])[0];
        assert!(
            (first - second).abs() < f32::EPSILON,
            "同样的内容应量出同样的宽度"
        );
    }

    #[test]
    fn bubble_width_is_measured_the_same_in_a_right_aligned_row() {
        // 用户消息那一行是右对齐的：量宽度不能被父级布局带偏
        let ctx = egui::Context::default();
        let mut measure = egui_commonmark::CommonMarkCache::default();
        let mut cache = std::collections::HashMap::new();
        let palette = crate::theme::palette(true);
        let mut widths = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                for content in ["咪", "咪咪"] {
                    let rendered = crate::emoji::to_render_markdown(content);
                    widths.push(content_width(
                        ui,
                        &mut measure,
                        &mut cache,
                        &rendered,
                        palette,
                    ));
                }
            });
        });
        output.textures_delta.clear();
        assert!(widths[0] < 20.0, "右对齐的行里也要量准：{widths:?}");
        assert!(widths[1] > widths[0], "两字应比一字宽：{widths:?}");
    }

    #[test]
    fn invalid_timestamp_is_blank() {
        assert_eq!(format_time(i64::MIN), "");
    }

    #[test]
    fn entrance_animation_starts_hidden_and_ends_settled() {
        // 刚出现：完全透明、下移 20px
        let (alpha, rise) = entrance(10.0, 10.0);
        assert_eq!(alpha, 0.0);
        assert_eq!(rise, ENTRANCE_RISE);

        // 动画结束：不透明、不再偏移
        let (alpha, rise) = entrance(10.0 + ENTRANCE_SECONDS, 10.0);
        assert_eq!(alpha, 1.0);
        assert_eq!(rise, 0.0);

        // 中途单调上浮、逐渐变清晰
        let mut previous = -1.0;
        for step in 0..=10 {
            let elapsed = ENTRANCE_SECONDS * f64::from(step) / 10.0;
            let (alpha, rise) = entrance(10.0 + elapsed, 10.0);
            assert!((0.0..=1.0).contains(&alpha));
            assert!(alpha >= previous, "透明度应单调递增");
            assert!(rise >= 0.0);
            previous = alpha;
        }
    }
}
