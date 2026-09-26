//! 设置面板：网页版右侧抽屉的等价实现。
//!
//! 网页版抽屉是 `fixed right-0` 覆盖在页面之上，并配一层 `.mask-overlay` 把聊天区压暗。
//! 这里用 `Area`（Foreground 层）做同样的覆盖式抽屉：聊天区不再被挤窄，点击遮罩即关闭。

use decem_core::{
    AIModel, AVAILABLE_MODELS, ChatSettings, find_model, is_vision_model, model_display_name,
};
use egui::{Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke, Vec2};

use crate::app::DecemApp;
use crate::theme::Palette;
use crate::ui::icons::{self, Icon};
use crate::ui::widgets;

use super::Action;

/// 抽屉宽度（网页版 `w-80`）。
const DRAWER_WIDTH: f32 = 320.0;
/// 头像预览边长。
const AVATAR_PREVIEW: f32 = 64.0;
/// 小节标题图标大小。
const LABEL_ICON: f32 = 14.0;

/// 绘制遮罩与抽屉。
pub fn show(ui: &mut egui::Ui, app: &mut DecemApp, palette: Palette, actions: &mut Vec<Action>) {
    let ctx = ui.ctx().clone();
    show_mask(&ctx, palette, actions);
    show_drawer(&ctx, app, palette, actions);
}

/// 遮罩：点击关闭抽屉（网页版 `.mask-overlay`）。
fn show_mask(ctx: &egui::Context, palette: Palette, actions: &mut Vec<Action>) {
    let screen = ctx.viewport_rect();
    egui::Area::new(egui::Id::new("settings-mask"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, palette.mask);
            if response.clicked() {
                actions.push(Action::ToggleSettings);
            }
        });
}

fn show_drawer(
    ctx: &egui::Context,
    app: &mut DecemApp,
    palette: Palette,
    actions: &mut Vec<Action>,
) {
    let height = ctx.viewport_rect().height();
    egui::Area::new(egui::Id::new("settings-drawer"))
        // 必须比遮罩那一层高：egui 会「点哪个 Area 就把哪个提到最前」（area.rs 的 move_to_top），
        // 两者同层的话，只要有一次点在抽屉空白处落到遮罩上，遮罩就跑到抽屉前面，
        // 之后在抽屉里怎么点都点到遮罩（把面板关掉）—— 用户报的「面板有时点不了」就是这个。
        .order(egui::Order::Foreground)
        // 抽屉自己也要吃掉点击：不然点空白处会漏到下面的遮罩上，把面板关掉。
        // 模型下拉（Order::Foreground，晚于抽屉创建）仍然盖在抽屉上面。
        .sense(egui::Sense::click())
        .anchor(egui::Align2::RIGHT_TOP, Vec2::ZERO)
        .show(ctx, |ui| {
            Frame::new()
                .fill(palette.glass)
                .inner_margin(Margin::symmetric(16, 14))
                .stroke(Stroke::new(1.0, palette.border))
                .show(ui, |ui| {
                    ui.set_width(DRAWER_WIDTH);
                    ui.set_min_height(height - 28.0);

                    let DecemApp {
                        settings,
                        textures,
                        hovered_model,
                        ..
                    } = app;

                    // 标题行
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("设置")
                                .size(18.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::icon_button(
                                ui,
                                Icon::Close,
                                30.0,
                                palette.muted,
                                palette.row_bg,
                            )
                            .on_hover_text("关闭")
                            .clicked()
                            {
                                actions.push(Action::ToggleSettings);
                            }
                        });
                    });
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("settings-scroll")
                        .show(ui, |ui| {
                            model_section(ui, settings, hovered_model, palette, actions);
                            divider(ui, palette);
                            avatar_section(ui, settings, textures, palette, actions);
                            divider(ui, palette);
                            theme_section(ui, settings, palette, actions);
                            divider(ui, palette);
                            clear_section(ui, palette, actions);
                            footer(ui, palette);
                        });
                });
        });
}

// ---------------------------------------------------------------- 模型

