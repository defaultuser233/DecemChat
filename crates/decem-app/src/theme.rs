//! 配色：对齐网页版 `src/index.css` 的 CSS 变量与组件类。
//!
//! 数值都取自那边的定义，不要凭观感改：
//!
//! - `--coral: #F38181`、`--yellow: #FCE38A`、`--accent(mint): #95E1D3`
//! - `.message-bubble-user`：`linear-gradient(135deg, #F38181, #e86a6a)` + 白字 + 圆角 `18 18 4 18`
//! - `.message-bubble-bot`：`--message-bot`（深色 `#1a1a1a`）+ 圆角 `18 18 18 4` + `1px rgba(255,255,255,.1)` 描边
//! - `.glass-effect`：`rgba(26,26,26,.8)` + `1px rgba(255,255,255,.1)`
//! - `.mask-overlay`：`rgba(0,0,0,.5)`

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// 一套界面配色。
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// 是否深色。
    pub dark: bool,
    /// 窗口底色。
    pub background: Color32,
    /// 卡片底色。
    pub card: Color32,
    /// 半透明面板底色（`.glass-effect`：头部、输入区、设置抽屉）。
    pub glass: Color32,
    /// 用户气泡底色（珊瑚渐变取中值）。
    pub bubble_user: Color32,
    /// 用户气泡文字色。
    pub bubble_user_text: Color32,
    /// Decem 气泡底色。
    pub bubble_char: Color32,
    /// 气泡描边。
    pub bubble_border: Color32,
    /// 代码块底色。
    pub code_bg: Color32,
    /// 行内代码底色（网页版 `bg-black/20` 叠在气泡上）。
    pub code_inline_bg: Color32,
    /// 输入框 / 列表项等「次一级」底色（`bg-muted/50`）。
    pub row_bg: Color32,
    /// 主色（珊瑚红 `#F38181`）。
    pub accent: Color32,
    /// 强调色（薄荷绿 `#95E1D3`）。
    pub mint: Color32,
    /// 强调色上的文字色（`--accent-foreground`）。
    pub mint_ink: Color32,
    /// 点缀色（鹅黄 `#FCE38A`）。
    pub yellow: Color32,
    /// 正文颜色。
    pub text: Color32,
    /// 次要文字颜色。
    pub muted: Color32,
    /// 面板描边（`--border`）。
    pub border: Color32,
    /// 危险操作颜色。
    pub danger: Color32,
    /// 遮罩底色（`.mask-overlay`）。
    pub mask: Color32,
    /// 开关钮颜色。
    pub knob: Color32,
}

/// 深色配色（默认）。
pub const DARK: Palette = Palette {
    dark: true,
    background: Color32::from_rgb(0, 0, 0),
    card: Color32::from_rgb(0x1a, 0x1a, 0x1a),
    glass: Color32::from_rgba_premultiplied(21, 21, 21, 204),
    // #F38181 → #e86a6a 渐变，单色填充取中值
    bubble_user: Color32::from_rgb(0xED, 0x75, 0x75),
    bubble_user_text: Color32::WHITE,
    bubble_char: Color32::from_rgb(0x1a, 0x1a, 0x1a),
    bubble_border: Color32::from_rgb(0x2e, 0x2e, 0x2e),
    code_bg: Color32::from_rgb(0x12, 0x12, 0x12),
    code_inline_bg: Color32::from_rgb(0x15, 0x15, 0x15),
    row_bg: Color32::from_rgb(0x1d, 0x1d, 0x1d),
    accent: Color32::from_rgb(0xF3, 0x81, 0x81),
    mint: Color32::from_rgb(0x95, 0xE1, 0xD3),
    mint_ink: Color32::from_rgb(0x0a, 0x0a, 0x0a),
    yellow: Color32::from_rgb(0xFC, 0xE3, 0x8A),
    text: Color32::from_rgb(0xF2, 0xF2, 0xF2),
    muted: Color32::from_rgb(0xB3, 0xB3, 0xB3),
    border: Color32::from_rgb(0x33, 0x33, 0x33),
    danger: Color32::from_rgb(0xEF, 0x53, 0x50),
    mask: Color32::from_rgba_premultiplied(0, 0, 0, 128),
    knob: Color32::from_rgb(0x0a, 0x0a, 0x0a),
};

/// 浅色配色。
pub const LIGHT: Palette = Palette {
    dark: false,
    background: Color32::from_rgb(0xF5, 0xF5, 0xF5),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    glass: Color32::from_rgba_premultiplied(255, 255, 255, 204),
    bubble_user: Color32::from_rgb(0xED, 0x75, 0x75),
    bubble_user_text: Color32::WHITE,
    bubble_char: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    bubble_border: Color32::from_rgb(0xE6, 0xE6, 0xE6),
    code_bg: Color32::from_rgb(0xF2, 0xF2, 0xF2),
    code_inline_bg: Color32::from_rgb(0xF2, 0xF2, 0xF2),
    row_bg: Color32::from_rgb(0xF2, 0xF2, 0xF2),
    accent: Color32::from_rgb(0xF3, 0x81, 0x81),
    mint: Color32::from_rgb(0x4D, 0xB6, 0xAC),
    mint_ink: Color32::from_rgb(0x1a, 0x1a, 0x1a),
    yellow: Color32::from_rgb(0xE8, 0xC5, 0x4A),
    text: Color32::from_rgb(0x1A, 0x1A, 0x1A),
    muted: Color32::from_rgb(0x66, 0x66, 0x66),
    border: Color32::from_rgb(0xD9, 0xD9, 0xD9),
    danger: Color32::from_rgb(0xD3, 0x2F, 0x2F),
    mask: Color32::from_rgba_premultiplied(0, 0, 0, 77),
    knob: Color32::WHITE,
};

