//! SVG 路径解析与扁平化。
//!
//! lucide 的图标就是 SVG 的 `<path d="...">`，要画进 egui 就得把 `d` 解析成折线。
//! 这里只实现这些图标真正用到的命令（M/L/H/V/C/S/Q/T/A/Z 及其相对形式 + 隐式重复），
//! 遇到不认识的命令返回 `None` —— 由测试保证已收录的图标都能完整解析。

/// 网格坐标（与 SVG 的 24×24 视口一致）。
pub type Point = (f32, f32);

/// 把 `d` 解析成若干条折线；命令不合法时返回 `None`。
///
/// 每条子路径的首点就是第一个点；闭合子路径会显式把首点补在末尾，
/// 这样绘制时不需要再判断是否闭合。
pub fn flatten(d: &str) -> Option<Vec<Vec<Point>>> {
    let mut lexer = Lexer::new(d);
    let mut subpaths: Vec<Vec<Point>> = Vec::new();
    let mut current: Vec<Point> = Vec::new();

    // 当前点、子路径起点、上一条曲线的控制点（供 S/T 反射）
    let mut x = 0.0_f32;
    let mut y = 0.0_f32;
    let mut start = (0.0_f32, 0.0_f32);
    let mut last_cubic_ctrl: Option<Point> = None;
    let mut last_quad_ctrl: Option<Point> = None;

    let mut command: Option<char> = None;
    loop {
        if let Some(next) = lexer.peek_command() {
            lexer.consume_command();
            command = Some(next);
        } else {
            // 没有新的命令字母时重复上一个命令（M/m 的后续点按 L/l 处理）
            command = match command? {
                'M' => Some('L'),
                'm' => Some('l'),
                other => Some(other),
            };
        }

        let cmd = command?;
        match cmd {
            'M' | 'm' => {
                let (nx, ny) = lexer.pair(cmd.is_ascii_lowercase(), (x, y))?;
                if !current.is_empty() {
                    subpaths.push(std::mem::take(&mut current));
                }
                x = nx;
                y = ny;
                start = (x, y);
                current.push((x, y));
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'L' | 'l' => {
                let (nx, ny) = lexer.pair(cmd.is_ascii_lowercase(), (x, y))?;
                x = nx;
                y = ny;
                current.push((x, y));
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'H' | 'h' => {
                let value = lexer.number()?;
                x = if cmd == 'h' { x + value } else { value };
                current.push((x, y));
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'V' | 'v' => {
                let value = lexer.number()?;
                y = if cmd == 'v' { y + value } else { value };
                current.push((x, y));
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'C' | 'c' => {
                let relative = cmd == 'c';
                let c1 = lexer.pair(relative, (x, y))?;
                let c2 = lexer.pair(relative, (x, y))?;
                let end = lexer.pair(relative, (x, y))?;
                sample_cubic(&mut current, (x, y), c1, c2, end);
                x = end.0;
                y = end.1;
                last_cubic_ctrl = Some(c2);
                last_quad_ctrl = None;
            }
            'S' | 's' => {
                let relative = cmd == 's';
                let c1 = reflect(last_cubic_ctrl, (x, y));
                let c2 = lexer.pair(relative, (x, y))?;
                let end = lexer.pair(relative, (x, y))?;
                sample_cubic(&mut current, (x, y), c1, c2, end);
                x = end.0;
                y = end.1;
                last_cubic_ctrl = Some(c2);
                last_quad_ctrl = None;
            }
            'Q' | 'q' => {
                let relative = cmd == 'q';
                let ctrl = lexer.pair(relative, (x, y))?;
                let end = lexer.pair(relative, (x, y))?;
                sample_quad(&mut current, (x, y), ctrl, end);
                x = end.0;
                y = end.1;
                last_quad_ctrl = Some(ctrl);
                last_cubic_ctrl = None;
            }
            'T' | 't' => {
                let relative = cmd == 't';
                let ctrl = reflect(last_quad_ctrl, (x, y));
                let end = lexer.pair(relative, (x, y))?;
                sample_quad(&mut current, (x, y), ctrl, end);
                x = end.0;
                y = end.1;
                last_quad_ctrl = Some(ctrl);
                last_cubic_ctrl = None;
            }
            'A' | 'a' => {
                let rx = lexer.number()?;
                let ry = lexer.number()?;
                let rotation = lexer.number()?;
                let large_arc = lexer.flag()?;
                let sweep = lexer.flag()?;
                let (nx, ny) = lexer.pair(cmd == 'a', (x, y))?;
                sample_arc(
                    &mut current,
                    (x, y),
                    (nx, ny),
                    rx,
                    ry,
                    rotation,
                    large_arc,
                    sweep,
                );
                x = nx;
                y = ny;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'Z' | 'z' => {
                if let Some(first) = current.first().copied() {
                    current.push(first);
                    subpaths.push(std::mem::take(&mut current));
                }
                x = start.0;
                y = start.1;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            _ => return None,
        }

        lexer.skip_separators();
        if lexer.at_end() {
            break;
        }
    }

    if current.len() > 1 {
        subpaths.push(current);
    }
    Some(subpaths)
}

fn reflect(ctrl: Option<Point>, current: Point) -> Point {
    match ctrl {
        Some((cx, cy)) => (2.0 * current.0 - cx, 2.0 * current.1 - cy),
        None => current,
    }
}

fn sample_cubic(out: &mut Vec<Point>, from: Point, c1: Point, c2: Point, to: Point) {
    let steps = curve_steps(&[from, c1, c2, to]);
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let u = 1.0 - t;
        let x =
            u * u * u * from.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * to.0;
        let y =
            u * u * u * from.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * to.1;
        out.push((x, y));
    }
}

fn sample_quad(out: &mut Vec<Point>, from: Point, ctrl: Point, to: Point) {
    let steps = curve_steps(&[from, ctrl, to]);
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let u = 1.0 - t;
        let x = u * u * from.0 + 2.0 * u * t * ctrl.0 + t * t * to.0;
        let y = u * u * from.1 + 2.0 * u * t * ctrl.1 + t * t * to.1;
        out.push((x, y));
    }
}

/// 采样密度：按控制多边形的长度取，够平滑又不至于点太多。
fn curve_steps(points: &[Point]) -> usize {
    let mut length = 0.0;
    for pair in points.windows(2) {
        length += ((pair[1].0 - pair[0].0).powi(2) + (pair[1].1 - pair[0].1).powi(2)).sqrt();
    }
    ((length / 0.35).ceil() as usize).clamp(6, 48)
}

/// 按 SVG 规范的端点参数化把椭圆弧转成圆心参数化再采样。
#[allow(clippy::too_many_arguments)]
fn sample_arc(
    out: &mut Vec<Point>,
    from: Point,
    to: Point,
    rx: f32,
    ry: f32,
    rotation_deg: f32,
    large_arc: bool,
    sweep: bool,
) {
    if rx == 0.0 || ry == 0.0 || from == to {
        out.push(to);
        return;
    }

    let phi = rotation_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();
    let (mut rx, mut ry) = (rx.abs(), ry.abs());

    let dx = (from.0 - to.0) / 2.0;
    let dy = (from.1 - to.1) / 2.0;
    let x1 = cos_phi * dx + sin_phi * dy;
    let y1 = -sin_phi * dx + cos_phi * dy;

    // 半径太小就按规范等比放大
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }

    let numerator = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let factor = if denominator == 0.0 {
        0.0
    } else {
        (numerator / denominator).sqrt()
    };
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let cx1 = sign * factor * (rx * y1 / ry);
    let cy1 = sign * factor * (-ry * x1 / rx);

    let cx = cos_phi * cx1 - sin_phi * cy1 + (from.0 + to.0) / 2.0;
    let cy = sin_phi * cx1 + cos_phi * cy1 + (from.1 + to.1) / 2.0;

    let theta = |ux: f32, uy: f32| uy.atan2(ux);
    let start_angle = theta((x1 - cx1) / rx, (y1 - cy1) / ry);
    let end_angle = theta((-x1 - cx1) / rx, (-y1 - cy1) / ry);

    let mut delta = end_angle - start_angle;
    if !sweep && delta > 0.0 {
        delta -= std::f32::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f32::consts::TAU;
    }

    let steps = ((delta.abs() / 0.2).ceil() as usize).clamp(6, 64);
    for step in 1..=steps {
        let angle = start_angle + delta * (step as f32 / steps as f32);
        let (sin_a, cos_a) = angle.sin_cos();
        let px = cos_phi * rx * cos_a - sin_phi * ry * sin_a + cx;
        let py = sin_phi * rx * cos_a + cos_phi * ry * sin_a + cy;
        out.push((px, py));
    }
}

/// SVG 路径的极简词法分析器。
struct Lexer<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            index: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.index >= self.bytes.len()
    }

    fn skip_separators(&mut self) {
        while let Some(byte) = self.bytes.get(self.index) {
            if byte.is_ascii_whitespace() || *byte == b',' {
                self.index += 1;
            } else {
                break;
            }
        }
    }

    fn peek_command(&self) -> Option<char> {
        let mut probe = self.index;
        while let Some(byte) = self.bytes.get(probe) {
            if byte.is_ascii_whitespace() || *byte == b',' {
                probe += 1;
            } else {
                break;
            }
        }
        let byte = *self.bytes.get(probe)?;
        byte.is_ascii_alphabetic().then_some(byte as char)
    }

    fn consume_command(&mut self) {
        self.skip_separators();
        self.index += 1;
    }

    fn number(&mut self) -> Option<f32> {
        self.skip_separators();
        let start = self.index;
        if matches!(self.bytes.get(self.index), Some(b'+') | Some(b'-')) {
            self.index += 1;
        }
        while matches!(self.bytes.get(self.index), Some(byte) if byte.is_ascii_digit()) {
            self.index += 1;
        }
        if self.bytes.get(self.index) == Some(&b'.') {
            self.index += 1;
            while matches!(self.bytes.get(self.index), Some(byte) if byte.is_ascii_digit()) {
                self.index += 1;
            }
        }
        if matches!(self.bytes.get(self.index), Some(b'e') | Some(b'E')) {
            self.index += 1;
            if matches!(self.bytes.get(self.index), Some(b'+') | Some(b'-')) {
                self.index += 1;
            }
            while matches!(self.bytes.get(self.index), Some(byte) if byte.is_ascii_digit()) {
                self.index += 1;
            }
        }
        if self.index == start {
            return None;
        }
        std::str::from_utf8(&self.bytes[start..self.index])
            .ok()?
            .parse()
            .ok()
    }

    /// 弧线的 large-arc / sweep 标志：只占一个字符。
    fn flag(&mut self) -> Option<bool> {
        self.number().and_then(|value| match value as i32 {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        })
    }

    /// 读一对坐标；`relative` 时相对 `origin` 偏移。
    fn pair(&mut self, relative: bool, origin: Point) -> Option<Point> {
        let x = self.number()?;
        let y = self.number()?;
        Some(if relative {
            (origin.0 + x, origin.1 + y)
        } else {
            (x, y)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::lucide_data;

    #[test]
    fn parses_straight_lines() {
        let subpaths = flatten("M2 2L10 2 18 2").expect("应能解析");
        assert_eq!(subpaths.len(), 1);
        assert_eq!(subpaths[0], [(2.0, 2.0), (10.0, 2.0), (18.0, 2.0)]);
    }

    #[test]
    fn handles_relative_and_vertical_horizontal() {
        let subpaths = flatten("M4 4h4v4h-4z").expect("应能解析");
        assert_eq!(subpaths.len(), 1);
        // 闭合时首点会补在末尾
        assert_eq!(
            subpaths[0],
            [(4.0, 4.0), (8.0, 4.0), (8.0, 8.0), (4.0, 8.0), (4.0, 4.0)]
        );
    }

    #[test]
    fn multiple_subpaths_are_separated() {
        let subpaths = flatten("M10 11v6M14 11v6").expect("应能解析");
        assert_eq!(subpaths.len(), 2);
        assert_eq!(subpaths[0], [(10.0, 11.0), (10.0, 17.0)]);
        assert_eq!(subpaths[1], [(14.0, 11.0), (14.0, 17.0)]);
    }

    #[test]
    fn cubic_and_smooth_curves_are_sampled() {
        let subpaths = flatten("M15 22v-4c3 0 6-2 6-5.5s.5-3-2-3").expect("应能解析");
        let points = &subpaths[0];
        assert!(points.len() > 8, "曲线应被采样成多个点：{}", points.len());
        assert!(points.iter().all(|(x, y)| x.is_finite() && y.is_finite()));
    }

    #[test]
    fn arcs_are_sampled() {
        let subpaths = flatten("M19 21v-2a4 4 0 0 0-4-4").expect("应能解析");
        let points = &subpaths[0];
        assert!(points.len() > 4);
        assert!(points.iter().all(|(x, y)| x.is_finite() && y.is_finite()));
        // 弧的终点应当落在给定坐标上（允许采样误差）
        let last = points.last().copied().unwrap_or_default();
        assert!(
            (last.0 - 15.0).abs() < 0.05 && (last.1 - 15.0).abs() < 0.05,
            "{last:?}"
        );
    }

    #[test]
    fn unknown_command_is_rejected() {
        assert!(flatten("M0 0K10 10").is_none());
    }

    #[test]
    fn every_bundled_icon_parses_into_finite_points() {
        for (name, nodes) in lucide_data::ALL {
            for node in *nodes {
                match node {
                    lucide_data::Node::Path(d) => {
                        let subpaths =
                            flatten(d).unwrap_or_else(|| panic!("{name} 的路径解析失败：{d}"));
                        assert!(!subpaths.is_empty(), "{name} 的路径没有产生子路径");
                        for point in subpaths.iter().flatten() {
                            assert!(
                                point.0.is_finite() && point.1.is_finite(),
                                "{name} 产生了非法坐标 {point:?}"
                            );
                            assert!(
                                (-1.0..=25.0).contains(&point.0)
                                    && (-1.0..=25.0).contains(&point.1),
                                "{name} 的坐标越出 24×24 网格：{point:?}"
                            );
                        }
                    }
                    lucide_data::Node::Circle { cx, cy, r } => {
                        assert!(
                            *r > 0.0 && cx.is_finite() && cy.is_finite(),
                            "{name} 的圆非法"
                        );
                    }
                    lucide_data::Node::Rect { width, height, .. } => {
                        assert!(*width > 0.0 && *height > 0.0, "{name} 的矩形非法");
                    }
                }
            }
        }
    }
}