fn model_section(
    ui: &mut egui::Ui,
    settings: &mut ChatSettings,
    hovered_model: &mut Option<String>,
    palette: Palette,
    actions: &mut Vec<Action>,
) {
    label(ui, Icon::Bot, "切换模型", palette.text, palette.accent);
    ui.add_space(4.0);

    let selected_text = model_display_name(&settings.model);
    let trigger = widgets::select_trigger(ui, &selected_text, palette);
    egui::Popup::menu(&trigger)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(360.0);
            for model in AVAILABLE_MODELS {
                let selected = settings.model == model.id;
                if model_row(ui, model, selected, hovered_model, palette).clicked() {
                    actions.push(Action::SetModel(model.id.to_owned()));
                }
            }
        });

    if let Some(model) = find_model(&settings.model) {
        ui.add_space(4.0);
        crate::emoji::wrapped_text(
            ui,
            model.description,
            egui::FontId::proportional(11.0),
            palette.muted,
        );
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("当前：{selected_text}"))
                .size(11.0)
                .color(palette.muted),
        );
        if is_vision_model(&settings.model) {
            badge(ui, palette);
        }
    });
}

/// 下拉里的一行：模型名 + 视觉标记，第二行是描述。
///
/// 选中与悬停是同一种样式：薄荷底色 + 近黑文字，与网页版的
/// `focus:bg-accent focus:text-accent-foreground`（`--accent` 就是薄荷）一致；
/// 两者的区别只剩行尾那个对勾。
fn model_row(
    ui: &mut egui::Ui,
    model: &AIModel,
    selected: bool,
    hovered_model: &mut Option<String>,
    palette: Palette,
) -> egui::Response {
    // 悬停色要提前定下来，而 egui 的 hover 得等控件画完才知道 —— 用上一帧记下的结果。
    // 所以「谁被悬停」只能记一个：本行不悬停时不要动这个字段，
    // 否则后面几行会把前面记下的值覆盖掉，只有最后一行能高亮。
    let was_hovered = hovered_model.as_deref() == Some(model.id);
    let highlighted = selected || was_hovered;

    let (name_color, desc_color) = if highlighted {
        (palette.mint_ink, palette.mint_ink.gamma_multiply(0.75))
    } else {
        (palette.text, palette.muted)
    };
    let fill = if highlighted {
        palette.mint
    } else {
        Color32::TRANSPARENT
    };
    // 「视觉理解」与那枚眼睛：网页版固定是薄荷色；压在薄荷底上时改用墨色，否则看不见
    let accent_color = if highlighted {
        palette.mint_ink
    } else {
        palette.mint
    };

    let response = Frame::new()
        .fill(fill)
        .corner_radius(8)
        .inner_margin(Margin::symmetric(10, 7))
        .show(ui, |ui| {
            // 行里的文字不参与选择：光标不会变成输入态，点击也不会被「选文字」抢走
            widgets::no_text_selection(ui);
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(model.name).size(13.5).color(name_color));
                    if model.supports_vision {
                        icon(ui, Icon::Eye, 11.0, accent_color);
                        ui.label(RichText::new("视觉理解").size(10.0).color(accent_color));
                    }
                    if selected {
                        // 网页版 Select 的选中标记在行尾
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            icon(ui, Icon::Check, 16.0, name_color);
                        });
                    }
                });
                crate::emoji::wrapped_text(
                    ui,
                    model.description,
                    egui::FontId::proportional(11.0),
                    desc_color,
                );
            });
        })
        .response
        .interact(Sense::click());

    let now_hovered = response.hovered();
    if now_hovered {
        *hovered_model = Some(model.id.to_owned());
    } else if was_hovered {
        *hovered_model = None;
    }
    if now_hovered != was_hovered {
        // 高亮要下一帧才画得出来，这里主动补一帧，不必等鼠标再动一下
        ui.ctx().request_repaint();
    }
    response
}

/// 「视觉理解」小胶囊。
fn badge(ui: &mut egui::Ui, palette: Palette) {
    Frame::new()
        .fill(palette.mint.gamma_multiply(0.25))
        .corner_radius(99)
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            icon(ui, Icon::Eye, 11.0, palette.mint);
            ui.label(RichText::new("视觉理解").size(10.0).color(palette.mint));
        });
}

// ---------------------------------------------------------------- 头像

