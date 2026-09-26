//! 图标：直接画网页版用的那套 lucide 矢量。
//!
//! 几何数据取自 `node_modules` 里的 lucide-react（见 [`super::lucide_data`]），
//! 由 [`super::svg_path`] 把 SVG 路径扁平化后交给 egui 绘制 —— 不存在"手绘近似"。
//! 描边宽度沿用 lucide 的默认值（24 网格里 2px）。

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use super::lucide_data::{self, Node};
use super::svg_path;

/// lucide 的设计网格边长。
const GRID: f32 = 24.0;
/// lucide 的默认描边宽度（网格单位）。
const STROKE: f32 = 2.0;

/// 可绘制的图标，对应 lucide 里的同名图标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// `github`。
    Github,
    /// `settings`（齿轮）。
    Gear,
    /// `image`（图片附件）。
    Image,
    /// `send`（纸飞机）。
    Send,
    /// `moon`。
    Moon,
    /// `sun`。
    Sun,
    /// `sparkles`。
    Sparkles,
    /// `trash-2`。
    Trash,
    /// `upload`。
    Upload,
    /// `user`。
    Person,
    /// `bot`。
    Bot,
    /// `x`。
    Close,
    /// `rotate-ccw`（重新生成）。
    Refresh,
    /// `loader`（等待指示）。
    Loader,
    /// `eye`（视觉理解）。
    Eye,
    /// `chevron-down`。
    ChevronDown,
    /// `check`（下拉选中标记）。
    Check,
    /// `triangle-alert`（警告）。
    Alert,
}

impl Icon {
    /// 对应的 lucide 元素列表。
    fn nodes(self) -> &'static [Node] {
        match self {
            Self::Github => lucide_data::GITHUB,
            Self::Gear => lucide_data::SETTINGS,
            Self::Image => lucide_data::IMAGE,
            Self::Send => lucide_data::SEND,
            Self::Moon => lucide_data::MOON,
            Self::Sun => lucide_data::SUN,
            Self::Sparkles => lucide_data::SPARKLES,
            Self::Trash => lucide_data::TRASH_2,
            Self::Upload => lucide_data::UPLOAD,
            Self::Person => lucide_data::USER,
            Self::Bot => lucide_data::BOT,
            Self::Close => lucide_data::X,
            Self::Refresh => lucide_data::ROTATE_CCW,
            Self::Loader => lucide_data::LOADER,
            Self::Eye => lucide_data::EYE,
            Self::ChevronDown => lucide_data::CHEVRON_DOWN,
            Self::Check => lucide_data::CHECK,
            Self::Alert => lucide_data::TRIANGLE_ALERT,
        }
    }
}

/// 在 `rect` 内绘制图标。
///
/// `weight` 是描边粗细的倍数（1.0 = lucide 默认）。
pub fn draw(painter: &Painter, icon: Icon, rect: Rect, color: Color32, weight: f32) {
    draw_rotated(painter, icon, rect, color, weight, 0.0);
}

/// 同上，但整体绕中心旋转 `angle` 弧度（用于转圈的等待指示）。
pub fn draw_rotated(
    painter: &Painter,
    icon: Icon,
    rect: Rect,
    color: Color32,
    weight: f32,
    angle: f32,
) {
    let side = rect.width().min(rect.height());
    if side <= 0.0 {
        return;
    }
    let scale = side / GRID;
    let center = rect.center();
    let stroke = Stroke::new((STROKE * scale * weight).max(0.6), color);

    let place = |x: f32, y: f32| -> Pos2 {
        let local = Vec2::new((x - GRID * 0.5) * scale, (y - GRID * 0.5) * scale);
        let rotated = if angle == 0.0 {
            local
        } else {
            let (sin, cos) = angle.sin_cos();
            Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
        };
        center + rotated
    };

    for node in icon.nodes() {
        match node {
            Node::Path(d) => {
                let Some(subpaths) = svg_path::flatten(d) else {
                    continue;
                };
                for points in subpaths {
                    if points.len() < 2 {
                        continue;
                    }
                    let screen: Vec<Pos2> = points.into_iter().map(|(x, y)| place(x, y)).collect();
                    painter.add(Shape::line(screen, stroke));
                }
            }
            Node::Circle { cx, cy, r } => {
                if angle == 0.0 {
                    painter.circle_stroke(place(*cx, *cy), r * scale, stroke);
                } else {
                    painter.add(Shape::circle_stroke(place(*cx, *cy), r * scale, stroke));
                }
            }
            Node::Rect {
                x,
                y,
                width,
                height,
                rx,
            } => {
                let min = place(*x, *y);
                let max = place(x + width, y + height);
                let rect = Rect::from_min_max(min, max);
                painter.rect_stroke(rect, rx * scale, stroke, egui::StrokeKind::Middle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_geometry() {
        for icon in [
            Icon::Github,
            Icon::Gear,
            Icon::Image,
            Icon::Send,
            Icon::Moon,
            Icon::Sun,
            Icon::Sparkles,
            Icon::Trash,
            Icon::Upload,
            Icon::Person,
            Icon::Bot,
            Icon::Close,
            Icon::Refresh,
            Icon::Loader,
            Icon::Eye,
            Icon::ChevronDown,
            Icon::Check,
            Icon::Alert,
        ] {
            assert!(!icon.nodes().is_empty(), "{icon:?} 没有几何数据");
        }
    }
}
