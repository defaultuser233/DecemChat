// 从网页版依赖的 lucide-react 里提取图标几何，生成 crates/decem-app/src/ui/lucide_data.rs。
//
// 用法（仓库根目录）：node scripts/gen-lucide-icons.mjs
// 这样桌面端与本仓库的网页版用的是同一套矢量图标，不存在“手绘近似”。
//
// lucide 采用 ISC 许可（见 node_modules/lucide-react/LICENSE）。

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const lucideDir = fs
  .readdirSync(path.join(root, "node_modules/.pnpm"))
  .filter((name) => name.startsWith("lucide-react@"))
  .map((name) => path.join(root, "node_modules/.pnpm", name, "node_modules/lucide-react"))
  .find((dir) => fs.existsSync(dir));

if (!lucideDir) {
  console.error("找不到 lucide-react，请先 pnpm install");
  process.exit(1);
}

const version = JSON.parse(fs.readFileSync(path.join(lucideDir, "package.json"), "utf8")).version;

// 需要哪些图标：网页版实际用到的那些
const wanted = [
  "github",
  "settings",
  "sparkles",
  "send",
  "image",
  "x",
  "loader",
  "rotate-ccw",
  "moon",
  "sun",
  "trash-2",
  "user",
  "bot",
  "upload",
  "triangle-alert",
  "eye",
  "chevron-down",
  "check",
];

// Rust 的 f32 字段需要浮点字面量：12 / "12" -> 12.0
const num = (value) => {
  const parsed = Number(value);
  return Number.isInteger(parsed) ? `${parsed}.0` : `${parsed}`;
};

const constName = (name) =>
  name.replace(/-([a-z0-9])/g, (_, c) => `_${c.toUpperCase()}`).toUpperCase();

const out = [
  "//! lucide 图标数据（自动生成，不要手改）。",
  "//!",
  "//! 数据取自网页版依赖的 lucide-react，保证桌面端与网页端是同一套矢量图标。",
  "//! 重新生成：`node scripts/gen-lucide-icons.mjs`。",
  "//!",
  `//! 来源：lucide-react ${version}（ISC 许可）。坐标写在 24×24 网格里，`,
  "//! 与 SVG 一致按 stroke-width 2、圆头圆角描边渲染。",
  "",
  "/// 一个图标元素。",
  "#[derive(Debug, Clone, Copy)]",
  "pub enum Node {",
  "    /// `d` 属性（可含多段子路径）。",
  "    Path(&'static str),",
  "    /// 圆（`cx`/`cy`/`r`）。",
  "    Circle { cx: f32, cy: f32, r: f32 },",
  "    /// 圆角矩形（`x`/`y`/`width`/`height`/`rx`）。",
  "    Rect {",
  "        x: f32,",
  "        y: f32,",
  "        width: f32,",
  "        height: f32,",
  "        rx: f32,",
  "    },",
  "}",
  "",
];

const generated = [];
for (const name of wanted) {
  const file = path.join(lucideDir, "dist/esm/icons", `${name}.js`);
  if (!fs.existsSync(file)) {
    console.error(`缺少图标文件：${name}`);
    process.exit(1);
  }
  const source = fs.readFileSync(file, "utf8");
  const match = source.match(/const __iconNode = ([\s\S]*?);\nconst/);
  if (!match) {
    console.error(`无法解析图标：${name}`);
    process.exit(1);
  }
  const nodes = eval(match[1]);
  out.push(`/// \`${name}\`。`, `pub const ${constName(name)}: &[Node] = &[`);
  for (const [tag, attrs] of nodes) {
    if (tag === "path") {
      out.push(`    Node::Path(${JSON.stringify(attrs.d)}),`);
    } else if (tag === "circle") {
      out.push(
        `    Node::Circle { cx: ${num(attrs.cx)}, cy: ${num(attrs.cy)}, r: ${num(attrs.r)} },`,
      );
    } else if (tag === "rect") {
      out.push(
        `    Node::Rect { x: ${num(attrs.x)}, y: ${num(attrs.y)}, width: ${num(attrs.width)}, height: ${num(attrs.height)}, rx: ${num(attrs.rx ?? 0)} },`,
      );
    } else {
      console.error(`图标 ${name} 用了尚未支持的元素：${tag}`);
      process.exit(1);
    }
  }
  out.push("];", "");
  generated.push(name);
}

out.push(
  "/// 全部图标（名称 + 元素），供测试遍历使用。",
  "#[cfg(test)]",
  "pub const ALL: &[(&str, &[Node])] = &[",
  ...generated.map((name) => `    ("${name}", ${constName(name)}),`),
  "];",
  "",
);

const target = path.join(root, "crates/decem-app/src/ui/lucide_data.rs");
fs.writeFileSync(target, out.join("\n"));
console.log(`已生成 ${generated.length} 个图标 -> ${path.relative(root, target)}（lucide-react ${version}）`);