fn avatar_section(
    ui: &mut egui::Ui,
    settings: &mut ChatSettings,
    textures: &mut super::textures::TextureCache,
    palette: Palette,
    actions: &mut Vec<Action>,
) {
    label(ui, Icon::Person, "更改头像", palette.text, palette.mint);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        match textures.avatar(ui.ctx(), &settings.user_avatar) {
            Some(texture) => {
                let response = ui.add(
                    egui::Image::from_texture(egui::load::SizedTexture::from_handle(&texture))
                        .fit_to_exact_size(Vec2::splat(AVATAR_PREVIEW))
                        .corner_radius(AVATAR_PREVIEW / 2.0),
                );
                ui.painter().circle_stroke(
                    response.rect.center(),
                    AVATAR_PREVIEW / 2.0 - 1.0,
                    Stroke::new(2.0, palette.accent),
                );
            }
            None => {
                ui.allocate_space(Vec2::splat(AVATAR_PREVIEW));
            }
        }

        ui.add_space(4.0);
        let width = (ui.available_width() - 4.0).max(80.0);
        let style = widgets::ButtonStyle {
            fill: palette.row_bg,
            text: palette.text,
            border: true,
            hover_boost: 0.0,
        };
        if widgets::button_with_icon(
            ui,
            Icon::Upload,
            "上传头像",
            Vec2::new(width, 34.0),
            style,
            palette,
        )
        .clicked()
        {
            actions.push(Action::PickAvatar);
        }
    });
}

// ---------------------------------------------------------------- 深浅色

fn theme_section(
    ui: &mut egui::Ui,
    settings: &mut ChatSettings,
    palette: Palette,
    actions: &mut Vec<Action>,
) {
    // 网页版：深色用 moon、浅色用 sun
    let icon_kind = if settings.is_dark_mode {
        Icon::Moon
    } else {
        Icon::Sun
    };
    label(ui, icon_kind, "深色/浅色模式", palette.text, palette.yellow);
    ui.add_space(6.0);

    Frame::new()
        .fill(palette.row_bg)
        .corner_radius(8)
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let text = if settings.is_dark_mode {
                    "深色模式"
                } else {
                    "浅色模式"
                };
                ui.label(RichText::new(text).size(14.0).color(palette.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut on = settings.is_dark_mode;
                    if widgets::switch(ui, &mut on, palette).changed() {
                        actions.push(Action::SetDarkMode(on));
                    }
                });
            });
        });
}

// ---------------------------------------------------------------- 清空

fn clear_section(ui: &mut egui::Ui, palette: Palette, actions: &mut Vec<Action>) {
    label(
        ui,
        Icon::Trash,
        "清空聊天记录",
        palette.danger,
        palette.danger,
    );
    ui.add_space(6.0);

    let style = widgets::ButtonStyle {
        fill: palette.danger,
        text: Color32::WHITE,
        border: false,
        hover_boost: 0.08,
    };
    if widgets::button_with_icon(
        ui,
        Icon::Trash,
        "清空记录",
        Vec2::new(ui.available_width(), 34.0),
        style,
        palette,
    )
    .clicked()
    {
        actions.push(Action::ShowClearConfirm(true));
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new("此操作不可撤销，将删除所有聊天记录")
            .size(11.0)
            .color(palette.muted),
    );
}

/// 「Ave atque vale」呼吸灯艺术字（网页版是渐变文字 + 动画，这里取中间色静态呈现）。
fn footer(ui: &mut egui::Ui, palette: Palette) {
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(6.0);
    ui.vertical_centered(|ui| {
        let glow = mix(palette.accent, palette.mint, 0.5);
        ui.label(RichText::new("Ave atque vale").size(16.0).color(glow));
    });
}

/// 两个颜色的线性插值。
fn mix(from: Color32, to: Color32, t: f32) -> Color32 {
    let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    Color32::from_rgb(
        lerp(from.r(), to.r()),
        lerp(from.g(), to.g()),
        lerp(from.b(), to.b()),
    )
}

// ---------------------------------------------------------------- 公共小件

/// 小节标题：彩色图标 + 粗体文字。
fn label(ui: &mut egui::Ui, icon_kind: Icon, text: &str, text_color: Color32, icon_color: Color32) {
    ui.horizontal(|ui| {
        icon(ui, icon_kind, LABEL_ICON, icon_color);
        ui.label(RichText::new(text).size(14.0).strong().color(text_color));
    });
}

/// 就地画一个小图标。
fn icon(ui: &mut egui::Ui, icon_kind: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    icons::draw(ui.painter(), icon_kind, rect, color, 1.0);
}

