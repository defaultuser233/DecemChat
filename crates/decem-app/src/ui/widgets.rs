//! 自绘控件：图标按钮与开关。
//!
//! egui 没有图标字体与开关控件，这里照网页版的观感自己画。

use egui::{Color32, CornerRadius, Response, Sense, Vec2};

use crate::theme::Palette;

use super::icons::{self, Icon};

/// 图标按钮：默认无底色，hover 时铺一层淡色（对应网页版的 `hover:bg-muted`）。
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    box_size: f32,
    color: Color32,
    hover_bg: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(box_size), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same((box_size * 0.25) as u8), hover_bg);
    }
    icons::draw(ui.painter(), icon, rect.shrink(box_size * 0.24), color, 1.0);
    response
}

/// 关掉「文字可选中」。
///
/// 按钮和可点行里的文字应该像网页版那样不可选（`src/App.css:32` 的
/// `button, .no-select { user-select: none }`）。不关的话有两个毛病：
/// 鼠标停在按钮文字上会变成输入态光标；按下时还可能被「拖选文字」抢走，
/// 于是点了没反应 —— 也就是「有时选不中」。
pub fn no_text_selection(ui: &mut egui::Ui) {
    ui.style_mut().interaction.selectable_labels = false;
}

/// 有底色的图标按钮（发送键那种）。
pub fn icon_button_filled(
    ui: &mut egui::Ui,
    icon: Icon,
    box_size: f32,
    fill: Color32,
    icon_color: Color32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(box_size), Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::same((box_size * 0.3) as u8), fill);
    icons::draw(
        ui.painter(),
        icon,
        rect.shrink(box_size * 0.26),
        icon_color,
        1.0,
    );
    response
}

/// 「图标 + 文字」按钮：居中对齐，hover 时底色变亮。
pub fn button_with_icon(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    size: Vec2,
    style: ButtonStyle,
    palette: Palette,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.hovered() && style.hover_boost > 0.0 {
        style.fill.gamma_multiply(1.0 + style.hover_boost)
    } else {
        style.fill
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    if style.border {
        painter.rect_stroke(
            rect,
            CornerRadius::same(8),
            egui::Stroke::new(1.0, palette.border),
            egui::StrokeKind::Inside,
        );
    }

    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(13.0),
        style.text,
    );
    let icon_side = 16.0;
    let gap = 8.0;
    let total = icon_side + gap + galley.size().x;
    let start = rect.center().x - total * 0.5;
    icons::draw(
        painter,
        icon,
        egui::Rect::from_min_size(
            egui::pos2(start, rect.center().y - icon_side * 0.5),
            Vec2::splat(icon_side),
        ),
        style.text,
        1.0,
    );
    painter.galley(
        egui::pos2(
            start + icon_side + gap,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        style.text,
    );
    response
}

/// 有底色、内容为旋转指示器的按钮（网页版发送键在加载时的样子）。
pub fn icon_button_spinner(
    ui: &mut egui::Ui,
    box_size: f32,
    fill: Color32,
    icon_color: Color32,
    angle: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(box_size), Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::same((box_size * 0.3) as u8), fill);
    icons::draw_rotated(
        ui.painter(),
        Icon::Loader,
        rect.shrink(box_size * 0.28),
        icon_color,
        1.0,
        angle,
    );
    response
}

/// 「图标 + 文字」按钮的外观。
pub struct ButtonStyle {
    /// 底色。
    pub fill: Color32,
    /// 文字与图标颜色。
    pub text: Color32,
    /// 是否画描边。
    pub border: bool,
    /// hover 时底色提亮倍数（0 表示不变）。
    pub hover_boost: f32,
}

/// 下拉框触发器：整行圆角矩形 + 左对齐的值 + 右侧 lucide chevron。
pub fn select_trigger(ui: &mut egui::Ui, text: &str, palette: Palette) -> Response {
    let size = Vec2::new(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.hovered() {
        palette.border.gamma_multiply(0.8)
    } else {
        palette.row_bg
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        egui::Stroke::new(1.0, palette.border),
        egui::StrokeKind::Inside,
    );

    let galley = painter.layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(14.0),
        palette.text,
    );
    painter.galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y * 0.5),
        galley,
        palette.text,
    );

    let icon_side = 16.0;
    icons::draw(
        painter,
        Icon::ChevronDown,
        egui::Rect::from_center_size(
            egui::pos2(rect.right() - icon_side * 0.75 - 4.0, rect.center().y),
            Vec2::splat(icon_side),
        ),
        palette.muted,
        1.0,
    );
    response
}

/// 开关：胶囊轨道 + 圆钮，点击切换。
pub fn switch(ui: &mut egui::Ui, on: &mut bool, palette: Palette) -> Response {
    let size = Vec2::new(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    // 位置按动画进度插值，切换时不会“跳”
    let progress = ui.ctx().animate_bool(response.id, *on);
    let radius = rect.height() * 0.5;
    let track = if *on { palette.accent } else { palette.border };
    ui.painter().rect_filled(rect, radius, track);

    let knob_radius = radius - 3.0;
    let left = rect.left() + radius;
    let right = rect.right() - radius;
    let center = egui::pos2(left + (right - left) * progress, rect.center().y);
    ui.painter()
        .circle_filled(center, knob_radius, palette.knob);

    response
}
