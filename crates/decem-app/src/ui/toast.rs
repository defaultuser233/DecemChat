//! 顶部提示气泡：对应网页版的 sonner toast。

use egui::{Align2, Frame, Margin, RichText, Stroke};

use crate::app::{Toast, ToastKind};
use crate::theme::Palette;

/// 绘制所有未过期的提示。
pub fn show(ctx: &egui::Context, toasts: &[Toast], palette: Palette) {
    if toasts.is_empty() {
        return;
    }

    egui::Area::new(egui::Id::new("decem-toasts"))
        .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 12.0))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            for toast in toasts {
                let (fill, border, text_color) = match toast.kind {
                    ToastKind::Error => (
                        palette.danger.gamma_multiply(0.22),
                        palette.danger,
                        palette.text,
                    ),
                    ToastKind::Info => (palette.card, palette.border, palette.text),
                };
                Frame::new()
                    .fill(fill)
                    .corner_radius(10.0)
                    .inner_margin(Margin::symmetric(14, 9))
                    .stroke(Stroke::new(1.0, border))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        ui.label(RichText::new(&toast.text).size(12.5).color(text_color));
                    });
                ui.add_space(6.0);
            }
        });
}