/// 小节之间的分隔线。
fn divider(ui: &mut egui::Ui, palette: Palette) {
    ui.add_space(10.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, palette.border);
    ui.add_space(10.0);
}

/// 首次使用（还没配 API Key）时的填写框。
///
/// 网页版把 Key 放在服务端，桌面端没有服务端，只能让用户自己填一次；
/// 填过之后设置抽屉就与网页版完全一致（不再有多出来的「接口」小节）。
pub fn show_key_prompt(
    ctx: &egui::Context,
    palette: Palette,
    api_key: &mut String,
    actions: &mut Vec<Action>,
) {
    egui::Modal::new(egui::Id::new("decem-key-prompt")).show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.label(
            RichText::new("配置 API Key")
                .size(15.0)
                .strong()
                .color(palette.text),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "只有把接口地址改成空字符串（直连百炼）时才需要 Key。默认走网页版服务端，Key 留在服务器上，这里可以留空。",
            )
            .size(12.0)
            .color(palette.muted),
        );
        ui.add_space(8.0);
        let response = ui.add(
            egui::TextEdit::singleline(api_key)
                .password(true)
                .hint_text(RichText::new("sk-...").color(palette.muted))
                .desired_width(f32::INFINITY)
                .frame(
                    Frame::new()
                        .fill(palette.row_bg)
                        .corner_radius(8)
                        .inner_margin(Margin::symmetric(8, 5)),
                ),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("稍后再说").clicked() {
                actions.push(Action::CloseKeyPrompt);
            }
            let save = egui::Button::new(RichText::new("保存").color(Color32::WHITE))
                .fill(palette.accent)
                .corner_radius(8);
            if ui.add(save).clicked() {
                actions.push(Action::SaveApiKey);
            }
        });
        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            actions.push(Action::SaveApiKey);
        }
    });
}

