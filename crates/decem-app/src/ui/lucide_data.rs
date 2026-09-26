//! lucide 图标数据（自动生成，不要手改）。
//!
//! 数据取自网页版依赖的 lucide-react，保证桌面端与网页端是同一套矢量图标。
//! 重新生成：`node scripts/gen-lucide-icons.mjs`。
//!
//! 来源：lucide-react 0.562.0（ISC 许可）。坐标写在 24×24 网格里，
//! 与 SVG 一致按 stroke-width 2、圆头圆角描边渲染。

/// 一个图标元素。
#[derive(Debug, Clone, Copy)]
pub enum Node {
    /// `d` 属性（可含多段子路径）。
    Path(&'static str),
    /// 圆（`cx`/`cy`/`r`）。
    Circle { cx: f32, cy: f32, r: f32 },
    /// 圆角矩形（`x`/`y`/`width`/`height`/`rx`）。
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rx: f32,
    },
}

/// `github`。
pub const GITHUB: &[Node] = &[
    Node::Path(
        "M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4",
    ),
    Node::Path("M9 18c-4.51 2-5-2-7-2"),
];

/// `settings`。
pub const SETTINGS: &[Node] = &[
    Node::Path(
        "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915",
    ),
    Node::Circle {
        cx: 12.0,
        cy: 12.0,
        r: 3.0,
    },
];

/// `sparkles`。
pub const SPARKLES: &[Node] = &[
    Node::Path(
        "M11.017 2.814a1 1 0 0 1 1.966 0l1.051 5.558a2 2 0 0 0 1.594 1.594l5.558 1.051a1 1 0 0 1 0 1.966l-5.558 1.051a2 2 0 0 0-1.594 1.594l-1.051 5.558a1 1 0 0 1-1.966 0l-1.051-5.558a2 2 0 0 0-1.594-1.594l-5.558-1.051a1 1 0 0 1 0-1.966l5.558-1.051a2 2 0 0 0 1.594-1.594z",
    ),
    Node::Path("M20 2v4"),
    Node::Path("M22 4h-4"),
    Node::Circle {
        cx: 4.0,
        cy: 20.0,
        r: 2.0,
    },
];

/// `send`。
pub const SEND: &[Node] = &[
    Node::Path(
        "M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z",
    ),
    Node::Path("m21.854 2.147-10.94 10.939"),
];

/// `image`。
pub const IMAGE: &[Node] = &[
    Node::Rect {
        x: 3.0,
        y: 3.0,
        width: 18.0,
        height: 18.0,
        rx: 2.0,
    },
    Node::Circle {
        cx: 9.0,
        cy: 9.0,
        r: 2.0,
    },
    Node::Path("m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"),
];

/// `x`。
pub const X: &[Node] = &[Node::Path("M18 6 6 18"), Node::Path("m6 6 12 12")];

/// `loader`。
pub const LOADER: &[Node] = &[
    Node::Path("M12 2v4"),
    Node::Path("m16.2 7.8 2.9-2.9"),
    Node::Path("M18 12h4"),
    Node::Path("m16.2 16.2 2.9 2.9"),
    Node::Path("M12 18v4"),
    Node::Path("m4.9 19.1 2.9-2.9"),
    Node::Path("M2 12h4"),
    Node::Path("m4.9 4.9 2.9 2.9"),
];

/// `rotate-ccw`。
pub const ROTATE_CCW: &[Node] = &[
    Node::Path("M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"),
    Node::Path("M3 3v5h5"),
];

/// `moon`。
pub const MOON: &[Node] = &[Node::Path(
    "M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401",
)];

/// `sun`。
pub const SUN: &[Node] = &[
    Node::Circle {
        cx: 12.0,
        cy: 12.0,
        r: 4.0,
    },
    Node::Path("M12 2v2"),
    Node::Path("M12 20v2"),
    Node::Path("m4.93 4.93 1.41 1.41"),
    Node::Path("m17.66 17.66 1.41 1.41"),
    Node::Path("M2 12h2"),
    Node::Path("M20 12h2"),
    Node::Path("m6.34 17.66-1.41 1.41"),
    Node::Path("m19.07 4.93-1.41 1.41"),
];

/// `trash-2`。
pub const TRASH_2: &[Node] = &[
    Node::Path("M10 11v6"),
    Node::Path("M14 11v6"),
    Node::Path("M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"),
    Node::Path("M3 6h18"),
    Node::Path("M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"),
];

/// `user`。
pub const USER: &[Node] = &[
    Node::Path("M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"),
    Node::Circle {
        cx: 12.0,
        cy: 7.0,
        r: 4.0,
    },
];

/// `bot`。
pub const BOT: &[Node] = &[
    Node::Path("M12 8V4H8"),
    Node::Rect {
        x: 4.0,
        y: 8.0,
        width: 16.0,
        height: 12.0,
        rx: 2.0,
    },
    Node::Path("M2 14h2"),
    Node::Path("M20 14h2"),
    Node::Path("M15 13v2"),
    Node::Path("M9 13v2"),
];

/// `upload`。
pub const UPLOAD: &[Node] = &[
    Node::Path("M12 3v12"),
    Node::Path("m17 8-5-5-5 5"),
    Node::Path("M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"),
];

/// `triangle-alert`。
pub const TRIANGLE_ALERT: &[Node] = &[
    Node::Path("m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"),
    Node::Path("M12 9v4"),
    Node::Path("M12 17h.01"),
];

/// `eye`。
pub const EYE: &[Node] = &[
    Node::Path(
        "M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0",
    ),
    Node::Circle {
        cx: 12.0,
        cy: 12.0,
        r: 3.0,
    },
];

/// `chevron-down`。
pub const CHEVRON_DOWN: &[Node] = &[Node::Path("m6 9 6 6 6-6")];

/// `check`。
pub const CHECK: &[Node] = &[Node::Path("M20 6 9 17l-5-5")];

/// 全部图标（名称 + 元素），供测试遍历使用。
#[cfg(test)]
pub const ALL: &[(&str, &[Node])] = &[
    ("github", GITHUB),
    ("settings", SETTINGS),
    ("sparkles", SPARKLES),
    ("send", SEND),
    ("image", IMAGE),
    ("x", X),
    ("loader", LOADER),
    ("rotate-ccw", ROTATE_CCW),
    ("moon", MOON),
    ("sun", SUN),
    ("trash-2", TRASH_2),
    ("user", USER),
    ("bot", BOT),
    ("upload", UPLOAD),
    ("triangle-alert", TRIANGLE_ALERT),
    ("eye", EYE),
    ("chevron-down", CHEVRON_DOWN),
    ("check", CHECK),
];