/// 气泡圆角。
pub const BUBBLE_RADIUS: u8 = 18;

/// 用户气泡的圆角：四角都是圆的。
///
/// 网页版两侧气泡的靠头像那一角都会收窄成 4px 的「尾巴」
/// （用户侧右下、Decem 侧左下），用户点名说尖角不好看，所以两边都改成四角统一 ——
/// 别再照网页版改回去。
pub const USER_BUBBLE_CORNERS: CornerRadius = CornerRadius {
    nw: BUBBLE_RADIUS,
    ne: BUBBLE_RADIUS,
    sw: BUBBLE_RADIUS,
    se: BUBBLE_RADIUS,
};

/// Decem 气泡的圆角：同样四角都是圆的（见 [`USER_BUBBLE_CORNERS`]）。
pub const CHAR_BUBBLE_CORNERS: CornerRadius = CornerRadius {
    nw: BUBBLE_RADIUS,
    ne: BUBBLE_RADIUS,
    sw: BUBBLE_RADIUS,
    se: BUBBLE_RADIUS,
};

/// 输入区两个按钮的橙色（`#E95620`）。
///
/// 网页版这两个按钮是珊瑚色/灰底；这里是用户点名要的一处偏离，
/// 所以不要按网页版「改回去」。
pub const BUTTON_ORANGE: Color32 = Color32::from_rgb(0xE9, 0x56, 0x20);

impl Palette {
    /// 整块按透明度淡出（对应网页版的 `opacity` 动画）。
    pub fn faded(self, factor: f32) -> Self {
        let f = factor.clamp(0.0, 1.0);
        let fade = |c: Color32| c.gamma_multiply(f);
        Self {
            background: fade(self.background),
            card: fade(self.card),
            glass: fade(self.glass),
            bubble_user: fade(self.bubble_user),
            bubble_user_text: fade(self.bubble_user_text),
            bubble_char: fade(self.bubble_char),
            bubble_border: fade(self.bubble_border),
            code_bg: fade(self.code_bg),
            code_inline_bg: fade(self.code_inline_bg),
            row_bg: fade(self.row_bg),
            accent: fade(self.accent),
            mint: fade(self.mint),
            mint_ink: fade(self.mint_ink),
            yellow: fade(self.yellow),
            text: fade(self.text),
            muted: fade(self.muted),
            border: fade(self.border),
            danger: fade(self.danger),
            mask: fade(self.mask),
            knob: fade(self.knob),
            ..self
        }
    }
}

/// 取一套配色。
pub fn palette(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

/// 把配色应用到 egui 全局样式。
pub fn apply(ctx: &egui::Context, palette: Palette) {
    let mut visuals = if palette.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.card;
    // 代码块背景由 `visuals.extreme_bg_color` 决定（egui_commonmark_backend::elements::code_block）
    visuals.extreme_bg_color = palette.code_bg;
    visuals.override_text_color = Some(palette.text);
    visuals.hyperlink_color = palette.accent;
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.45);
    visuals.window_stroke = Stroke::new(1.0, palette.border);

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.muted);
    widgets.noninteractive.corner_radius = CornerRadius::same(8);
    widgets.inactive.bg_fill = palette.card;
    // 按钮底色（网页版的 bg-muted/50）
    widgets.inactive.weak_bg_fill = palette.row_bg;
    widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
    widgets.inactive.corner_radius = CornerRadius::same(8);
    widgets.hovered.corner_radius = CornerRadius::same(8);
    widgets.active.corner_radius = CornerRadius::same(8);
    widgets.open.corner_radius = CornerRadius::same(8);
    widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    widgets.hovered.bg_fill = palette.row_bg;
    widgets.hovered.weak_bg_fill = palette.row_bg;
    widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    widgets.active.bg_fill = palette.accent.gamma_multiply(0.6);
    widgets.active.weak_bg_fill = palette.accent.gamma_multiply(0.6);
    widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
    widgets.open.bg_fill = palette.card;

    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.scroll = egui::style::ScrollStyle::solid();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubbles_have_no_sharp_corner() {
        // 用户点名要的：两侧气泡都不要那个 4px 的「尾巴」（网页版靠头像那一角会收窄）。
        // 谁要是照网页版「改回去」，这条会拦下来。
        for corners in [USER_BUBBLE_CORNERS, CHAR_BUBBLE_CORNERS] {
            assert_eq!(corners.nw, BUBBLE_RADIUS);
            assert_eq!(corners.ne, BUBBLE_RADIUS);
            assert_eq!(corners.sw, BUBBLE_RADIUS);
            assert_eq!(corners.se, BUBBLE_RADIUS);
        }
    }
}