/// 「确认清空」弹窗。
pub fn show_clear_confirm(ctx: &egui::Context, palette: Palette, actions: &mut Vec<Action>) {
    egui::Modal::new(egui::Id::new("decem-clear-confirm")).show(ctx, |ui| {
        ui.set_max_width(320.0);
        ui.horizontal(|ui| {
            icon(ui, Icon::Alert, 18.0, palette.danger);
            ui.label(
                RichText::new("确认清空聊天记录？")
                    .size(15.0)
                    .strong()
                    .color(palette.danger),
            );
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new("此操作将永久删除所有聊天记录，且不可撤销。")
                .size(12.5)
                .color(palette.muted),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("取消").clicked() {
                actions.push(Action::ShowClearConfirm(false));
            }
            let confirm = egui::Button::new(RichText::new("确认清空").color(Color32::WHITE))
                .fill(palette.danger)
                .corner_radius(8);
            if ui.add(confirm).clicked() {
                actions.push(Action::ClearChat);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_test::{has_filled_rect, has_stroked_shape};

    /// 画一行模型，返回这一帧的图形、光标形状，以及行本身是否被判定为悬停。
    fn run_row(
        ctx: &egui::Context,
        selected: bool,
        hovered_model: &mut Option<String>,
        pointer: egui::Pos2,
    ) -> (Vec<egui::Shape>, egui::CursorIcon, bool) {
        run_rows(ctx, &[selected], hovered_model, pointer).remove(0)
    }

    /// 画一列模型（和抽屉里一样是连着画的），返回每行的结果。
    fn run_rows(
        ctx: &egui::Context,
        selected: &[bool],
        hovered_model: &mut Option<String>,
        pointer: egui::Pos2,
    ) -> Vec<(Vec<egui::Shape>, egui::CursorIcon, bool)> {
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(980.0, 600.0),
            )),
            events: vec![egui::Event::PointerMoved(pointer)],
            ..Default::default()
        };
        let palette = crate::theme::palette(true);
        let mut hovered = Vec::new();
        let mut output = ctx.run_ui(raw_input, |ui| {
            hovered.clear();
            for (index, selected) in selected.iter().enumerate() {
                let model = *AVAILABLE_MODELS.get(index).expect("模型表不够长");
                hovered.push(model_row(ui, &model, *selected, hovered_model, palette).hovered());
            }
        });
        output.textures_delta.clear();
        let cursor = output.platform_output.cursor_icon;
        let shapes: Vec<egui::Shape> = output
            .shapes
            .into_iter()
            .map(|clipped| clipped.shape)
            .collect();
        hovered
            .into_iter()
            .map(|hovered| (shapes.clone(), cursor, hovered))
            .collect()
    }

    /// 画面上的薄荷底矩形（悬停/选中那行的底色）。
    fn mint_rects(shapes: &[egui::Shape], palette: Palette) -> Vec<egui::Rect> {
        fn collect(shape: &egui::Shape, palette: Palette, out: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Vec(children) => {
                    for child in children {
                        collect(child, palette, out);
                    }
                }
                egui::Shape::Rect(rect) if rect.fill == palette.mint => out.push(rect.rect),
                _ => {}
            }
        }

        let mut out = Vec::new();
        for shape in shapes {
            collect(shape, palette, &mut out);
        }
        out
    }

    #[test]
    fn only_the_hovered_row_is_highlighted() {
        // 一列里每行都会写「谁被悬停」这个字段：本行不悬停时不能覆盖别人，
        // 否则只有最后一行能高亮（这是真机上踩过的坑）。
        let ctx = egui::Context::default();
        let palette = crate::theme::palette(true);
        let mut hovered = None;
        let away = egui::pos2(100.0, 560.0);

        // 指针不在列表上：没有高亮，也没记下谁
        let rows = run_rows(&ctx, &[false, false, false], &mut hovered, away);
        assert!(
            mint_rects(&rows[0].0, palette).is_empty(),
            "没悬停就不该有高亮"
        );
        assert_eq!(hovered, None);

        // 悬停第二行：只点亮它
        let over = egui::pos2(100.0, 70.0);
        let _ = run_rows(&ctx, &[false, false, false], &mut hovered, over);
        assert_eq!(
            hovered.as_deref(),
            Some("qwen3.7-plus"),
            "记下的应当是第二行"
        );

        let rows = run_rows(&ctx, &[false, false, false], &mut hovered, over);
        let shapes = &rows[0].0;
        let mint = mint_rects(shapes, palette);
        assert_eq!(mint.len(), 1, "同一时刻只该有一行是高亮的");
        let label = crate::flow_test::text_rect(shapes, "Qwen3.7-Plus").expect("应有第二行的名字");
        assert!(
            mint[0].contains(label.center()),
            "点亮的那一行应当是鼠标停着的那一行"
        );
    }

    #[test]
    fn selected_row_is_mint() {
        let ctx = egui::Context::default();
        let mut hovered = None;
        let palette = crate::theme::palette(true);
        let (shapes, _, _) = run_row(&ctx, true, &mut hovered, egui::pos2(100.0, 380.0));
        assert!(
            has_filled_rect(&shapes, palette.mint),
            "选中的行应是薄荷底色"
        );
    }

    #[test]
    fn hovered_row_is_mint() {
        let ctx = egui::Context::default();
        let palette = crate::theme::palette(true);
        let mut hovered = None;
        let over = egui::pos2(100.0, 20.0);
        let away = egui::pos2(100.0, 380.0);

        // 第一帧指针不在行上
        let (shapes, _, _) = run_row(&ctx, false, &mut hovered, away);
        assert!(
            !has_filled_rect(&shapes, palette.mint),
            "没悬停就不该有薄荷底"
        );
        assert!(
            has_stroked_shape(&shapes, palette.mint),
            "「视觉理解」那枚眼睛常驻薄荷色"
        );

        // 第二帧指针挪到行上（这一帧仍按上一帧的结果上色），第三帧才该变薄荷
        let _ = run_row(&ctx, false, &mut hovered, over);
        let (shapes, _cursor, row_hovered) = run_row(&ctx, false, &mut hovered, over);
        assert!(
            has_filled_rect(&shapes, palette.mint),
            "悬停的行应是薄荷底色"
        );
        assert!(
            has_stroked_shape(&shapes, palette.mint_ink),
            "薄荷底上的图标要转成墨色才看得见"
        );
        assert!(row_hovered, "指针在行上时行应当报告 hovered");
        // 光标形状不在这里断言：行里的标签拿不到 hover（上面压着可点区域），
        // 在这里断言会变成空测试 —— 交给「重新生成」那个文字按钮的测试。
    }
}
