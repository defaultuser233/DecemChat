//! 顶栏：Decem 头像、标题与右侧图标入口。

use egui::{Align, Color32, Frame, Layout, Margin, RichText, Stroke, Vec2};

use crate::app::DecemApp;
use crate::theme::Palette;
use crate::ui::icons::{self, Icon};
use crate::ui::widgets;

use super::{AVATAR_SIZE, Action};

/// 仓库地址（与网页版一致）。
const REPO_URL: &str = "https://github.com/defaultuser233/DecemChat";
/// 图标按钮尺寸（网页版 `p-2.5` + `w-5 h-5`）。
const ICON_BOX: f32 = 40.0;

/// 绘制顶栏。
pub fn show(ui: &mut egui::Ui, app: &mut DecemApp, palette: Palette, actions: &mut Vec<Action>) {
    egui::Panel::top("header")
        .frame(
            Frame::new()
                .fill(palette.glass)
                .inner_margin(Margin::symmetric(16, 11))
                .stroke(Stroke::new(1.0, palette.border)),
        )
        .show(ui, |ui| {
            let DecemApp {
                settings,
                textures,
                show_settings,
                ..
            } = app;

            ui.horizontal(|ui| {
                // 头像 + 右下角在线点
                let response = textures
                    .avatar(ui.ctx(), &settings.char_avatar)
                    .map(|texture| {
                        ui.add(
                            egui::Image::from_texture(egui::load::SizedTexture::from_handle(
                                &texture,
                            ))
                            .fit_to_exact_size(Vec2::splat(AVATAR_SIZE))
                            .corner_radius(AVATAR_SIZE / 2.0),
                        )
                    });
                if let Some(response) = response {
                    // 网页版给头像加了呼吸动画与 hover 放大
                    let painter = ui.painter();
                    painter.circle_stroke(
                        response.rect.center(),
                        AVATAR_SIZE / 2.0,
                        Stroke::new(2.0, palette.mint),
                    );
                    painter.circle_filled(
                        response.rect.right_bottom() - Vec2::splat(5.0),
                        5.0,
                        Color32::from_rgb(0x22, 0xC5, 0x5E),
                    );
                } else {
                    ui.allocate_space(Vec2::splat(AVATAR_SIZE));
                }

                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(
                            RichText::new("Decem")
                                .size(18.0)
                                .strong()
                                .color(palette.text),
                        );
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                        icons::draw(ui.painter(), Icon::Sparkles, rect, palette.yellow, 1.0);
                    });
                    ui.label(
                        RichText::new("在线 - 随时陪你聊天")
                            .size(11.5)
                            .color(palette.muted),
                    );
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, Icon::Gear, ICON_BOX, palette.text, palette.row_bg)
                        .on_hover_text(if *show_settings {
                            "关闭设置"
                        } else {
                            "打开设置"
                        })
                        .clicked()
                    {
                        actions.push(Action::ToggleSettings);
                    }

                    if widgets::icon_button(
                        ui,
                        Icon::Github,
                        ICON_BOX,
                        palette.text,
                        palette.row_bg,
                    )
                    .on_hover_text("GitHub 仓库")
                    .clicked()
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(REPO_URL));
                    }
                });
            });
        });
}
