//! 输入区：图片附件、多行输入、Enter 发送 / Shift+Enter 换行。
//!
//! 版式对齐网页版 `ChatInput.tsx`：整条通栏的圆角输入框，左侧图片图标、
//! 右侧纸飞机图标，下方居中提示文字。

use egui::{Color32, Frame, Margin, RichText, Stroke};

use crate::app::{DecemApp, PendingImage};
use crate::theme::{self, Palette};
use crate::ui::icons::Icon;
use crate::ui::widgets;

use super::{Action, textures::TextureCache};

/// 输入框的稳定 ID（用于判断焦点与拦截 Enter）。
const INPUT_ID: &str = "decem-composer-input";
/// 预览缩略图高度（网页版 `h-20`）。
const PREVIEW_HEIGHT: f32 = 80.0;
/// 输入区最大高度。
const INPUT_MAX_HEIGHT: f32 = 120.0;
/// 图标按钮尺寸（网页版 `w-10 h-10`）。
const BUTTON_SIZE: f32 = 40.0;
/// 行内间距（网页版 `gap-2`）。
const ROW_GAP: f32 = 8.0;
/// 输入框最小宽度。
const INPUT_MIN_WIDTH: f32 = 120.0;

/// 绘制底部输入区。
pub fn show(ui: &mut egui::Ui, app: &mut DecemApp, palette: Palette, actions: &mut Vec<Action>) {
    let busy = app.is_busy();
    let can_send = (!app.input.trim().is_empty() || app.pending_image.is_some()) && !busy;

    egui::Panel::bottom("composer")
        .frame(
            Frame::new()
                .fill(palette.glass)
                .inner_margin(Margin::symmetric(12, 12))
                .stroke(Stroke::new(1.0, palette.border)),
        )
        .show(ui, |ui| {
            let DecemApp {
                input,
                pending_image,
                textures,
                ..
            } = app;

            if let Some(image) = pending_image.as_ref() {
                draw_preview(ui, image, textures, palette, actions);
            }

            let input_id = egui::Id::new(INPUT_ID);
            let focused = ui.ctx().memory(|memory| memory.has_focus(input_id));
            let send_key = focused && take_enter_key(ui);

            egui::Frame::new()
                .fill(palette.row_bg)
                .corner_radius(16)
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = ROW_GAP;

                        // 网页版在「正在加载」或「已有图片」时禁用附图按钮。
                        // 颜色是用户点名要的橙色：图标常驻橙色，悬停底色是它的淡版。
                        let can_attach = !busy && pending_image.is_none();
                        let attach = widgets::icon_button(
                            ui,
                            Icon::Image,
                            BUTTON_SIZE,
                            if can_attach {
                                theme::BUTTON_ORANGE
                            } else {
                                theme::BUTTON_ORANGE.gamma_multiply(0.4)
                            },
                            theme::BUTTON_ORANGE.gamma_multiply(0.12),
                        )
                        .on_hover_text("选择要发送的图片（最大 5MB，GIF 会自动抽帧）");
                        if attach.clicked() && can_attach {
                            actions.push(Action::PickImage);
                        }

                        // 宽度在本行内部算：用面板外的可用宽度会把发送按钮顶到窗口边上
                        let input_width =
                            (ui.available_width() - ROW_GAP - BUTTON_SIZE).max(INPUT_MIN_WIDTH);
                        egui::ScrollArea::vertical()
                            .max_height(INPUT_MAX_HEIGHT)
                            .max_width(input_width)
                            .id_salt("composer-input-scroll")
                            .show(ui, |ui| {
                                let hint = if pending_image.is_some() {
                                    "添加描述（可选）..."
                                } else {
                                    "发个消息聊聊呗～"
                                };
                                ui.add(
                                    egui::TextEdit::multiline(input)
                                        .id(input_id)
                                        .desired_rows(1)
                                        .desired_width(input_width)
                                        .frame(egui::Frame::NONE)
                                        .hint_text(RichText::new(hint).color(palette.muted)),
                                );
                            });

                        // 可发送时橙底 + 白图标；空闲时保留暗底（那是「现在还不能发」的提示），
                        // 但图标也画成橙色。等待回复时图标换成旋转指示器。
                        let (fill, icon_color) = if can_send {
                            (theme::BUTTON_ORANGE, Color32::WHITE)
                        } else {
                            (palette.border.gamma_multiply(0.7), theme::BUTTON_ORANGE)
                        };
                        let send = if busy {
                            let angle = ui.input(|input| input.time) as f32 * std::f32::consts::TAU;
                            widgets::icon_button_spinner(ui, BUTTON_SIZE, fill, icon_color, angle)
                                .on_hover_text("正在等待回复…")
                        } else {
                            widgets::icon_button_filled(
                                ui,
                                Icon::Send,
                                BUTTON_SIZE,
                                fill,
                                icon_color,
                            )
                            .on_hover_text("发送（Enter）")
                        };
                        if send.clicked() || (send_key && can_send) {
                            actions.push(Action::Send);
                        }
                    });
                });

            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("按 Enter 发送，Shift + Enter 换行")
                        .size(11.0)
                        .color(palette.muted),
                );
            });
        });
}

/// 图片预览 + 移除按钮。
fn draw_preview(
    ui: &mut egui::Ui,
    image: &PendingImage,
    textures: &mut TextureCache,
    palette: Palette,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        if let Some(texture) = textures.data_url(ui.ctx(), &image.id, &image.data_url) {
            let size = texture.size_vec2();
            let scale = (PREVIEW_HEIGHT / size.y).min(1.0);
            ui.add(
                egui::Image::from_texture(egui::load::SizedTexture::from_handle(&texture))
                    .fit_to_exact_size(size * scale)
                    .corner_radius(8.0),
            );
        }
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&image.display_name)
                    .size(11.5)
                    .color(palette.muted),
            );
            if let Some(frames) = image.gif_frames {
                ui.label(
                    RichText::new(format!("GIF 已抽取 {frames} 帧，发送时将以拼接图识别"))
                        .size(11.0)
                        .color(palette.yellow),
                );
            }
            if widgets::icon_button(ui, Icon::Close, 26.0, palette.muted, palette.row_bg)
                .on_hover_text("移除图片")
                .clicked()
            {
                actions.push(Action::DropPendingImage);
            }
        });
    });
    ui.add_space(2.0);
}

/// 拦截「不带 Shift 的 Enter」，避免多行输入框把它当换行。
fn take_enter_key(ui: &mut egui::Ui) -> bool {
    ui.input_mut(|input| {
        let matches_plain_enter = |event: &egui::Event| {
            matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.shift
            )
        };
        let pressed = input.events.iter().any(matches_plain_enter);
        if pressed {
            input.events.retain(|event| !matches_plain_enter(event));
        }
        pressed
    })
}
